//! Mojang 版本启动参数 DTO，对应 version.json

use serde::{Deserialize, Serialize};

/// 规则适用的系统
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameOsObj {
    /// 系统名，取值 `windows` / `linux` / `osx`
    pub name: String,
    /// 架构，如 `x86`
    pub arch: String,
}

/// 参数适用规则
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameRulesObj {
    /// 动作，取值 `allow` / `disallow`
    pub action: String,
    /// 适用的系统，`None` 表示全部系统
    pub os: Option<GameOsObj>,
}

/// 运行库构件信息
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ArtifactObj {
    /// 相对运行库目录的路径
    pub path: String,
    /// SHA1 校验
    pub sha1: String,
    /// 下载地址
    pub url: String,
}

/// 各平台的原生库构件
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ClassifiersObj {
    /// Linux 原生库
    #[serde(rename = "natives-linux")]
    pub natives_linux: ArtifactObj,
    /// macOS 原生库
    #[serde(rename = "natives-osx")]
    pub natives_osx: ArtifactObj,
    /// Windows 原生库
    #[serde(rename = "natives-windows")]
    pub natives_windows: ArtifactObj,
    /// Windows 32 位原生库
    #[serde(rename = "natives-windows-32")]
    pub natives_windows_32: ArtifactObj,
    /// Windows 64 位原生库
    #[serde(rename = "natives-windows-64")]
    pub natives_windows_64: ArtifactObj,
}

/// 运行库下载信息
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameLibrariesDownloadsObj {
    /// 各平台原生库，没有原生库时为 `None`
    pub classifiers: Option<ClassifiersObj>,
    /// 主构件
    pub artifact: ArtifactObj,
}

/// 参数值，单个值或一组值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    /// 单个值
    Single(String),
    /// 多个值
    Multi(Vec<String>),
}

impl Default for ArgValue {
    fn default() -> Self {
        ArgValue::Single(Default::default())
    }
}

/// 带规则的 JVM 参数
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GameJvmObj {
    /// 适用规则
    pub rules: Vec<GameRulesObj>,
    /// 参数值
    pub value: ArgValue,
}

/// 启动参数项，纯字符串或带规则
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    /// 无条件参数
    Plain(String),
    /// 带规则的参数
    Conditional(GameJvmObj),
}

/// 游戏与 JVM 参数
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct GameArgumentsObj {
    /// 游戏参数
    pub game: Vec<Argument>,
    /// JVM 参数
    pub jvm: Vec<Argument>,
}

/// 资源索引信息
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameAssetIndexObj {
    /// 索引 ID
    pub id: String,
    /// 索引下载地址
    pub url: String,
}

/// 单个下载项
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct GameDownloadItemObj {
    /// SHA1 校验
    pub sha1: String,
    /// 下载地址
    pub url: String,
}

/// 游戏本体下载信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct GameDownloadsObj {
    /// 客户端
    pub client: GameDownloadItemObj,
}

/// 所需 Java 版本
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct GameJavaVersionObj {
    /// 主版本号
    #[serde(rename = "majorVersion")]
    pub major_version: i32,
}

/// 运行库信息
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameLibrariesObj {
    /// 下载信息
    pub downloads: GameLibrariesDownloadsObj,
    /// Maven 坐标
    pub name: String,
    /// 适用规则
    pub rules: Vec<GameRulesObj>,
    /// 下载源地址
    pub url: String,
    /// MMC 的库提示，取值 `local`：文件在**实例目录**的 `libraries/` 下、按
    /// `构件-版本.jar` 平铺，不下载
    #[serde(rename = "MMC-hint")]
    pub mmc_hint: Option<String>,
}

/// 日志配置文件信息
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ClientObj {
    /// 参数占位符
    pub argument: String,
    /// 配置文件下载信息
    pub file: GameDownloadItemObj,
}

/// 游戏日志配置
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LoggingObj {
    /// 日志配置文件信息
    pub client: ClientObj,
}

/// 版本启动数据
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct GameArgObj {
    /// 资源索引
    #[serde(rename = "assetIndex")]
    pub asset_index: Option<GameAssetIndexObj>,
    /// 游戏本体下载信息
    pub downloads: GameDownloadsObj,
    /// 版本 ID
    pub id: String,
    /// 所需 Java 版本
    #[serde(rename = "javaVersion")]
    pub java_version: Option<GameJavaVersionObj>,
    /// 运行库列表
    pub libraries: Option<Vec<GameLibrariesObj>>,
    /// 日志配置
    pub logging: Option<LoggingObj>,
    /// 主类
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// 旧版游戏参数，合并式字符串
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    /// 最低启动器版本
    #[serde(rename = "minimumLauncherVersion")]
    pub minimum_launcher_version: i32,
    /// 发布时间
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    /// 新版游戏与 JVM 参数
    pub arguments: Option<GameArgumentsObj>,
}
