//! 服务器实例的同步选项与配置文件快照
//!
//! 这里只放数据口径与读盘工具：同步方式、配置目录、自定义条目，以及
//! "按目录与方式收集文件 + 算 SHA1" 这套生成与对比共用的逻辑。
//! 清单拼装与写盘在 `generate.rs`，IPC 命令留在 `mod.rs`。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::err_keys;

use super::files;

/// 配置文件的同步方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigMode {
    /// 整个子树的文件打成一个压缩包，客户端解压前先删旧目录
    ArchiveAll,
    /// 只把本目录的直接文件打成一个压缩包，子目录由各自的条目负责
    ArchiveSelf,
    /// 整个子树的文件逐个列进清单，客户端按原路径下载
    FilesAll,
    /// 只把本目录的直接文件逐个列进清单
    FilesSelf,
    /// 发一条 delete 清单项，让客户端删掉这个目录
    Delete,
    /// 这个子目录不参与同步：父目录整包 / 整目录时把它排出去，自己不产出条目
    None,
}

impl ConfigMode {
    /// 按前端给的值解析同步方式
    ///
    /// `archive` / `files` 是旧版的值，按整包 / 整目录处理。
    ///
    /// - `value`: `archiveAll` / `archiveSelf` / `filesAll` / `filesSelf` / `delete`
    ///
    /// # 返回值
    ///
    /// 不认识时返回 `err.serverInstanceMode`
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "archive" | "archiveAll" => Ok(Self::ArchiveAll),
            "none" => Ok(Self::None),
            "archiveSelf" => Ok(Self::ArchiveSelf),
            "files" | "filesAll" => Ok(Self::FilesAll),
            "filesSelf" => Ok(Self::FilesSelf),
            "delete" => Ok(Self::Delete),
            _ => Err(err_keys::SERVER_INSTANCE_MODE.to_string()),
        }
    }

    /// 是否走 Archives：打成压缩包，或发 delete 条目
    pub(super) fn is_archive(self) -> bool {
        matches!(self, Self::ArchiveAll | Self::ArchiveSelf | Self::Delete)
    }

    /// 是否先把玩家原有目录删掉
    pub(super) fn deletes_old(self) -> bool {
        matches!(self, Self::ArchiveAll | Self::Delete)
    }

    /// 是否递归覆盖子目录
    pub(super) fn recursive(self) -> bool {
        matches!(self, Self::ArchiveAll | Self::FilesAll | Self::Delete)
    }
}

/// 自定义条目的动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CustomAction {
    /// 纳入同步，当普通文件托管
    Add,
    /// 删除客户端文件
    Delete,
}

/// 一条自定义条目
pub(super) struct CustomPick {
    /// 相对游戏目录的路径
    pub(super) path: PathBuf,
    /// 动作
    pub(super) action: CustomAction,
    /// 添加时直接写成客户端文件的内容；`None` 表示从实例里复制现有文件
    pub(super) content: Option<String>,
}

/// 一个要同步的配置目录
pub(super) struct ConfigDir {
    /// 目录名，相对游戏目录，如 `config`、`config/setting`
    pub(super) dir: String,
    /// 同步方式
    pub(super) mode: ConfigMode,
    /// 取消勾选的文件，相对游戏目录
    pub(super) exclude: Vec<String>,
    /// 要删除的文件，相对游戏目录
    pub(super) delete: Vec<String>,
    /// 压缩包解压前先删掉客户端旧目录
    pub(super) delete_old: bool,
}

/// 本次要写进包里的内容
pub(super) struct Contents {
    /// 勾选的模组，带手填的下载信息
    pub(super) mods: Vec<files::ModPick>,
    /// 配置目录与各自的同步方式
    pub(super) configs: Vec<ConfigDir>,
    /// 资源包文件，带手填的编号与地址
    pub(super) packs: Vec<PackPick>,
    /// 自定义条目：额外纳入同步或删除的文件
    pub(super) custom: Vec<CustomPick>,
}

/// 一个要同步的资源包
pub(super) struct PackPick {
    /// 相对游戏目录的路径
    pub(super) path: PathBuf,
    /// 手动填的下载地址
    pub(super) url: String,
    /// 手动填的项目编号
    pub(super) pid: String,
    /// 手动填的文件编号
    pub(super) fid: String,
    /// 是否使用自定义编号
    pub(super) custom: bool,
    /// 编号是否被用户改过
    pub(super) edited: bool,
    /// 编号类型是否被用户改过
    pub(super) type_changed: bool,
}

/// 校验并规范化配置目录
///
/// 允许任意层级的子目录，如 `config` / `config/setting/sub`。
///
/// - `value`: 前端给的目录名
///
/// # 返回值
///
/// 统一用 `/` 分隔的目录名；为空或含空段 / `.` / `..` 时返回
/// `err.serverInstanceDir`
pub(super) fn config_dir(value: &str) -> Result<String, String> {
    let dir = value.trim().replace('\\', "/");
    let parts: Vec<&str> = dir.split('/').collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(err_keys::SERVER_INSTANCE_DIR.to_string());
    }
    Ok(dir)
}

/// 参与同步的配置目录下的文件内容快照
///
/// 路径相对游戏目录（正斜杠）；父目录与子目录重叠时同一路径只算一次。
/// 不同步与删除目录不参与，避免把没同步的文件算进对比。
///
/// - `base`: 游戏目录
/// - `configs`: 配置目录选项
///
/// # 返回值
///
/// 路径 → SHA1；读目录或算哈希失败时返回错误文案
pub(super) fn config_snapshot(base: &Path, configs: &[ConfigDir]) -> Result<BTreeMap<String, String>, String> {
    collect_config_files(base, configs.iter().map(|item| (item.dir.as_str(), item.mode)), true)
}

/// 按目录与同步方式收集文件内容，供生成快照与打开时的对比共用
///
/// - `base`: 游戏目录
/// - `dirs`: 目录与同步方式
/// - `strict`: `true` 时目录读不了 / 算哈希失败直接报错；`false` 时跳过
///
/// # 返回值
///
/// 路径 → SHA1；`strict` 为 `true` 时失败返回错误文案
pub(super) fn collect_config_files<'a>(
    base: &Path,
    dirs: impl Iterator<Item = (&'a str, ConfigMode)>,
    strict: bool,
) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for (dir, mode) in dirs {
        let list = match mode {
            ConfigMode::ArchiveAll | ConfigMode::FilesAll => files::dir_files(base, dir),
            ConfigMode::ArchiveSelf | ConfigMode::FilesSelf => files::dir_direct_files(base, dir),
            ConfigMode::None | ConfigMode::Delete => continue,
        };
        let list = match list {
            Ok(list) => list,
            Err(error) if strict => return Err(error),
            Err(_) => continue,
        };
        for rel in list {
            let wire = files::wire_path(&rel);
            if map.contains_key(&wire) {
                continue;
            }
            match files::sha1(&base.join(&rel)) {
                Ok(sha1) => {
                    map.insert(wire, sha1);
                }
                Err(error) if strict => return Err(error),
                Err(_) => {}
            }
        }
    }
    Ok(map)
}

/// 收集要删除的文件
///
/// 每条路径只接受它所在条目负责的范围，前端传错路径时不误删别处；重复标记只留一份。
/// 文件不要求还在：发布者可能先删了实例里的文件再来标记。
///
/// - `base`: 游戏目录
/// - `configs`: 配置目录选项
///
/// # 返回值
///
/// 去重排序后的相对路径；路径非法、指向目录或在条目范围外时返回错误文案
pub(super) fn config_deletions(base: &Path, configs: &[ConfigDir]) -> Result<Vec<PathBuf>, String> {
    let mut list = BTreeSet::new();
    for config in configs {
        // 不参与同步 / 整目录删除的条目不单独删文件，前者不该动客户端，后者已经删掉了
        if matches!(config.mode, ConfigMode::None | ConfigMode::Delete) {
            continue;
        }
        let dir = Path::new(&config.dir);
        for path in &config.delete {
            let rel = files::custom_target(base, path)?;
            if rel == dir || !rel.starts_with(dir) {
                return Err(err_keys::SERVER_INSTANCE_DIR.to_string());
            }
            list.insert(rel);
        }
    }
    Ok(list.into_iter().collect())
}

/// 校验发布地址并去掉末尾的斜杠
///
/// - `value`: 前端填的地址
///
/// # 返回值
///
/// 规范化后的地址，拼托管文件时直接用；不是 http(s) 地址时返回
/// `err.serverInstanceUrl`
pub(super) fn normalize_url(value: &str) -> Result<String, String> {
    let url = value.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(rest) if !rest.is_empty() => Ok(url.to_string()),
        _ => Err(err_keys::SERVER_INSTANCE_URL.to_string()),
    }
}

/// 准备输出文件夹，并确认它不在实例目录里
///
/// 写进实例目录会把刚生成的文件当成下一次的同步内容，升级流程也会把它们删掉。
///
/// - `instance`: 实例根目录
/// - `value`: 前端选的文件夹
///
/// # 返回值
///
/// 规范化后的文件夹；为空、建不出来或落在实例里时返回 `err.serverInstanceFolder`
pub(super) fn prepare_folder(instance: &Path, value: &str) -> Result<PathBuf, String> {
    let folder = value.trim();
    if folder.is_empty() {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }
    let path = PathBuf::from(folder);
    fs::create_dir_all(&path).map_err(|_| err_keys::SERVER_INSTANCE_FOLDER.to_string())?;
    let path = path
        .canonicalize()
        .map_err(|_| err_keys::SERVER_INSTANCE_FOLDER.to_string())?;
    if !path.is_dir() {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }

    if let Ok(instance) = instance.canonicalize()
        && path.starts_with(&instance)
    {
        return Err(err_keys::SERVER_INSTANCE_FOLDER.to_string());
    }
    Ok(path)
}
