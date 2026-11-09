//! 服务器包导出
//!
//! 把一个本地实例打成 MMC 结构的实例包，同时带一份 `game.json`，MultiMC / Prism / HMCL
//! 与 M²L 都能直接导入。包内顶层目录是服务器实例名，同步 agent 与生成好的 `server.json`
//! 放在 `.minecraft`，`instance.cfg` 用相对路径挂上 agent：MMC / Prism 的工作目录是
//! `.minecraft`，HMCL 导入时把 `.minecraft` 内容摊到实例根，两边都能按相对路径找到。

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use mml_base::{
    archives::{ArchiveAddFile, ArchiveType, BaseArchive, IBaseArchiveGui},
    serialize_tools,
};
use mml_game::{
    launcher::{ModPackType, instance_setting_obj::InstanceSettingObj},
    loader::LoaderType,
    other_launcher::mmc_obj::{ComponentsObj, MMCObj},
    serverpack::serverpack_obj::ServerPackObj,
};
use mml_names::names;
use uuid::Uuid;

use super::{config_dir, files, normalize_url, ConfigMode};
use crate::{dtos::server_instance_dto::ServerInstanceOptionsDto, err_keys};

/// 内嵌的同步 agent，导入后由 MMC 的 `JvmArgs` 拉起来
pub(super) const SYNC_JAR: &[u8] = include_bytes!("../../../resources/mml-server-sync.jar");

/// 同步 agent 的文件名，同时用作包内条目名与 `instance.cfg` 里的引用名
const SYNC_JAR_NAME: &str = "mml-server-sync.jar";

/// 自动生成的顶层条目，基础文件勾到同名文件时跳过
const RESERVED_NAMES: [&str; 4] = [
    names::MMCJSON_FILE,
    names::MMCCFG_FILE,
    names::GAME_FILE,
    names::SERVER_FILE,
];

/// 导出服务器包
///
/// - `game`: 源实例
/// - `options`: 生成选项，包名 / 发布地址 / 勾选都取自它
/// - `file`: 输出的 zip 路径，父目录不存在会创建
/// - `complete`: `true` 连同模组 / 资源包 / 配置目录一起打包，`false` 只带基础文件
/// - `progress`: 写入包内条目时的进度回调
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；包名为空、路径越界或打包失败返回错误文案
pub(super) fn export_pack(
    game: &InstanceSettingObj,
    options: &ServerInstanceOptionsDto,
    file: &Path,
    complete: bool,
    progress: Arc<dyn Fn(usize, usize, String) + Send + Sync>,
) -> Result<(), String> {
    let name = options.name.trim();
    if name.is_empty() {
        return Err(err_keys::FILE_NAME.to_string());
    }
    let base_url = normalize_url(&options.url)?;
    let manifest_url = format!("{base_url}/{}", names::SERVER_FILE);
    let icon = icon_file(game);
    // 包内带一份生成好的清单：导入后第一次同步拿它当旧清单，删除与可选模组状态才对得上
    let manifest = read_manifest(options)?;

    let data = serialize_tools::json_to_bytes(&MMCObj {
        format_version: 1,
        components: components(game),
    })
    .map_err(|e| e.to_string())?;
    let icon_key = icon.as_ref().and_then(|(icon_name, _)| {
        Path::new(icon_name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
    });
    // 源实例的自定义参数也带上；旧的同步 agent 由下面这条新的替代
    let mut jvm_args: Vec<String> = game
        .jvm_arg
        .as_ref()
        .and_then(|jvm| jvm.jvm_args.as_deref())
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_sync_agent(line))
        .map(mml_game::game_export::mmc_vars)
        .collect();
    // 相对路径：MMC / Prism 的工作目录是 .minecraft，HMCL 导入后把 .minecraft 内容摊到实例根
    jvm_args.push(format!("-javaagent:{SYNC_JAR_NAME}={manifest_url}"));
    let cfg = mml_game::game_export::mmc_instance_cfg(
        name,
        icon_key.as_deref(),
        game.jvm_arg.as_ref(),
        &jvm_args,
    );
    let game_json = game_json(
        game,
        name,
        &base_url,
        icon.as_ref().map(|(name, _)| name.as_str()),
    )?;

    // 键是包内相对实例根的路径，基础文件与完整包内容重名时自然去重
    let mut pending: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let root = game.get_base_path();
    for rel in files::base_files(&root, &options.base_files)? {
        pending.insert(rel.clone(), root.join(&rel));
    }
    if complete {
        let game_path = game.get_game_path();
        for pick in files::mods(&game_path, &options.mods)? {
            let rel = PathBuf::from(names::GAME_DIR).join(&pick.path);
            pending.insert(rel.clone(), game_path.join(&pick.path));
        }
        for pick in &options.resource_packs {
            let rel = files::file(&game_path, &pick.path)?;
            pending.insert(
                PathBuf::from(names::GAME_DIR).join(&rel),
                game_path.join(&rel),
            );
        }
        for item in &options.configs {
            let dir = config_dir(&item.dir)?;
            // 删除目录没有可放进包里的内容，清单里的空压缩包由同步 agent 处理
            let mut list = match ConfigMode::parse(&item.mode)? {
                ConfigMode::ArchiveAll | ConfigMode::FilesAll => files::dir_files(&game_path, &dir)?,
                ConfigMode::ArchiveSelf | ConfigMode::FilesSelf => {
                    files::dir_direct_files(&game_path, &dir)?
                }
                ConfigMode::Delete | ConfigMode::None => continue,
            };
            files::retain_excluded(&game_path, &mut list, &item.exclude)?;
            for rel in list {
                pending.insert(
                    PathBuf::from(names::GAME_DIR).join(&rel),
                    game_path.join(&rel),
                );
            }
        }
    }
    // 实例图标原样放在包根：M²L 的 game.json 按 Icon 字段找它，MMC 的 iconKey 就是文件名去扩展名
    if let Some((icon_name, icon_path)) = &icon {
        pending.insert(PathBuf::from(icon_name), icon_path.clone());
    }
    pending.retain(|rel, _| !is_reserved(rel) && !is_reserved_inside(rel));

    // 只给文件名没有父目录时不用建目录
    if let Some(parent) = file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let inst = format!("{name}/");
    let inner = format!("{}/", names::GAME_DIR);
    // 六个自动条目 + 待打包文件；进度条按"当前写到哪个文件"上报
    let total = 6 + pending.len();
    let mut zip =
        BaseArchive::create_empty(ArchiveType::Zip, file).map_err(|e| e.to_string())?;
    progress(0, total, names::MMCJSON_FILE.to_string());
    zip.add_data(&format!("{inst}{}", names::MMCJSON_FILE), &data)
        .map_err(|e| e.to_string())?;
    progress(1, total, names::MMCCFG_FILE.to_string());
    zip.add_data(&format!("{inst}{}", names::MMCCFG_FILE), cfg.as_bytes())
        .map_err(|e| e.to_string())?;
    progress(2, total, names::GAME_FILE.to_string());
    zip.add_data(&format!("{inst}{}", names::GAME_FILE), &game_json)
        .map_err(|e| e.to_string())?;
    progress(3, total, names::SERVER_FILE.to_string());
    zip.add_data(&format!("{inst}{}", names::SERVER_FILE), &manifest)
        .map_err(|e| e.to_string())?;
    // agent 与清单也放进 .minecraft：HMCL 导入 MMC 包时只搬 .minecraft，
    // 摊到实例根后相对路径的 agent 与 server.json 都在
    progress(4, total, format!("{inner}{SYNC_JAR_NAME}"));
    zip.add_data(&format!("{inst}{inner}{SYNC_JAR_NAME}"), SYNC_JAR)
        .map_err(|e| e.to_string())?;
    progress(5, total, format!("{inner}{}", names::SERVER_FILE));
    zip.add_data(&format!("{inst}{inner}{}", names::SERVER_FILE), &manifest)
        .map_err(|e| e.to_string())?;

    let mut pending_files = Vec::with_capacity(pending.len());
    for (rel, src) in pending {
        pending_files.push(ArchiveAddFile {
            src,
            dest: PathBuf::from(format!("{inst}{}", files::wire_path(&rel))),
        });
    }
    let gui = Some(Arc::new(PackProgress {
        progress: Arc::clone(&progress),
        total,
    }) as Arc<dyn IBaseArchiveGui>);
    zip.add_files_reporting(&pending_files, gui, 6)
        .map_err(|e| e.to_string())?;
    // 没有待打包文件时上面不会有 update，收尾也要推到 100%
    progress(total, total, String::new());
    Ok(())
}

/// 按实例信息生成 MMC 组件表
///
/// - `game`: 源实例
///
/// # 返回值
///
/// 第一个固定是 `net.minecraft`；加载器能在 MMC 里表达时再跟一个加载器组件
fn components(game: &InstanceSettingObj) -> Vec<ComponentsObj> {
    let mut components = vec![ComponentsObj {
        cached_version: game.version.clone(),
        uid: "net.minecraft".to_string(),
        version: game.version.clone(),
    }];
    let uid = match game.loader {
        LoaderType::Forge => Some("net.minecraftforge"),
        LoaderType::NeoForge => Some("net.neoforged"),
        LoaderType::Fabric => Some("net.fabricmc.fabric-loader"),
        LoaderType::Quilt => Some("org.quiltmc.quilt-loader"),
        _ => None,
    };
    if let Some(uid) = uid {
        let version = game.loader_version.clone().unwrap_or_default();
        components.push(ComponentsObj {
            cached_version: version.clone(),
            uid: uid.to_string(),
            version,
        });
    }
    components
}

/// 生成给 M²L 识别的 `game.json`
///
/// 只保留实例设置：名字 / 标识 / 目录换成包里的，类型标成服务器实例，
/// Java 清掉（玩家机器上不一定有同名 Java），图标保留并在包里带上同名文件。
///
/// - `game`: 源实例
/// - `name`: 服务器实例名
/// - `base_url`: 发布地址，去掉末尾斜杠
/// - `icon`: 包根下的图标文件名；没有图标时为 `None`
///
/// # 返回值
///
/// 序列化后的 JSON 字节；失败时返回错误文案
fn game_json(
    game: &InstanceSettingObj,
    name: &str,
    base_url: &str,
    icon: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut obj = game.clone();
    obj.name = name.to_string();
    obj.uuid = Uuid::new_v4();
    obj.dir = String::new();
    obj.is_modpack = true;
    obj.modpack_type = ModPackType::ServerPack;
    obj.server_url = Some(base_url.to_string());
    obj.jvm_name = None;
    obj.jvm_local = None;
    obj.icon = icon.map(str::to_string);
    strip_sync_agent(&mut obj);
    serialize_tools::json_to_bytes(&obj).map_err(|e| e.to_string())
}

/// 源实例的图标：文件名与绝对路径
///
/// - `game`: 源实例
///
/// # 返回值
///
/// 图标文件存在时返回 `(文件名, 绝对路径)`；没有图标文件返回 `None`
fn icon_file(game: &InstanceSettingObj) -> Option<(String, PathBuf)> {
    let path = game.get_icon_file_or_default();
    if !path.is_file() {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().into_owned();
    Some((name, path))
}

/// 去掉 JVM 参数里指向旧实例根的同步 agent
///
/// MML 导入后按 `ServerUrl` 自己同步，再留一条 `-javaagent:` 会指向不存在的路径。
///
/// - `game`: 待处理的实例设置
fn strip_sync_agent(game: &mut InstanceSettingObj) {
    let Some(jvm) = game.jvm_arg.as_mut() else {
        return;
    };
    let Some(args) = jvm.jvm_args.as_deref() else {
        return;
    };
    let kept: Vec<&str> = args
        .lines()
        .filter(|line| !is_sync_agent(line))
        .collect();
    if kept.len() == args.lines().count() {
        return;
    }
    let text = kept.join("\n");
    jvm.jvm_args = if text.trim().is_empty() {
        None
    } else {
        Some(text)
    };
}

/// 这一行是否是指向同步 agent 的 `-javaagent:` 参数
///
/// - `line`: 一行 JVM 参数
///
/// # 返回值
///
/// 同时含 `-javaagent:` 与 agent 文件名时返回 true
fn is_sync_agent(line: &str) -> bool {
    line.contains("-javaagent:") && line.contains(SYNC_JAR_NAME)
}

/// 是否与自动生成的顶层条目重名
///
/// - `rel`: 相对实例根的路径
///
/// # 返回值
///
/// 顶层且命中保留名时返回 true
fn is_reserved(rel: &Path) -> bool {
    if rel.components().count() != 1 {
        return false;
    }
    let Some(name) = rel.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    RESERVED_NAMES
        .iter()
        .any(|item| name.eq_ignore_ascii_case(item))
}

/// 包内 .minecraft 下由导出代码固定的条目
///
/// 基础文件勾到 .minecraft 整目录时跳过这两个，避免同一路径写两遍。
///
/// - rel: 相对实例根的路径
///
/// # 返回值
///
/// .minecraft/mml-server-sync.jar 或 .minecraft/server.json 返回 true
fn is_reserved_inside(rel: &Path) -> bool {
    let dir = Path::new(names::GAME_DIR);
    let jar = dir.join(SYNC_JAR_NAME);
    let manifest = dir.join(names::SERVER_FILE);
    rel == jar.as_path() || rel == manifest.as_path()
}

/// 读取已生成的服务器实例清单
///
/// 服务器包会把清单放在实例根：M²L 与同步 agent 第一次同步时拿它当旧清单，
/// 删除旧文件、可选模组的启用状态才对得上。
///
/// - `options`: 生成选项，`folder` 是清单所在目录
///
/// # 返回值
///
/// 清单字节；目录为空、文件不存在或不是合法清单时返回 `err.serverInstanceManifest`
fn read_manifest(options: &ServerInstanceOptionsDto) -> Result<Vec<u8>, String> {
    let folder = options.folder.trim();
    if folder.is_empty() {
        return Err(err_keys::SERVER_INSTANCE_MANIFEST.to_string());
    }
    let path = Path::new(folder).join(names::SERVER_FILE);
    let data = fs::read(path).map_err(|_| err_keys::SERVER_INSTANCE_MANIFEST.to_string())?;
    serialize_tools::json_from_bytes::<ServerPackObj>(&data)
        .map_err(|_| err_keys::SERVER_INSTANCE_MANIFEST.to_string())?;
    Ok(data)
}

/// 压缩包逐条写入时的进度回调，转成服务器实例进度事件
struct PackProgress {
    /// 进度回调：已写条数、总数、当前文件名
    progress: Arc<dyn Fn(usize, usize, String) + Send + Sync>,
    /// 总条数，含六个自动条目
    total: usize,
}

impl IBaseArchiveGui for PackProgress {
    fn start(&self, _total: usize) {}

    fn update(&self, filename: Option<String>, current: usize) {
        (self.progress)(current, self.total, filename.unwrap_or_default());
    }

    fn done(&self) {}

    fn file_rename(&self, _name: &str) -> bool {
        true
    }
}
