//! `.minecraft` 探测：候选目录、可导入版本、"问过了"标记

use std::fs;
use std::path::{Path, PathBuf};

use mml_base::{get_base_dir, serialize_tools};
use mml_game::other_launcher::official_obj::OfficialObj;
use mml_names::names;

use crate::dtos::{MinecraftInfoDto, MinecraftVersionDto};

/// 不搬的顶层条目：启动器自己的东西，M²L 会重新下载或不需要
///
/// 后几条是 HMCL / PCL 的：PCL 会在 `.minecraft` 里放 `PCL` 目录与 `LatestLaunch.bat`，
/// HMCL 的配置叫 `hmcl.json`；这些是启动器数据，不该进实例。
const SKIP_ENTRIES: &[&str] = &[
    "versions",
    "libraries",
    "assets",
    "logs",
    "crash-reports",
    "launcher_profiles.json",
    "launcher_accounts.json",
    "launcher_msa_credentials.bin",
    "launcher_settings.json",
    "usercache.json",
    ".cache",
    "hmcl.json",
    ".hmcl.json",
    "PCL",
    "PCL.ini",
    "LatestLaunch.bat",
];

/// 标记文件：运行目录下的 `minecraft_migrate.json`
///
/// # 返回值
///
/// 标记文件路径
pub(crate) fn marker_file() -> PathBuf {
    get_base_dir().join(names::MINECRAFT_MIGRATE_FILE)
}

/// 是否已经问过，标记文件存在即算问过
///
/// # 返回值
///
/// 标记文件存在即为 `true`
pub(crate) fn asked() -> bool {
    marker_file().is_file()
}

/// 记下"已问过"以及当时的选择
///
/// - `choice`: 当时的选择，取 `import` / `skip`
/// - `version`: 导入的版本 id；跳过时为空串
///
/// # 返回值
///
/// 写成功返回 `Ok(())`；写盘失败返回 `Err`，内容为错误原文
pub(crate) fn mark(choice: &str, version: &str) -> Result<(), String> {
    let obj = MigrateMarker {
        choice: choice.to_string(),
        version: version.to_string(),
        time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    serialize_tools::json_to_file(&obj, marker_file()).map_err(|err| err.to_string())
}

/// `.minecraft` 的"问过了"标记，见 [`mark`]
#[derive(serde::Serialize)]
struct MigrateMarker {
    /// 用户的选择，取 `import` / `skip`
    #[serde(rename = "Choice")]
    choice: String,
    /// 导入的版本 id；跳过时为空串
    #[serde(rename = "Version")]
    version: String,
    /// 记录时间，格式 `%Y-%m-%d %H:%M:%S`，仅用于排查
    #[serde(rename = "Time")]
    time: String,
}

/// 按可信度排列的候选目录
///
/// Windows 上官方启动器与绝大多数第三方启动器都用 `%APPDATA%\.minecraft`；
/// 微软商店版在 `Packages` 下的私有目录里，单独补一条。
///
/// # 返回值
///
/// 候选目录，不检查是否存在
fn candidates() -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Some(dir) = dirs::config_dir() {
            list.push(dir.join(names::GAME_DIR));
        }
        if let Some(dir) = dirs::data_local_dir() {
            list.push(
                dir.join("Packages")
                    .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                    .join("LocalCache")
                    .join("Local")
                    .join(names::GAME_DIR),
            );
        }
    }
    #[cfg(target_os = "linux")]
    if let Some(dir) = dirs::home_dir() {
        list.push(dir.join(names::GAME_DIR));
    }
    #[cfg(target_os = "macos")]
    if let Some(dir) = dirs::home_dir() {
        list.push(
            dir.join("Library")
                .join("Application Support")
                .join(names::GAME_DIR.trim_start_matches('.')),
        );
    }

    list
}

/// 探测本机的 `.minecraft`，取第一个"目录存在且有版本"的候选
///
/// # 返回值
///
/// 命中的目录；一个都没命中返回 `None`
pub(crate) fn detect() -> Option<PathBuf> {
    candidates()
        .into_iter()
        .find(|dir| dir.is_dir() && !versions(dir).is_empty())
}

/// 组装探测结果
///
/// - `root`: `.minecraft` 目录
///
/// # 返回值
///
/// 见 [`MinecraftInfoDto`]
pub(crate) fn info(root: PathBuf) -> MinecraftInfoDto {
    MinecraftInfoDto {
        path: root.display().to_string(),
        versions: versions(&root),
    }
}

/// 列出版本，按版本 id 排序
///
/// 版本目录里没有同名 `.json` 的不算：那种目录启动器自己都启动不了。
///
/// - `root`: `.minecraft` 目录
///
/// # 返回值
///
/// 可导入的版本；`versions` 目录读不到时为空
pub(crate) fn versions(root: &Path) -> Vec<MinecraftVersionDto> {
    let Ok(reader) = fs::read_dir(root.join(names::VERSION_DIR)) else {
        return Vec::new();
    };
    let mut list: Vec<MinecraftVersionDto> = reader
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().to_string();
            let json = entry.path().join(format!("{id}{}", names::JSON_DOT_EXT));
            if !json.is_file() {
                return None;
            }
            let (known, loader) = probe(&json);
            Some(MinecraftVersionDto { id, known, loader })
        })
        .collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    list
}

/// 读版本 json，判断 M²L 认不认识这个版本、用的是哪个加载器
///
/// - `json`: 版本 json
///
/// # 返回值
///
/// `(是否收录, 加载器 id)`；解析失败按未收录的自定义版本处理
fn probe(json: &Path) -> (bool, String) {
    let Ok(obj) = OfficialObj::read_from_file(json) else {
        return (false, "custom".to_string());
    };
    let instance = obj.to_instance();
    // to_instance 只在版本号被 M²L 收录时才填 version，空的即自定义版本
    if instance.version.is_empty() {
        return (false, "custom".to_string());
    }
    (true, instance.loader.id().to_string())
}

/// 这个版本的游戏数据放在哪、要搬哪些条目
///
/// HMCL / PCL 默认开"版本隔离"：`mods`、`saves`、`config` 都在 `versions/<id>/` 里，
/// 根目录只剩 `versions` / `libraries` / `assets`。版本目录里有游戏数据就用它，
/// 否则按官方启动器的布局搬根目录。
///
/// - `root`: `.minecraft` 目录
/// - `version`: 版本 id
///
/// # 返回值
///
/// `(游戏数据目录, 要搬的条目)`
pub(crate) fn game_data_root(root: &Path, version: &str) -> (PathBuf, Vec<PathBuf>) {
    let dir = root.join(names::VERSION_DIR).join(version);
    if dir.is_dir() {
        let entries = version_entries(&dir, version);
        if !entries.is_empty() {
            return (dir, entries);
        }
    }
    (root.to_path_buf(), game_entries(root))
}

/// 版本目录里的游戏数据条目，已排序
///
/// 版本 json 与客户端 jar 是启动文件，不算游戏数据。
///
/// - `dir`: `versions/<id>` 目录
/// - `version`: 版本 id
///
/// # 返回值
///
/// 条目的绝对路径，不含 [`SKIP_ENTRIES`] 与 `<id>.json` / `<id>.jar`
pub(crate) fn version_entries(dir: &Path, version: &str) -> Vec<PathBuf> {
    let json = format!("{version}{}", names::JSON_DOT_EXT);
    let jar = format!("{version}{}", names::JAR_DOT_EXT);
    let Ok(reader) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut list: Vec<PathBuf> = reader
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            !is_skipped(&name)
                && !name.eq_ignore_ascii_case(&json)
                && !name.eq_ignore_ascii_case(&jar)
        })
        .collect();
    list.sort();
    list
}

/// 名字是不是"启动器自己的东西"
///
/// - `name`: 条目名
///
/// # 返回值
///
/// 命中 [`SKIP_ENTRIES`] 时为 `true`
fn is_skipped(name: &str) -> bool {
    SKIP_ENTRIES
        .iter()
        .any(|skip| skip.eq_ignore_ascii_case(name))
}

/// 会被搬走的顶层条目，已排序
///
/// - `root`: `.minecraft` 目录
///
/// # 返回值
///
/// 顶层条目的绝对路径，不含 [`SKIP_ENTRIES`] 里的名字
pub(crate) fn game_entries(root: &Path) -> Vec<PathBuf> {
    let Ok(reader) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut list: Vec<PathBuf> = reader
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            !is_skipped(&name)
        })
        .collect();
    list.sort();
    list
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 官方启动器布局：游戏数据在根目录
    #[test]
    fn game_data_root_uses_root_when_not_isolated() {
        let dir =
            mml_testutil::temp_path(format!("minecraft-detect-root-{}", uuid::Uuid::new_v4()));
        let version = dir.join(names::VERSION_DIR).join("1.21.4");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join("1.21.4.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.join("mods")).unwrap();

        let (root, entries) = game_data_root(&dir, "1.21.4");
        assert_eq!(root, dir);
        assert_eq!(entries, vec![dir.join("mods")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// HMCL / PCL 的版本隔离：游戏数据在 versions/<id>/ 里，版本 json 与 jar 不算
    #[test]
    fn game_data_root_uses_version_dir_when_isolated() {
        let dir = mml_testutil::temp_path(format!("minecraft-detect-iso-{}", uuid::Uuid::new_v4()));
        let version = dir.join(names::VERSION_DIR).join("1.20.1");
        std::fs::create_dir_all(version.join("mods")).unwrap();
        std::fs::create_dir_all(version.join("PCL")).unwrap();
        std::fs::write(version.join("1.20.1.json"), "{}").unwrap();
        std::fs::write(version.join("1.20.1.jar"), "").unwrap();

        let (root, entries) = game_data_root(&dir, "1.20.1");
        assert_eq!(root, version);
        assert_eq!(entries, vec![version.join("mods")]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
