//! 配置数据结构定义
//!
//! 本模块定义了启动器配置文件中所有可配置项的 Rust 数据结构。
//! 这些结构体通过 `serde` 序列化为 JSON 格式存储到 `config.json` 中。
//!
//! # 主要配置项
//!
//! | 结构体 | 用途 |
//! |--------|------|
//! | [`ConfigObj`] | 顶层配置文件 |
//! | [`HttpObj`] | 网络/代理设置 |
//! | [`DnsObj`] | 自定义 DNS 设置 |
//! | [`RunArgObj`] | JVM 启动参数 |
//! | [`WindowSettingObj`] | 游戏窗口设置 |
//! | [`GameCheckObj`] | 游戏文件校验设置 |
//! | [`JvmConfigObj`] | Java 运行时配置 |

use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// 把 JSON 里的 `null` 当成默认值
///
/// ColorMC 的 `config.json` 把代理用户名 / 密码写成 `null`，而 M²L 这两个字段是
/// `String`：迁移过来的配置必须还能读，否则核心初始化失败、启动器起不来。
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Java 虚拟机配置
///
/// 记录一个已添加的 Java 运行时的名称和路径。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct JvmConfigObj {
    /// Java 显示名称，对应 JSON 键 `Name`，默认空字符串
    #[serde(rename = "Name")]
    pub name: String,
    /// Java 可执行文件路径，可为相对或绝对路径；对应 JSON 键 `Local`，默认空字符串
    #[serde(rename = "Local")]
    pub local: String,
}

impl Default for JvmConfigObj {
    fn default() -> Self {
        Self {
            name: String::new(),
            local: String::new(),
        }
    }
}

/// 下载源选择
///
/// 决定从哪个镜像源下载 Minecraft 相关资源。
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceLocal {
    /// Mojang 官方下载源，序列化为 0，默认值
    Offical,
    /// BMCLAPI 国内镜像源，由 bangbang93 维护，序列化为 1
    Bmclapi,
}

impl Default for SourceLocal {
    fn default() -> Self {
        SourceLocal::Offical
    }
}

/// 代理使用策略
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProxyState {
    /// 自动检测系统代理设置，序列化为 0，默认值
    Auto,
    /// 不使用代理，序列化为 1
    None,
    /// 使用用户自定义代理，序列化为 2
    User,
}

impl Default for ProxyState {
    fn default() -> Self {
        ProxyState::Auto
    }
}

/// 代理类型
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProxyType {
    /// HTTP 代理，序列化为 0，默认值
    Http,
    /// SOCKS4 代理，序列化为 1
    Sock4,
    /// SOCKS5 代理，序列化为 2
    Sock5,
}

impl Default for ProxyType {
    fn default() -> Self {
        ProxyType::Http
    }
}

/// 启动器网络配置
///
/// 控制下载行为、代理设置和文件校验策略。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct HttpObj {
    /// 资源下载源，对应 JSON 键 `Source`，默认 `Offical`
    #[serde(rename = "Source")]
    pub source: SourceLocal,
    /// 下载并发线程数，对应 JSON 键 `DownloadThread`，默认 5
    #[serde(rename = "DownloadThread")]
    pub download_thread: u32,
    /// 代理服务器 IP 地址，对应 JSON 键 `ProxyIP`，默认 `127.0.0.1`
    #[serde(rename = "ProxyIP")]
    pub proxy_ip: String,
    /// 代理服务器端口，对应 JSON 键 `ProxyPort`，默认 7890
    #[serde(rename = "ProxyPort")]
    pub proxy_port: u16,
    /// 代理认证用户名，对应 JSON 键 `ProxyUser`，默认空字符串；ColorMC 写的是 `null`
    #[serde(rename = "ProxyUser", deserialize_with = "null_as_default")]
    pub proxy_user: String,
    /// 代理认证密码，对应 JSON 键 `ProxyPassword`，默认空字符串；ColorMC 写的是 `null`
    #[serde(rename = "ProxyPassword", deserialize_with = "null_as_default")]
    pub proxy_password: String,

    /// 一般请求的代理策略，对应 JSON 键 `ProxyWork`，默认 `Auto`
    #[serde(rename = "ProxyWork")]
    pub work_proxy: ProxyState,
    /// 一般请求的代理类型，对应 JSON 键 `ProxyWorkType`，默认 `Http`
    #[serde(rename = "ProxyWorkType")]
    pub work_proxy_type: ProxyType,

    /// 登录请求的代理策略，对应 JSON 键 `ProxyLogin`，默认 `Auto`
    #[serde(rename = "ProxyLogin")]
    pub login_proxy: ProxyState,
    /// 登录请求的代理类型，对应 JSON 键 `ProxyLoginType`，默认 `Http`
    #[serde(rename = "ProxyLoginType")]
    pub login_proxy_type: ProxyType,

    /// 是否校验下载文件完整性，校验算法为 SHA1；对应 JSON 键 `CheckFile`，默认 true
    #[serde(rename = "CheckFile")]
    pub check_file: bool,
    /// 是否自动下载缺失文件，对应 JSON 键 `AutoDownload`，默认 true
    #[serde(rename = "AutoDownload")]
    pub auto_download: bool,
}

impl Default for HttpObj {
    fn default() -> Self {
        Self {
            source: SourceLocal::Offical,
            download_thread: 10,
            proxy_ip: String::from("127.0.0.1"),
            proxy_port: 7890,
            proxy_user: String::new(),
            proxy_password: String::new(),
            check_file: true,
            auto_download: true,
            work_proxy: ProxyState::Auto,
            work_proxy_type: ProxyType::Http,
            login_proxy: ProxyState::Auto,
            login_proxy_type: ProxyType::Http,
        }
    }
}

/// 自定义 DNS 设置
///
/// 支持 DNS over HTTPS，即 DoH，用于绕过 DNS 污染。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct DnsObj {
    /// 是否启用自定义 DNS，对应 JSON 键 `Enable`，默认 false
    #[serde(rename = "Enable")]
    pub enable: bool,
    /// DNS over HTTPS 服务器地址列表，对应 JSON 键 `Https`，默认空列表
    #[serde(rename = "Https")]
    pub https: Vec<String>,
    /// 是否对代理连接也启用自定义 DNS，对应 JSON 键 `HttpProxy`，默认 false
    #[serde(rename = "HttpProxy")]
    pub http_proxy: bool,
}

impl Default for DnsObj {
    fn default() -> Self {
        Self {
            enable: false,
            https: Vec::new(),
            http_proxy: false,
        }
    }
}

/// JVM 垃圾回收器类型
#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GCType {
    /// 自动选择，按 Java 版本匹配合适的 GC；序列化为 0，默认值
    Auto,
    /// G1 垃圾回收器，适合大内存、低延迟场景；序列化为 1
    G1GC,
    /// 分代式 Z Garbage Collector，Java 21+ 推荐；序列化为 2
    ZGC,
    /// 不添加任何 GC 参数，使用 JVM 默认；序列化为 3
    None,
}

impl Default for GCType {
    fn default() -> Self {
        GCType::Auto
    }
}

/// 游戏启动参数配置
///
/// 控制 JVM 参数、游戏参数、内存分配、启动前后执行命令等。
/// 所有字段均为 `Option` 类型，`None` 表示使用全局默认值。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct RunArgObj {
    /// 是否移除原有的 JVM 参数，对应 JSON 键 `RemoveJvmArg`，缺省为 `None`
    #[serde(rename = "RemoveJvmArg")]
    pub remove_jvm_arg: Option<bool>,
    /// 是否移除原有的游戏参数，对应 JSON 键 `RemoveGameArg`，缺省为 `None`
    #[serde(rename = "RemoveGameArg")]
    pub remove_game_arg: Option<bool>,
    /// 自定义 JVM 参数，追加或替换；对应 JSON 键 `JvmArgs`，缺省为 `None`
    #[serde(rename = "JvmArgs")]
    pub jvm_args: Option<String>,
    /// 自定义游戏参数，追加或替换；对应 JSON 键 `GameArgs`，缺省为 `None`
    #[serde(rename = "GameArgs")]
    pub game_args: Option<String>,
    /// 自定义 JVM 环境变量，对应 JSON 键 `JvmEnv`，缺省为 `None`
    #[serde(rename = "JvmEnv")]
    pub jvm_env: Option<String>,
    /// 是否把 HOME / APPDATA / XDG 等系统目录隔离到实例目录，对应 JSON 键 `IsolateHome`，
    /// 缺省为 `None`（按启用处理）
    #[serde(rename = "IsolateHome")]
    pub isolate_home: Option<bool>,
    /// GC 模式，对应 JSON 键 `GC`，缺省为 `None`
    #[serde(rename = "GC")]
    pub gc_mode: Option<GCType>,
    /// 最大内存，单位 MB；对应 JSON 键 `MaxMemory`，缺省为 `None`
    #[serde(rename = "MaxMemory")]
    pub max_memory: Option<u32>,
    /// 最小内存，单位 MB；对应 JSON 键 `MinMemory`，缺省为 `None`
    #[serde(rename = "MinMemory")]
    pub min_memory: Option<u32>,
    /// 是否启用 ColorASM 注入，对应 JSON 键 `ColorASM`，缺省为 `None`
    #[serde(rename = "ColorASM")]
    pub colorasm: Option<bool>,
    /// 是否在启动 Minecraft 前执行预启动命令，对应 JSON 键 `LaunchPre`，缺省为 `None`
    #[serde(rename = "LaunchPre")]
    pub launch_pre_run: Option<bool>,
    /// 预启动命令是否与游戏同时运行，`true` 同时、`false` 等命令结束后再启动游戏；
    /// 对应 JSON 键 `PreRunSame`，缺省为 `None`
    #[serde(rename = "PreRunSame")]
    pub pre_run_with_game: Option<bool>,
    /// 是否在游戏结束后执行后置命令，对应 JSON 键 `LaunchPost`，缺省为 `None`
    #[serde(rename = "LaunchPost")]
    pub launch_post_run: Option<bool>,
    /// 预启动命令内容，对应 JSON 键 `LaunchPreData`，缺省为 `None`
    #[serde(rename = "LaunchPreData")]
    pub pre_run_arg: Option<String>,
    /// 后置命令内容，对应 JSON 键 `LaunchPostData`，缺省为 `None`
    #[serde(rename = "LaunchPostData")]
    pub post_run_arg: Option<String>,
}

impl Default for RunArgObj {
    fn default() -> Self {
        Self {
            remove_jvm_arg: Option::None,
            remove_game_arg: Option::None,
            jvm_args: Option::None,
            game_args: Option::None,
            jvm_env: Option::None,
            isolate_home: Option::None,
            gc_mode: Option::None,
            max_memory: Option::None,
            min_memory: Option::None,
            colorasm: Option::None,
            launch_pre_run: Option::None,
            pre_run_with_game: Option::None,
            launch_post_run: Option::None,
            pre_run_arg: Option::None,
            post_run_arg: Option::None,
        }
    }
}

impl RunArgObj {
    /// 创建带合理默认值的启动参数
    ///
    /// 内存 512–4096 MB，GC 自动，预启动命令与游戏同时运行
    ///
    /// # 返回值
    ///
    /// 新建的启动参数，各字段都是 `Some`，GC 模式为 `Auto`
    pub fn new() -> Self {
        RunArgObj {
            remove_jvm_arg: Option::Some(false),
            remove_game_arg: Option::Some(false),
            jvm_args: Option::Some(String::new()),
            game_args: Option::Some(String::new()),
            jvm_env: Option::Some(String::new()),
            isolate_home: Option::Some(true),
            gc_mode: Option::Some(GCType::Auto),
            max_memory: Option::Some(4096),
            min_memory: Option::Some(512),
            colorasm: Option::Some(false),
            launch_pre_run: Option::Some(false),
            pre_run_with_game: Option::Some(true),
            launch_post_run: Option::Some(false),
            pre_run_arg: Option::Some(String::new()),
            post_run_arg: Option::Some(String::new()),
        }
    }
}

/// 游戏窗口设置
///
/// 控制 Minecraft 游戏窗口的大小、标题和全屏模式。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct WindowSettingObj {
    /// 是否启动全屏模式，对应 JSON 键 `FullScreen`，缺省为 `None`
    #[serde(rename = "FullScreen")]
    pub full_screen: Option<bool>,
    /// 窗口宽度，单位像素；对应 JSON 键 `Width`，缺省为 `None`
    #[serde(rename = "Width")]
    pub width: Option<u16>,
    /// 窗口高度，单位像素；对应 JSON 键 `Height`，缺省为 `None`
    #[serde(rename = "Height")]
    pub height: Option<u16>,
    /// 自定义游戏窗口标题，对应 JSON 键 `GameTitle`，缺省为 `None`
    #[serde(rename = "GameTitle")]
    pub game_title: Option<String>,
    /// 是否启用自定义标题，对应 JSON 键 `EditTitle`，缺省为 `None`
    #[serde(rename = "EditTitle")]
    pub edit_title: Option<bool>,
    /// 是否使用随机标题，对应 JSON 键 `RandomTitle`，缺省为 `None`
    #[serde(rename = "RandomTitle")]
    pub random_title: Option<bool>,
    /// 是否循环切换标题，对应 JSON 键 `CycTitle`，缺省为 `None`
    #[serde(rename = "CycTitle")]
    pub cycle_title: Option<bool>,
    /// 循环标题切换延迟，单位毫秒；对应 JSON 键 `TitleDelay`，缺省为 `None`
    #[serde(rename = "TitleDelay")]
    pub title_delay: Option<u32>,
}

impl WindowSettingObj {
    /// 创建默认窗口设置：1280×720，窗口模式
    ///
    /// # 返回值
    ///
    /// 新建的窗口设置：窗口模式、1280×720，其余字段为 `None`
    pub fn new() -> Self {
        Self {
            full_screen: Some(false),
            width: Some(1280),
            height: Some(720),
            game_title: None,
            edit_title: None,
            random_title: None,
            cycle_title: None,
            title_delay: None,
        }
    }
}

impl Default for WindowSettingObj {
    fn default() -> Self {
        Self {
            full_screen: Option::None,
            width: Option::None,
            height: Option::None,
            game_title: Option::None,
            edit_title: Option::None,
            random_title: Option::None,
            cycle_title: Option::None,
            title_delay: Option::None,
        }
    }
}

/// 游戏文件完整性检查设置
///
/// 控制启动器对游戏核心、运行库、资源文件和模组的校验行为。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct GameCheckObj {
    /// 检查游戏核心 jar 文件是否存在，对应 JSON 键 `CheckCore`，默认 true
    #[serde(rename = "CheckCore")]
    pub core: bool,
    /// 检查运行库文件是否存在，对应 JSON 键 `CheckLib`，默认 true
    #[serde(rename = "CheckLib")]
    pub lib: bool,
    /// 检查资源文件是否存在，对应 JSON 键 `CheckAssets`，默认 true
    #[serde(rename = "CheckAssets")]
    pub assets: bool,
    /// 检查模组文件是否存在，对应 JSON 键 `CheckMod`，默认 true
    #[serde(rename = "CheckMod")]
    pub game_mod: bool,
    /// 校验游戏核心 SHA1，对应 JSON 键 `CheckCoreSha1`，默认 true
    #[serde(rename = "CheckCoreSha1")]
    pub core_sha1: bool,
    /// 校验运行库 SHA1，对应 JSON 键 `CheckLibSha1`，默认 true
    #[serde(rename = "CheckLibSha1")]
    pub lib_sha1: bool,
    /// 校验资源文件 SHA1，对应 JSON 键 `CheckAssetsSha1`，默认 true
    #[serde(rename = "CheckAssetsSha1")]
    pub assets_sha1: bool,
    /// 校验模组 SHA1，对应 JSON 键 `CheckModSha1`，默认 true
    #[serde(rename = "CheckModSha1")]
    pub mod_sha1: bool,
}

impl Default for GameCheckObj {
    fn default() -> Self {
        Self {
            core: true,
            lib: true,
            assets: true,
            game_mod: true,
            core_sha1: true,
            lib_sha1: true,
            assets_sha1: true,
            mod_sha1: true,
        }
    }
}

/// 启动器顶层配置
///
/// 包含所有可配置项，序列化为 JSON 存储。
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ConfigObj {
    /// 配置文件版本，用于自动迁移；对应 JSON 键 `Version`，默认为当前启动器版本
    #[serde(rename = "Version")]
    pub version: String,
    /// 已添加的 Java 运行时列表，对应 JSON 键 `JavaList`，默认空列表
    #[serde(rename = "JavaList")]
    pub java_list: Vec<JvmConfigObj>,
    /// 网络设置，对应 JSON 键 `Http`，默认 `HttpObj::default()`
    #[serde(rename = "Http")]
    pub http: HttpObj,
    /// 自定义 DNS 设置，对应 JSON 键 `Dns`，默认 `DnsObj::default()`
    #[serde(rename = "Dns")]
    pub dns: DnsObj,
    /// 默认 JVM 启动参数，对应 JSON 键 `DefaultJvmArg`，默认 `RunArgObj::new()`
    #[serde(rename = "DefaultJvmArg")]
    pub jvm_arg: RunArgObj,
    /// 游戏窗口设置，对应 JSON 键 `Window`，默认 `WindowSettingObj::new()`
    #[serde(rename = "Window")]
    pub window: WindowSettingObj,
    /// 游戏文件检查设置，对应 JSON 键 `GameCheck`，默认 `GameCheckObj::default()`
    #[serde(rename = "GameCheck")]
    pub check: GameCheckObj,
}

impl Default for ConfigObj {
    fn default() -> Self {
        Self {
            version: mml_names::VERSION.clone(),
            java_list: Vec::new(),
            http: HttpObj::default(),
            dns: DnsObj::default(),
            jvm_arg: RunArgObj::new(),
            window: WindowSettingObj::new(),
            check: GameCheckObj::default(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::ConfigObj;

    /// ColorMC 的 config.json 原样（代理用户名 / 密码写的是 null）
    ///
    /// 迁移后 M²L 要能读它，否则核心初始化直接失败、启动器起不来。
    #[test]
    fn color_mc_config_with_null_proxy_loads() {
        let json = r#"{
  "Version": "40.20260928",
  "JavaList": [
    {
      "Name": "java_1.8.0_301",
      "Local": "C:\\Program Files\\Java\\jre1.8.0_301\\bin\\javaw.exe"
    }
  ],
  "Http": {
    "Source": 0,
    "DownloadThread": 20,
    "ProxyIP": "127.0.0.1",
    "ProxyPort": 7890,
    "ProxyUser": null,
    "ProxyPassword": null,
    "LoginProxy": true,
    "DownloadProxy": true,
    "GameProxy": false,
    "CheckFile": true,
    "AutoDownload": true
  },
  "Dns": {
    "Enable": false,
    "Https": [
      "https://223.5.5.5/dns-query"
    ],
    "HttpProxy": false
  },
  "DefaultJvmArg": {
    "RemoveJvmArg": null,
    "RemoveGameArg": null,
    "JvmArgs": null,
    "GameArgs": null,
    "JvmEnv": null,
    "GC": 0,
    "MaxMemory": 4096,
    "MinMemory": 512,
    "ColorASM": false,
    "LaunchPre": false,
    "PreRunSame": false,
    "LaunchPreData": null,
    "LaunchPost": false,
    "LaunchPostData": null
  },
  "Window": {
    "FullScreen": false,
    "Height": 1080,
    "Width": 1920,
    "GameTitle": null,
    "RandomTitle": false,
    "CycTitle": false,
    "TitleDelay": 0,
    "EditTitle": false
  },
  "GameCheck": {
    "CheckCore": true,
    "CheckLib": true,
    "CheckAssets": true,
    "CheckMod": true,
    "CheckCoreSha1": true,
    "CheckLibSha1": true,
    "CheckAssetsSha1": true,
    "CheckModSha1": true
  },
  "SafeLog4j": false
}"#;
        let obj: ConfigObj = serde_json::from_str(json).expect("ColorMC 的配置要能读");
        assert_eq!(obj.http.proxy_port, 7890);
        assert!(obj.http.proxy_user.is_empty());
        assert_eq!(obj.java_list.len(), 1);
    }
}
