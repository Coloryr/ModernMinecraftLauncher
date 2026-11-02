//! Mojang 版本清单与启动数据
//!
//! 子模块:
//!
//! | 模块 | 职责 |
//! | --- | --- |
//! | `assets_obj` | 资源索引 DTO |
//! | `game_arg_obj` | 版本启动参数 DTO |
//! | `version_checker` | 版本清单检查 |
//! | `version_obj` | 版本清单 DTO |

use mml_base::file_item::{FileHash, FileItemObj};
use mml_net::url_helper;
use mml_sys::{ArchEnum, Os, SystemInfo};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::{
    launcher_path::{assets_path, libraries_path, version_path},
    mojang::game_arg_obj::{GameArgObj, GameOsObj, GameRulesObj, LoggingObj},
};

pub mod assets_obj;
pub mod game_arg_obj;
pub mod version_checker;
pub mod version_obj;

/// 游戏版本类型，按变体顺序序列化为 `u8`
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VersionType {
    /// 发布版
    Release,
    /// 测试版
    Snapshot,
    /// 其他
    Other,
    /// 全部
    All,
}

impl Default for VersionType {
    fn default() -> Self {
        VersionType::Release
    }
}

impl VersionType {
    /// 独立 ID，跨进程传输用，显示名由前端 i18n 翻译
    ///
    /// # 返回值
    ///
    /// 返回该类型的 ID 字符串
    pub fn id(&self) -> &'static str {
        match self {
            VersionType::Release => "release",
            VersionType::Snapshot => "snapshot",
            VersionType::Other => "other",
            VersionType::All => "all",
        }
    }

    /// 按 ID 解析版本类型
    ///
    /// - `id`: 版本 ID，形如 `snapshot`
    ///
    /// # 返回值
    ///
    /// 未知 ID 返回 `Release`
    pub fn from_id(id: &str) -> Self {
        match id {
            "snapshot" => VersionType::Snapshot,
            "other" => VersionType::Other,
            "all" => VersionType::All,
            _ => VersionType::Release,
        }
    }
}

/// 规则里的系统条件是否匹配当前系统
///
/// 系统名可以带架构后缀，GTNH 的 lwjgl3 patch 就写成 `osx-arm64` / `windows-arm64`。
/// 后缀参与判断：`osx-arm64` 只在 Apple Silicon 上匹配，`osx` 两种架构都匹配。
///
/// - `os`: 规则里的系统条件
/// - `sys`: 当前系统信息
///
/// # 返回值
///
/// 系统名与架构都匹配时返回 `true`；不认识的系统名或架构返回 `false`
fn rule_os_matches(os: &GameOsObj, sys: &SystemInfo) -> bool {
    let (name, arch) = os.name.split_once('-').unwrap_or((os.name.as_str(), ""));
    let os_ok = match name {
        "" => true,
        "osx" => sys.os == Os::MacOS,
        "windows" => sys.os == Os::Windows,
        "linux" => sys.os == Os::Linux,
        "freebsd" => cfg!(target_os = "freebsd"),
        _ => false,
    };
    if !os_ok {
        return false;
    }

    if !arch.is_empty() {
        return match arch {
            "arm64" | "aarch64" => sys.system_arch == ArchEnum::AArch64,
            "arm32" | "arm" => sys.system_arch == ArchEnum::Arm,
            "x86_64" | "x64" | "amd64" | "x86" => !sys.is_arm,
            _ => false,
        };
    }

    // 老规则把架构写在 os.arch 里，x86 一直按"非 ARM"口径
    os.arch.is_empty() || (os.arch == "x86" && !sys.is_arm)
}

/// 检查规则是否适用
///
/// 多条 allow 规则按"任一命中即放行"处理，GTNH 的 `osx` + `osx-arm64` 就是这种写法；
/// 命中的 disallow 规则可以否决。只有 disallow 规则时默认放行。
///
/// - `list`: 规则列表
///
/// # 返回值
///
/// 返回规则是否允许当前系统
pub fn check_allow(list: &Vec<GameRulesObj>) -> bool {
    let sys = mml_sys::get_system_info();
    let mut allow = true;
    let mut allow_seen = false;
    let mut allow_matched = false;

    for item in list.iter() {
        let matched = match &item.os {
            Some(os) => rule_os_matches(os, &sys),
            None => true,
        };
        match item.action.as_str() {
            "allow" => {
                allow_seen = true;
                if matched {
                    allow_matched = true;
                    allow = true;
                }
            }
            "disallow" => {
                if matched {
                    allow = false;
                }
            }
            _ => {}
        }
    }

    // 有 allow 规则却一条都没命中，说明当前系统不在允许范围内
    if allow_seen && !allow_matched {
        return false;
    }
    allow
}

/// 安全Log4j文件
///
/// - `obj`: 游戏数据
///
/// # 返回值
///
/// 返回 log4j2 配置文件下载项
pub fn build_log4j_item(obj: &LoggingObj) -> FileItemObj {
    FileItemObj {
        name: String::from("log4j2-xml"),
        file: version_path::get_version_dir()
            .join("log4j2")
            .join("log4j2.xml"),
        url: obj.client.file.url.clone(),
        hash: FileHash::Sha1(obj.client.file.sha1.clone()),
        later: Default::default(),
    }
}

/// 创建游戏资源下载项目
///
/// - `name`: 资源名
/// - `hash`: 资源 sha1 校验值
///
/// # 返回值
///
/// 返回资源文件下载项
pub fn build_assets_item(name: &str, hash: &str) -> FileItemObj {
    let dir: String = hash.chars().take(2).collect();
    FileItemObj {
        name: String::from(name),
        file: assets_path::get_obj_dir().join(dir).join(hash),
        url: url_helper::get_download_assets(hash),
        hash: FileHash::Sha1(String::from(hash)),
        later: Default::default(),
    }
}

/// 创建游戏本体下载项目
///
/// - `version`: 游戏版本号
///
/// # 返回值
///
/// 返回游戏本体下载项，版本不存在时 panic
pub fn build_game_item(version: &str) -> FileItemObj {
    version_path::get_version(version)
        .unwrap()
        .build_game_item()
}

impl GameArgObj {
    /// 创建游戏本体下载项目
    ///
    /// # 返回值
    ///
    /// 返回游戏本体下载项
    pub fn build_game_item(&self) -> FileItemObj {
        FileItemObj {
            name: format!("minecraft-clinet-{}.jar", self.id),
            file: libraries_path::get_game_file(&self.id),
            url: url_helper::get_minecraft_client(&self.downloads.client.url, &self.id),
            hash: FileHash::Sha1(self.downloads.client.sha1.clone()),
            later: Default::default(),
        }
    }
}
