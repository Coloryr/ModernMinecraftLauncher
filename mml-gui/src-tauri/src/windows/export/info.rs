//! 可分享的实例信息报告
//!
//! 导出窗口的「生成实例信息」把实例整理成一份 JSON，方便贴给别人排查：
//! 实例与加载器、启动设置、模组 / 资源清单、启动器与平台环境。
//!
//! **不写**服务器地址、Java 本地路径、代理凭据与自定义命令：这几项最容易带出个人信息，
//! 而排查也不需要它们。格式标识 `mml-instance-info-v2`，结构变了就升版本。

use std::{
    collections::HashMap,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use mml_base::hash_helper::{HashType, gen_hash_from_file};
use mml_config::config_obj::GCType;
use mml_game::{
    launcher::{
        LogEncoding, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    loader::LoaderType,
};
use mml_names::names;
use serde::Serialize;

/// 报告格式标识
const FORMAT: &str = "mml-instance-info-v2";

/// 报告里不在实例设置内的取值
pub(super) struct ReportEnv {
    /// 界面语言，形如 `zh_cn`
    pub(super) ui_lang: String,
    /// 全局默认的 HOME 隔离；实例没写时按它算生效值
    pub(super) global_isolate: bool,
}

/// 生成可分享的实例信息
///
/// - `uuid`: 实例标识
/// - `file`: 用户选择的 JSON 保存路径
///
/// # 返回值
///
/// 写入结果；实例不存在、读取目录或写盘失败返回错误文案
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub async fn export_generate_info(uuid: String, file: String) -> Result<(), String> {
    // 报告要遍历整个实例目录，跑完之前不许删它
    let _task = crate::windows::instance_task::begin(&uuid)?;
    let game = super::super::instance_snapshot(&uuid)?;
    tokio::task::spawn_blocking(move || {
        let env = ReportEnv {
            ui_lang: mml_names::get_lang(crate::gui_config::locale()).to_string(),
            global_isolate: mml_config::read_config()
                .jvm_arg
                .isolate_home
                .unwrap_or(true),
        };
        let online = online_of(&game);
        let report = report(&game, &game.get_game_path(), &env, &online)?;
        let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
        crate::windows::export_file::write(Path::new(&file), |output| {
            output.write_all(&bytes).map_err(|e| e.to_string())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 实例信息报告
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct InstanceReport {
    /// 报告格式标识
    format: &'static str,
    /// 启动器版本与构建通道
    launcher: LauncherSection,
    /// 运行平台
    system: SystemSection,
    /// 语言与日志编码
    locale: LocaleSection,
    /// 实例显示名
    name: String,
    /// 实例标识
    uuid: uuid::Uuid,
    /// 实例目录名，不含运行目录前缀
    directory: String,
    /// Minecraft 版本
    minecraft: String,
    /// 版本类型，如 `release` / `snapshot`
    game_type: &'static str,
    /// 加载器 ID，如 `forge`
    loader: &'static str,
    /// 加载器版本，原版为空
    loader_version: Option<String>,
    /// 是否使用自定义加载器
    custom_loader: bool,
    /// 整合包来源
    modpack: ModpackSection,
    /// 启动设置
    launch: LaunchSection,
    /// 模组列表
    mods: Vec<ModEntry>,
    /// 资源包
    resource_packs: Vec<FileEntry>,
    /// 光影包
    shader_packs: Vec<FileEntry>,
    /// 存档目录与其中的数据包
    saves: Vec<SaveEntry>,
    /// 结构文件
    schematics: Vec<FileEntry>,
    /// 截图数量
    screenshots: usize,
}

/// 启动器版本与构建通道
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct LauncherSection {
    /// 版本号
    version: String,
    /// 构建通道：`release` / `prerelease` / `debug`
    channel: &'static str,
}

/// 运行平台
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct SystemSection {
    /// 操作系统，取自 `std::env::consts::OS`
    os: &'static str,
    /// CPU 架构，取自 `std::env::consts::ARCH`
    arch: &'static str,
}

/// 语言与日志编码
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct LocaleSection {
    /// 界面语言
    ui: String,
    /// 日志编码：`utf8` / `gbk`
    log_encoding: &'static str,
}

/// 整合包来源
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ModpackSection {
    /// 类型：`curseforge` / `modrinth` / `mcmod` / `serverpack` / `none`
    kind: String,
    /// 项目号
    project_id: Option<String>,
    /// 文件号
    file_id: Option<String>,
}

/// 启动设置
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct LaunchSection {
    /// 最小内存，单位 MB；没写表示跟随启动器默认
    min_memory: Option<u32>,
    /// 最大内存，单位 MB；没写表示跟随启动器默认
    max_memory: Option<u32>,
    /// GC 模式：`auto` / `g1gc` / `zgc` / `none`；键与内核配置保持一致
    #[serde(rename = "GC")]
    gc: &'static str,
    /// 附加 JVM 参数
    jvm_args: Vec<String>,
    /// 附加游戏参数
    game_args: Vec<String>,
    /// 自定义主类
    main_class: Option<String>,
    /// 附加 classpath
    class_path: Vec<String>,
    /// HOME / APPDATA 是否隔离到实例目录
    isolate_home: bool,
    /// 指定使用的 Java 名；不含路径
    java_name: Option<String>,
    /// 游戏窗口宽度，单位像素
    window_width: Option<u16>,
    /// 游戏窗口高度，单位像素
    window_height: Option<u16>,
    /// 是否全屏
    fullscreen: bool,
}

/// 一个模组
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ModEntry {
    /// 相对游戏目录的路径，如 `mods/foo.jar`
    path: String,
    /// 是否启用；带 `.disable` / `.disabled` 后缀的是停用
    enabled: bool,
    /// 文件大小，单位字节
    size: u64,
    /// 文件 SHA1；读不到文件时退回实例记录的在线校验值
    sha1: Option<String>,
    /// 项目号
    project_id: Option<String>,
    /// 文件号
    file_id: Option<String>,
    /// 下载地址
    url: Option<String>,
}

/// 一个文件
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct FileEntry {
    /// 相对游戏目录的路径
    path: String,
    /// 文件大小，单位字节
    size: u64,
    /// SHA1；资源包 / 光影包 / 结构文件不读内容，恒为 `None`
    sha1: Option<String>,
}

/// 一个存档
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct SaveEntry {
    /// 存档目录名，也是游戏内 `saves/<目录名>` 的那一段
    directory: String,
    /// 目录总大小，单位字节
    size: u64,
    /// 存档内数据包的文件名
    data_packs: Vec<String>,
}

/// 组装报告
///
/// - `game`: 实例设置
/// - `path`: 游戏目录，即 `.minecraft`
/// - `env`: 不在实例设置里的取值
/// - `online`: 实例记录的在线来源，键是文件名
///
/// # 返回值
///
/// 见 [`InstanceReport`]；模组目录读不了时返回错误
fn report(
    game: &InstanceSettingObj,
    path: &Path,
    env: &ReportEnv,
    online: &HashMap<String, OnlineInfoObj>,
) -> Result<InstanceReport, String> {
    let build = crate::build_info::app_build_info();
    let jvm = game.jvm_arg.as_ref();

    Ok(InstanceReport {
        format: FORMAT,
        launcher: LauncherSection {
            version: build.version,
            channel: channel_id(build.channel),
        },
        system: SystemSection {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        },
        locale: LocaleSection {
            ui: env.ui_lang.clone(),
            log_encoding: encoding_id(game.encoding),
        },
        name: game.name.clone(),
        uuid: game.uuid,
        directory: game.dir.clone(),
        minecraft: game.version.clone(),
        game_type: game.game_type.id(),
        loader: game.loader.id(),
        loader_version: game.loader_version.clone(),
        custom_loader: game.loader == LoaderType::Custom,
        modpack: ModpackSection {
            kind: if game.is_modpack {
                game.modpack_type.to_string()
            } else {
                "none".to_string()
            },
            project_id: game.pid.clone(),
            file_id: game.fid.clone(),
        },
        launch: LaunchSection {
            min_memory: jvm.and_then(|jvm| jvm.min_memory),
            max_memory: jvm.and_then(|jvm| jvm.max_memory),
            gc: gc_id(jvm.and_then(|jvm| jvm.gc_mode)),
            jvm_args: lines(jvm.and_then(|jvm| jvm.jvm_args.as_deref())),
            game_args: lines(jvm.and_then(|jvm| jvm.game_args.as_deref())),
            main_class: game
                .advance_jvm
                .as_ref()
                .and_then(|adv| adv.main_class.clone())
                .filter(|text| !text.trim().is_empty()),
            class_path: lines(
                game.advance_jvm
                    .as_ref()
                    .and_then(|adv| adv.class_path.as_deref()),
            ),
            isolate_home: jvm
                .and_then(|jvm| jvm.isolate_home)
                .unwrap_or(env.global_isolate),
            java_name: game.jvm_name.clone().filter(|name| !name.trim().is_empty()),
            window_width: game.window.as_ref().and_then(|window| window.width),
            window_height: game.window.as_ref().and_then(|window| window.height),
            fullscreen: game
                .window
                .as_ref()
                .and_then(|window| window.full_screen)
                .unwrap_or(false),
        },
        mods: mod_entries(path, online)?,
        resource_packs: plain_entries(path, &[names::GAME_RESOURCEPACKS_DIR]),
        shader_packs: plain_entries(path, &[names::GAME_SHADERPACKS_DIR]),
        saves: save_entries(path),
        schematics: plain_entries(path, &[names::GAME_SCHEMATICS_DIR]),
        screenshots: count_files(&path.join(names::GAME_SCREENSHOTS_DIR)),
    })
}

/// 实例记录的在线来源，键是文件名
///
/// - `game`: 实例设置
///
/// # 返回值
///
/// 文件名 → 来源信息；项目号、文件号、下载地址全空的条目不算来源
fn online_of(game: &InstanceSettingObj) -> HashMap<String, OnlineInfoObj> {
    let mut map = HashMap::new();
    for info in game.read_online_info().into_values() {
        if info.modid.is_empty() && info.fileid.is_empty() && info.url.trim().is_empty() {
            continue;
        }
        map.insert(info.file.clone(), info);
    }
    map
}

/// 模组清单，带启用状态与在线来源
///
/// - `base`: 游戏目录
/// - `online`: 在线来源表
///
/// # 返回值
///
/// 见 [`ModEntry`]，按路径排序；模组目录不存在时为空表，读不了时返回错误
fn mod_entries(
    base: &Path,
    online: &HashMap<String, OnlineInfoObj>,
) -> Result<Vec<ModEntry>, String> {
    let dir = base.join(names::GAME_MODS_DIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        // 模组清单是这份报告最核心的一块，顶层读不了就直说
        Err(err) => return Err(err.to_string()),
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect(base, &path, &mut files);
        } else if kind.is_file() {
            push_relative(base, &path, &mut files);
        }
    }
    files.sort();

    Ok(files
        .into_iter()
        .map(|rel| {
            let name = rel
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            let full = base.join(&rel);
            // 停用过的模组文件名多了后缀，按去掉后缀的名字再找一次，
            // 否则在线来源会整条丢掉
            let info = online
                .get(&name)
                .or_else(|| online.get(&strip_disabled(&name)));
            ModEntry {
                path: wire_path(&rel),
                enabled: !is_disabled(&name),
                size: file_len(&full),
                // 现算的哈希才代表磁盘上的这一份；文件读不了时退回在线记录的值
                sha1: gen_hash_from_file(HashType::Sha1, &full).ok().or_else(|| {
                    info.map(|info| info.sha1.trim().to_string())
                        .filter(|hash| !hash.is_empty())
                }),
                project_id: info
                    .map(|info| info.modid.trim().to_string())
                    .filter(|id| !id.is_empty()),
                file_id: info
                    .map(|info| info.fileid.trim().to_string())
                    .filter(|id| !id.is_empty()),
                url: info
                    .map(|info| info.url.trim().to_string())
                    .filter(|url| !url.is_empty()),
            }
        })
        .collect())
}

/// 普通文件清单，不算哈希
///
/// - `base`: 游戏目录
/// - `dirs`: 要列的目录名，相对游戏目录
///
/// # 返回值
///
/// 见 [`FileEntry`]，按路径排序
fn plain_entries(base: &Path, dirs: &[&str]) -> Vec<FileEntry> {
    let mut files = Vec::new();
    for dir in dirs {
        let path = base.join(dir);
        if path.is_dir() {
            collect(base, &path, &mut files);
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|rel| FileEntry {
            path: wire_path(&rel),
            size: file_len(&base.join(&rel)),
            sha1: None,
        })
        .collect()
}

/// 存档清单：目录名、总大小与其中的数据包
///
/// - `base`: 游戏目录
///
/// # 返回值
///
/// 见 [`SaveEntry`]，按目录名排序；存档目录不存在时为空表
fn save_entries(base: &Path) -> Vec<SaveEntry> {
    let dir = base.join(names::GAME_SAVES_DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut list: Vec<SaveEntry> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| {
            let path = entry.path();
            let directory = entry.file_name().to_string_lossy().to_string();
            let mut data_packs = Vec::new();
            if let Ok(packs) = fs::read_dir(path.join(names::GAME_DATAPACK_DIR)) {
                data_packs = packs
                    .flatten()
                    .map(|pack| pack.file_name().to_string_lossy().to_string())
                    .collect();
            }
            data_packs.sort();
            SaveEntry {
                directory,
                size: dir_len(&path),
                data_packs,
            }
        })
        .collect();
    list.sort_by(|a, b| a.directory.cmp(&b.directory));
    list
}

/// 递归收集目录下的文件，路径相对 `base`
///
/// 链接跳过、子目录读不到就忽略：报告能出来比整份失败有用。
///
/// - `base`: 游戏目录
/// - `dir`: 待遍历的目录
/// - `out`: 收集结果
fn collect(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect(base, &path, out);
        } else if kind.is_file() {
            push_relative(base, &path, out);
        }
    }
}

/// 把绝对路径转成相对游戏目录的路径收进去，不在 `base` 里的忽略
///
/// - `base`: 游戏目录
/// - `path`: 文件路径
/// - `out`: 收集结果
fn push_relative(base: &Path, path: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rel) = path.strip_prefix(base) {
        out.push(rel.to_path_buf());
    }
}

/// 文件大小，单位字节；读不到按 0
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 字节数
fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

/// 递归累加目录里所有文件的大小
///
/// - `dir`: 目录
///
/// # 返回值
///
/// 字节数；链接不跟进
fn dir_len(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            total += dir_len(&entry.path());
        } else if kind.is_file() {
            total += file_len(&entry.path());
        }
    }
    total
}

/// 数一个目录下有多少个文件
///
/// - `dir`: 目录
///
/// # 返回值
///
/// 文件数；目录不存在时是 0
fn count_files(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .count()
}

/// 去掉停用后缀，拿到停用前的文件名
///
/// - `name`: 文件名
///
/// # 返回值
///
/// 不带 `.disable` / `.disabled` 后缀的名字；本来就没后缀时原样返回
fn strip_disabled(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for suffix in [names::DISABLED_DOT_EXT, names::DISABLE_DOT_EXT] {
        if lower.ends_with(suffix) {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    name.to_string()
}

/// 文件名是不是停用状态
///
/// - `name`: 文件名
///
/// # 返回值
///
/// 带 `.disable` / `.disabled` 后缀返回 `true`
fn is_disabled(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(names::DISABLE_DOT_EXT) || name.ends_with(names::DISABLED_DOT_EXT)
}

/// 相对路径转清单里的正斜杠写法
///
/// - `rel`: 相对路径
///
/// # 返回值
///
/// 形如 `mods/foo.jar`
fn wire_path(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// 多行参数拆成一行一条
///
/// - `text`: 用户填的原始参数，一行一条
///
/// # 返回值
///
/// 去掉空行后的参数表
fn lines(text: Option<&str>) -> Vec<String> {
    text.map(|text| {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// GC 模式的独立 ID，与启动参数界面同一套取值
///
/// - `gc`: 实例设置里的 GC 模式
///
/// # 返回值
///
/// `auto` / `g1gc` / `zgc` / `none`
fn gc_id(gc: Option<GCType>) -> &'static str {
    match gc.unwrap_or(GCType::Auto) {
        GCType::Auto => "auto",
        GCType::G1GC => "g1gc",
        GCType::ZGC => "zgc",
        GCType::None => "none",
    }
}

/// 日志编码的独立 ID
///
/// - `encoding`: 实例设置里的日志编码
///
/// # 返回值
///
/// `utf8` 或 `gbk`
fn encoding_id(encoding: LogEncoding) -> &'static str {
    match encoding {
        LogEncoding::GBK => "gbk",
        LogEncoding::UTF8 => "utf8",
    }
}

/// 构建通道的独立 ID
///
/// - `channel`: 编译期定下的构建通道
///
/// # 返回值
///
/// `release` / `prerelease` / `debug`
fn channel_id(channel: crate::dtos::build_info_dto::BuildChannel) -> &'static str {
    use crate::dtos::build_info_dto::BuildChannel;
    match channel {
        BuildChannel::Release => "release",
        BuildChannel::Prerelease => "prerelease",
        BuildChannel::Debug => "debug",
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_export_info_tests.rs"]
mod tests;
