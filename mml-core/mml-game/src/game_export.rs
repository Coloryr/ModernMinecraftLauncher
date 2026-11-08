//! 实例导出为整合包

#[cfg(test)]
#[path = "../tests/unit/game_export_tests.rs"]
mod tests;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use mml_base::{
    archives::{ArchiveAddFile, ArchiveType, BaseArchive, BaseArchiveGui},
    file_item::FileHash,
    serialize_tools,
};
use mml_config::config_obj::RunArgObj;
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};

use mml_net::{
    curseforge_api::{self, list_obj::AuthorsObj},
    modrinth_api::version_obj::HasheObj,
};

use crate::{
    curseforge::pack_obj::{CurseForgePackObj, FilesObj, MinecraftObj, ModLoadersObj},
    launcher::{
        self, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
        project_save_obj::MmlProjectSaveObj,
    },
    loader::LoaderType,
    modrinth::pack_obj::{ModrinthPackFileObj, ModrinthPackObj},
    other_launcher::{
        hmcl_obj::{AddonsObj, HMCLObj},
        mmc_obj::{ComponentsObj, MMCObj},
    },
};

/// 导出压缩包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPackType {
    /// MML 格式
    MML,
    /// CurseForge 格式
    CurseForge,
    /// Modrinth 格式
    Modrinth,
    /// MMC 格式，整包实例，供 MultiMC / Prism 导入
    MMC,
    /// HMCL 格式，整包实例，供 HMCL 导入
    HMCL,
    /// 纯压缩包，只打包文件不带整合包清单
    Zip,
}

/// 导出的在线文件
pub struct OnlineFileExport {
    /// 文件路径
    pub file: PathBuf,
    /// 文件大小，单位字节
    pub size: usize,
    /// 导出方式
    pub pack: ExportPackType,
    /// 下载地址
    pub url: String,
    /// 文件校验值
    pub hash: FileHash,
    /// 文件在线信息，非线上来源时为 `None`
    pub info: Option<OnlineInfoObj>,
}

/// 导出需要的参数
pub struct ExportArg {
    /// 导出保存的位置
    pub file: PathBuf,
    /// 打包类型
    pub pack: ExportPackType,
    /// 压缩类型
    pub archive: ArchiveType,
    /// 在线模组信息
    pub mods: Vec<OnlineFileExport>,
    /// 在线文件信息
    pub files: Vec<OnlineFileExport>,
    /// 不打包的文件
    pub unselect: Vec<PathBuf>,
    /// 一起打包的文件
    pub select: Vec<PathBuf>,
    /// 取消令牌，置位后在写下一个文件前收手；`None` 表示不中途取消
    pub cancel: Option<Arc<AtomicBool>>,
    /// 整合包名字
    pub name: String,
    /// 整合包作者
    pub author: String,
    /// 整合包版本号
    pub version: String,
    /// 整合包简介
    pub summary: String,
    /// 压缩进度条
    pub gui: BaseArchiveGui,
}

impl InstanceSettingObj {
    /// 导出整合包
    ///
    /// - `arg`: 导出参数
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；打包失败返回对应错误
    pub async fn export(&self, arg: ExportArg) -> CoreResult<()> {
        match arg.pack {
            ExportPackType::MML => colormc(self, arg),
            ExportPackType::CurseForge => curseforge(self, arg).await,
            ExportPackType::Modrinth => modrinth(self, arg).await,
            ExportPackType::MMC => mmc(self, arg),
            ExportPackType::HMCL => hmcl(self, arg),
            ExportPackType::Zip => zip_pack(self, arg),
        }
    }
}

/// 导出是否已被取消
///
/// - `cancel`: 取消令牌，`None` 表示不中途取消
///
/// # 返回值
///
/// 已置位时返回任务取消错误，调用方据此停下
fn check_cancel(cancel: &Option<Arc<AtomicBool>>) -> CoreResult<()> {
    if cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::Relaxed))
    {
        return Err(ErrorType::TaskCancel);
    }
    Ok(())
}

/// 将作者列表拼接为字符串
///
/// - `author`: 作者列表
///
/// # 返回值
///
/// 返回拼接后的作者名；空列表返回空串
fn get_author_string(author: &Vec<AuthorsObj>) -> String {
    if author.is_empty() {
        String::new()
    } else {
        let mut data = String::new();

        for item in author.iter() {
            data.push_str(&item.name);
        }

        data
    }
}

/// 导出为 ColorMC 整合包格式
///
/// - `game`: 目标实例
/// - `data`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
fn colormc(game: &InstanceSettingObj, data: ExportArg) -> CoreResult<()> {
    let mut list = data.unselect;
    list.push(game.get_online_info_file());

    let list = list
        .iter()
        .map(|item| item.to_string_lossy().to_string())
        .collect();

    let mut list1 = HashMap::new();
    for item in data.mods.iter() {
        if let Some(info) = &item.info {
            list1.insert(info.modid.clone(), info.clone());
        }
    }

    let mut archive = BaseArchive::compress(
        data.archive,
        &data.file,
        &game.get_base_path(),
        None,
        &Some(list),
        data.gui.clone(),
    )?;
    archive.add_data(
        names::MOD_INFO_FILE,
        &serialize_tools::json_to_bytes(&list1)?,
    )?;

    Ok(())
}

/// 导出为纯压缩包：整个实例目录原样打包，不写任何整合包清单
///
/// 包内就是实例目录本身：游戏文件在 `.minecraft/` 下，`game.json` 等实例元数据在根。
/// 因为没有清单，这份包不能被任何启动器当整合包导入，只能手工解包使用。
///
/// - `game`: 目标实例
/// - `data`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
fn zip_pack(game: &InstanceSettingObj, data: ExportArg) -> CoreResult<()> {
    let list = data
        .unselect
        .iter()
        .map(|item| item.to_string_lossy().to_string())
        .collect();

    BaseArchive::compress(
        data.archive,
        &data.file,
        &game.get_base_path(),
        None,
        &Some(list),
        data.gui.clone(),
    )?;

    Ok(())
}

/// 导出为 CurseForge 整合包格式
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
async fn curseforge(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut obj = CurseForgePackObj {
        name: arg.name,
        author: arg.author,
        version: arg.version,
        manifest_type: "minecraftModpack".to_string(),
        manifest_version: 1,
        overrides: names::OVERRIDE_DIR.to_string(),
        minecraft: MinecraftObj {
            version: game.version.clone(),
            mod_loaders: Vec::new(),
        },
        ..Default::default()
    };

    if game.loader != LoaderType::Normal {
        obj.minecraft.mod_loaders.push(ModLoadersObj {
            id: format!(
                "{}-{}",
                game.loader.id(),
                game.loader_version.clone().unwrap_or_default()
            ),
            primary: true,
        });
    }

    for item in arg.mods {
        if let Some(info) = item.info {
            obj.files.push(FilesObj {
                file_id: info.fileid.parse::<u64>().unwrap_or_default(),
                project_id: info.modid.parse::<u64>().unwrap_or_default(),
                required: true,
            });
        }
    }

    // modlist.html 只是给人看的索引，项目名与链接要联网查：查不到就写个空列表，
    // 没有可索引的模组时干脆不查，别让网络问题把整个导出卡住
    let mut project_ids = obj
        .files
        .iter()
        .map(|item| item.project_id)
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    project_ids.sort_unstable();
    project_ids.dedup();
    let mut html = String::from("<ul>");
    if !project_ids.is_empty() {
        match curseforge_api::get_mods_info(project_ids).await {
            Ok(data) => {
                for item in data.data {
                    html.push_str(&format!(
                        "<li><a href=\"{}\"{} (by {})</a></li>",
                        item.links.website_url,
                        item.name,
                        get_author_string(&item.authors)
                    ));
                }
            }
            Err(err) => mml_log::error_type(err),
        }
    }
    html.push_str("</ul>");

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;

    if let Some(gui) = &arg.gui {
        gui.start(arg.select.len() + 2);
    }

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MANIFEST_FILE.to_string()), 0);
    }

    zip.add_data(names::MANIFEST_FILE, &serialize_tools::json_to_bytes(&obj)?)?;

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MOD_LIST_FILE.to_string()), 0);
    }

    zip.add_data(names::MOD_LIST_FILE, &html.as_bytes())?;

    let path = game.get_game_path();
    let mut pending = Vec::with_capacity(arg.select.len());

    for item in &arg.select {
        check_cancel(&arg.cancel)?;
        let rel = match item.strip_prefix(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let name = if rel_str.starts_with('/') {
            format!("{}{}", names::OVERRIDE_DIR, rel_str)
        } else {
            format!("{}/{}", names::OVERRIDE_DIR, rel_str)
        };

        pending.push(ArchiveAddFile {
            src: item.clone(),
            dest: PathBuf::from(name),
        });
    }

    // 一次写完：逐条 add_file 每写一条都要重写一遍中央目录
    zip.add_files_reporting(&pending, arg.gui.clone(), 2)?;

    Ok(())
}

/// 导出为 Modrinth 整合包格式
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
async fn modrinth(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut obj = ModrinthPackObj {
        format_version: 1,
        game: String::from(names::MINECRAFT_KEY),
        version_id: arg.version,
        name: arg.name,
        summary: arg.summary,
        files: Vec::new(),
        dependencies: HashMap::new(),
    };

    obj.dependencies
        .insert(String::from(names::MINECRAFT_KEY), game.version.clone());
    match game.loader {
        LoaderType::Forge => {
            obj.dependencies.insert(
                String::from(names::FORGE_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        LoaderType::Fabric => {
            obj.dependencies.insert(
                String::from(names::FABRIC_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        LoaderType::Quilt => {
            obj.dependencies.insert(
                String::from(names::QUILT_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        LoaderType::NeoForge => {
            obj.dependencies.insert(
                String::from(names::NEOFORGE_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        _ => {}
    }

    let path = game.get_game_path();

    for item in arg.mods {
        if let Some(info) = item.info {
            let rel = match item.file.strip_prefix(&path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            obj.files.push(ModrinthPackFileObj {
                path: rel.to_string_lossy().replace('\\', "/"),
                hashes: HasheObj {
                    sha1: item.hash.get_sha1().unwrap_or_default(),
                    sha512: item.hash.get_sha512().unwrap_or_default(),
                },
                downloads: vec![item.url],
                file_size: item.size as u64,
                project: Some(MmlProjectSaveObj {
                    source_type: launcher::get_source_type(&info.modid, &info.fileid),
                    pid: info.modid.clone(),
                    fid: info.fileid.clone(),
                }),
            });
        }
    }

    // 没有 projectID、只有下载地址的条目：本地来源的模组、或用户自己填了地址的文件。
    // 它们照样能写进 index 的 files（downloads 给地址，project 留空），
    // 与 overrides 的区别是"由谁提供这份文件"
    for item in arg.files {
        let rel = match item.file.strip_prefix(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        obj.files.push(ModrinthPackFileObj {
            path: rel.to_string_lossy().replace('\\', "/"),
            hashes: HasheObj {
                sha1: item.hash.get_sha1().unwrap_or_default(),
                sha512: item.hash.get_sha512().unwrap_or_default(),
            },
            downloads: vec![item.url],
            file_size: item.size as u64,
            project: None,
        });
    }

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;

    if let Some(gui) = &arg.gui {
        gui.start(arg.select.len() + 1);
    }

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MODRINTH_FILE.to_string()), 0);
    }

    zip.add_data(names::MODRINTH_FILE, &serialize_tools::json_to_bytes(&obj)?)?;

    let mut pending = Vec::with_capacity(arg.select.len());

    for item in &arg.select {
        check_cancel(&arg.cancel)?;
        let rel = match item.strip_prefix(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let name = if rel_str.starts_with('/') {
            format!("{}{}", names::OVERRIDE_DIR, rel_str)
        } else {
            format!("{}/{}", names::OVERRIDE_DIR, rel_str)
        };

        pending.push(ArchiveAddFile {
            src: item.clone(),
            dest: PathBuf::from(name),
        });
    }

    // 一次写完：逐条 add_file 每写一条都要重写一遍中央目录
    zip.add_files_reporting(&pending, arg.gui.clone(), 1)?;

    Ok(())
}

/// 把根目录下的一批文件按 `<prefix>/<相对根目录的路径>` 加进压缩包
///
/// 两种整包格式的差别只在根与包内前缀：MMC 以实例目录为根、前缀是实例名；
/// HMCL 以游戏目录为根、前缀是 overrides。
///
/// - `zip`: 目标压缩包
/// - `base`: 选文件时的根目录
/// - `prefix`: 包内前缀，末尾不带 `/`
/// - `files`: 待打包的文件
/// - `gui`: 进度回调，`None` 时不上报
/// - `offset`: 进度起点，回调里的序号加在它上面
/// - `cancel`: 取消令牌，见 [`check_cancel`]
///
/// # 返回值
///
/// 打包失败或被取消时返回对应错误
fn add_game_files(
    zip: &mut BaseArchive,
    base: &Path,
    prefix: &str,
    files: &[PathBuf],
    gui: &BaseArchiveGui,
    offset: usize,
    cancel: &Option<Arc<AtomicBool>>,
) -> CoreResult<()> {
    let mut pending = Vec::with_capacity(files.len());
    for item in files {
        check_cancel(cancel)?;
        // 不在游戏目录下的文件直接跳过：包内路径只能相对它算
        let Ok(rel) = item.strip_prefix(base) else {
            continue;
        };
        let name = format!("{}/{}", prefix, rel.to_string_lossy().replace('\\', "/"));
        pending.push(ArchiveAddFile {
            src: item.clone(),
            dest: PathBuf::from(name),
        });
    }

    // 一次写完：逐条 add_file 每写一条都要重写一遍中央目录
    zip.add_files_reporting(&pending, gui.clone(), offset)
}

/// 整包格式要打包的文件：没有下载清单，模组与文件一律进包
///
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 待打包的文件列表
fn pack_all_files(arg: &ExportArg) -> Vec<PathBuf> {
    let mut files = arg.select.clone();
    files.extend(arg.mods.iter().map(|item| item.file.clone()));
    files.extend(arg.files.iter().map(|item| item.file.clone()));
    files
}

/// 导出为 MMC 实例包：整包实例，供 MultiMC / Prism 导入
///
/// 包内是"一个实例目录"：`mmc-pack.json` 写组件、`instance.cfg` 写实例设置，
/// 勾选的文件按实例目录结构放入：`.minecraft/` 仍在 `.minecraft/`，实例根上的文件留在根。
/// 组件 uid 与导入侧 `other_launcher` 的
/// `to_instance` 判据一一对应，导出的包能被本启动器原样导回来。
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误。自定义加载器在 MMC 里没有对应组件，
/// 这时只导游戏版本、不写加载器组件
fn mmc(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
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

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;
    // 顶层套一层实例目录：MMC / Prism 从 zip 导入时按它建实例
    let inst_dir = format!("{}/", game.name);
    let mut files = pack_all_files(&arg);

    if let Some(gui) = &arg.gui {
        gui.start(files.len() + 2);
        gui.update(Some(names::MMCJSON_FILE.to_string()), 0);
    }

    let data = serialize_tools::json_to_bytes(&MMCObj {
        format_version: 1,
        components,
    })?;
    zip.add_data(&format!("{inst_dir}{}", names::MMCJSON_FILE), &data)?;

    // 图标：MMC 的 iconKey 就是图标文件名去掉扩展名，导入时按它在实例根目录找同名图片；
    // 原图直接进包，不再另造一份 icon.png
    let icon = game.get_icon_file_or_default();
    let icon_name = icon
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let icon_key = icon
        .is_file()
        .then(|| icon.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .flatten();

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MMCCFG_FILE.to_string()), 1);
    }

    let jvm_args = mmc_jvm_args(game.jvm_arg.as_ref());
    zip.add_data(
        &format!("{inst_dir}{}", names::MMCCFG_FILE),
        mmc_instance_cfg(&game.name, icon_key.as_deref(), game.jvm_arg.as_ref(), &jvm_args)
            .as_bytes(),
    )?;

    if let Some(name) = icon_name.filter(|_| icon.is_file()) {
        // 勾选里本来就带上图标时不要再写一遍：同一路径两条记录会让导入方取到哪条看实现
        files.retain(|file| file != &icon);
        zip.add_file(&format!("{inst_dir}{name}"), &icon)?;
    }

    // 勾选相对实例目录：.minecraft 原样落在实例目录下，实例根上的文件也按原位置带走
    add_game_files(
        &mut zip,
        &game.get_base_path(),
        inst_dir.trim_end_matches('/'),
        &files,
        &arg.gui,
        2,
        &arg.cancel,
    )
}

/// MMC 的 `instance.cfg`：按 MMC 自己的完整格式写一份，不只是一两个键
///
/// Prism / MultiMC 的 cfg 是带 `[General]` 段的 INI；缺了那些 Override / ConfigVersion
/// 键时，导入方会按自己的默认值处理，表现为"设置没带过来"。
///
/// - `name`: 实例名
/// - `icon_key`: 包根图标文件名去掉扩展名，如 `gtnh_icon`；没有图标时传 `None`，写 `default`
/// - `jvm`: 源实例的 JVM 设置，内存上下限写进 cfg
/// - `jvm_args`: 已转成 MMC 变量的 JVM 参数，空表时不写 `JvmArgs`
///
/// # 返回值
///
/// INI 文本，行尾是 `\n`
pub fn mmc_instance_cfg(
    name: &str,
    icon_key: Option<&str>,
    jvm: Option<&RunArgObj>,
    jvm_args: &[String],
) -> String {
    let max = jvm.and_then(|jvm| jvm.max_memory);
    let min = jvm.and_then(|jvm| jvm.min_memory);
    let mut text = String::from("[General]\n");
    text.push_str("InstanceType=OneSix\n");
    text.push_str("JoinServerOnLaunch=false\n");
    text.push_str("OverrideCommands=false\n");
    text.push_str("OverrideConsole=false\n");
    text.push_str("OverrideGameTime=false\n");
    text.push_str(&format!("OverrideJavaArgs={}\n", !jvm_args.is_empty()));
    text.push_str("OverrideJavaLocation=false\n");
    text.push_str(&format!("OverrideMemory={}\n", max.is_some() || min.is_some()));
    text.push_str("OverrideNativeWorkarounds=false\n");
    text.push_str("OverrideWindow=false\n");
    text.push_str(&format!("iconKey={}\n", icon_key.unwrap_or("default")));
    text.push_str(&format!("name={name}\n"));
    text.push_str("notes=\n");
    text.push_str("ConfigVersion=1.2\n");
    text.push_str("AutomaticJava=false\n");
    text.push_str("ExportAuthor=\n");
    text.push_str("ExportName=\n");
    text.push_str("ExportOptionalFiles=true\n");
    text.push_str("ExportSummary=\n");
    text.push_str("ExportVersion=1.0.0\n");
    text.push_str("LogPrePostOutput=true\n");
    text.push_str("ManagedPack=false\n");
    text.push_str("ManagedPackID=\n");
    text.push_str("ManagedPackName=\n");
    text.push_str("ManagedPackType=\n");
    text.push_str("ManagedPackVersionID=\n");
    text.push_str("ManagedPackVersionName=\n");
    text.push_str("OverrideEnv=false\n");
    text.push_str("OverrideLegacySettings=false\n");
    text.push_str("OverrideMiscellaneous=false\n");
    text.push_str("OverridePerformance=false\n");
    text.push_str("Profiler=\n");
    text.push_str("UseAccountForInstance=false\n");
    text.push_str("lastLaunchTime=0\n");
    text.push_str("lastTimePlayed=0\n");
    text.push_str("linkedInstances=[]\n");
    text.push_str("totalTimePlayed=0\n");
    if let Some(max) = max {
        text.push_str(&format!("MaxMemAlloc={max}\n"));
    }
    if let Some(min) = min {
        text.push_str(&format!("MinMemAlloc={min}\n"));
    }
    if !jvm_args.is_empty() {
        text.push_str(&format!("JvmArgs={}\n", jvm_args.join(" ")));
    }
    text
}

/// 把源实例的自定义 JVM 参数转成 MMC 写法
///
/// - `jvm`: 源实例的 JVM 设置
///
/// # 返回值
///
/// 逐条替换占位符后的参数；没有自定义参数时为空表
fn mmc_jvm_args(jvm: Option<&RunArgObj>) -> Vec<String> {
    jvm.and_then(|jvm| jvm.jvm_args.as_deref())
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(mmc_vars)
        .collect()
}

/// 把 MML 路径占位符换成 MMC 变量
///
/// MMC 的变量是单个 $ 前缀、没有结尾 $，如 $INST_DIR；由启动器在启动前做文本替换。
///
/// - `text`: 待替换的 JVM 参数 / 命令文本
///
/// # 返回值
///
/// 替换后的文本；没有对应 MMC 变量的占位符（如 %LAUNCH_DIR%）原样保留
pub fn mmc_vars(text: &str) -> String {
    text.replace(names::ARG_GAME_BASE_DIR, "$INST_DIR")
        .replace(names::ARG_GAME_DIR, "$INST_MC_DIR")
        .replace(names::ARG_JAVA_LOCAL, "$INST_JAVA")
        .replace(names::ARG_JAVA_ARG, "$INST_JAVA_ARGS")
        .replace(names::ARG_GAME_NAME, "$INST_NAME")
        .replace(names::ARG_GAME_UUID, "$INST_ID")
}

/// 导出为 HMCL 整合包：整包实例，供 HMCL 导入
///
/// 包内是根目录的 `mcbbs.packmeta`（HMCL 的组件表）加 `manifest.json`，
/// 游戏文件放在 `overrides/` 下。`manifest.json` 是 HMCL 自己不会写、但本启动器
/// 导入 HMCL 包时**必读**的一份，用它的 overrides 字段定位游戏文件，所以导出必须带上。
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误。自定义加载器在 HMCL 里没有对应组件，
/// 这时只导游戏版本
fn hmcl(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let obj = HMCLObj {
        name: game.name.clone(),
        addons: hmcl_addons(game),
        launch_info: None,
    };

    let mut manifest = CurseForgePackObj {
        name: arg.name.clone(),
        author: arg.author.clone(),
        version: arg.version.clone(),
        manifest_type: "minecraftModpack".to_string(),
        manifest_version: 1,
        overrides: names::OVERRIDE_DIR.to_string(),
        minecraft: MinecraftObj {
            version: game.version.clone(),
            mod_loaders: Vec::new(),
        },
        ..Default::default()
    };
    if game.loader != LoaderType::Normal {
        manifest.minecraft.mod_loaders.push(ModLoadersObj {
            id: format!(
                "{}-{}",
                game.loader.id(),
                game.loader_version.clone().unwrap_or_default()
            ),
            primary: true,
        });
    }

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;
    let files = pack_all_files(&arg);

    if let Some(gui) = &arg.gui {
        gui.start(files.len() + 2);
        gui.update(Some(names::HMCLFILE.to_string()), 0);
    }

    zip.add_data(names::HMCLFILE, &serialize_tools::json_to_bytes(&obj)?)?;

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MANIFEST_FILE.to_string()), 1);
    }

    zip.add_data(
        names::MANIFEST_FILE,
        &serialize_tools::json_to_bytes(&manifest)?,
    )?;

    add_game_files(
        &mut zip,
        &game.get_game_path(),
        names::OVERRIDE_DIR,
        &files,
        &arg.gui,
        2,
        &arg.cancel,
    )
}

/// HMCL 的组件表：game 加加载器
///
/// 组件 id 复用 names 里那几个键，取值与导入侧 `to_instance` 的判据一致。
///
/// - `game`: 目标实例
///
/// # 返回值
///
/// 组件列表，game 在前
fn hmcl_addons(game: &InstanceSettingObj) -> Vec<AddonsObj> {
    let mut list = vec![AddonsObj {
        id: names::GAME_KEY.to_string(),
        version: game.version.clone(),
    }];

    let id = match game.loader {
        LoaderType::Forge => Some(names::FORGE_KEY),
        LoaderType::NeoForge => Some(names::NEOFORGE_KEY),
        LoaderType::Fabric => Some(names::FABRIC_KEY),
        LoaderType::Quilt => Some(names::QUILT_KEY),
        _ => None,
    };
    if let Some(id) = id {
        list.push(AddonsObj {
            id: id.to_string(),
            version: game.loader_version.clone().unwrap_or_default(),
        });
    }

    list
}
