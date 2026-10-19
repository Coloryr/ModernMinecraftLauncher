use mml_config::config_obj::{RunArgObj, WindowSettingObj};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    launcher::{LogEncoding, ModPackType},
    loader::LoaderType,
    mojang::VersionType,
};

/// 加入服务器设置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ServerObj {
    /// 是否在启动后自动加入该服务器
    pub enable: bool,
    /// 服务器地址
    #[serde(rename = "IP")]
    pub ip: Option<String>,
    /// 服务器端口
    #[serde(rename = "Port")]
    pub port: Option<u16>,
}

/// 端口代理设置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ProxyHostObj {
    /// 服务器地址
    #[serde(rename = "IP")]
    pub ip: Option<String>,
    /// 服务器端口
    #[serde(rename = "Port")]
    pub port: Option<u16>,

    /// 代理用户名，缺省表示不需要认证
    #[serde(rename = "User")]
    pub user: Option<String>,
    /// 代理密码，缺省表示不需要认证
    #[serde(rename = "Password")]
    pub password: Option<String>,
}

/// 高级 JVM 启动参数
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct AdvanceJvmObj {
    /// 自定义主类，覆盖版本 JSON 里的 mainClass
    #[serde(rename = "MainClass")]
    pub main_class: Option<String>,
    /// 附加 classpath，追加在自动拼接的类路径之后
    #[serde(rename = "ClassPath")]
    pub class_path: Option<String>,
}

/// 自定义模组加载器设置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct CustomLoaderObj {
    /// 后加载原版运行库
    #[serde(rename = "OffLib")]
    pub offset_lib: bool,
    /// 删除原版运行库
    #[serde(rename = "RemoveLib")]
    pub remove_lib: bool,
    /// 是否启用自定义启动配置
    #[serde(rename = "CustomJson")]
    pub custom_json: bool,
    /// 删除原有启动配置
    #[serde(rename = "RemoveJson")]
    pub remove_json: bool,
}

/// 游戏实例配置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct InstanceSettingObj {
    /// 实例标识
    #[serde(rename = "UUID")]
    pub uuid: Uuid,
    /// 实例名
    #[serde(rename = "Name")]
    pub name: String,
    /// 路径名
    #[serde(rename = "DirName")]
    pub dir: String,
    /// 游戏版本
    #[serde(rename = "Version")]
    pub version: String,
    /// 模组加载器类型
    #[serde(rename = "Loader")]
    pub loader: LoaderType,
    /// 模组加载器版本
    #[serde(rename = "LoaderVersion")]
    pub loader_version: Option<String>,
    /// JVM 参数
    #[serde(rename = "JvmArg")]
    pub jvm_arg: Option<RunArgObj>,
    /// JVM 预设名，仅作展示，实际参数取 `jvm_arg`
    #[serde(rename = "JvmName")]
    pub jvm_name: Option<String>,
    /// 指定使用的 Java 可执行文件路径，缺省则自动选择
    #[serde(rename = "JvmLocal")]
    pub jvm_local: Option<String>,
    /// 窗口设置
    #[serde(rename = "Window")]
    pub window: Option<WindowSettingObj>,
    /// 加入服务器设置
    #[serde(rename = "StartServer")]
    pub start_server: Option<ServerObj>,
    /// 启动后自动进入的存档，值是存档**目录名**（不是 level.dat 里的显示名）；
    /// 只有 1.20 及以上的版本会用它拼 `--quickPlaySingleplayer`
    #[serde(rename = "AutoSave")]
    pub auto_save: Option<String>,
    /// 端口代理设置
    #[serde(rename = "ProxyHost")]
    pub proxy_host: Option<ProxyHostObj>,
    /// 高级Jvm设置
    #[serde(rename = "AdvanceJvm")]
    pub advance_jvm: Option<AdvanceJvmObj>,
    /// 是否为整合包实例
    #[serde(rename = "Modpack")]
    pub is_modpack: bool,
    /// 整合包来源类型
    #[serde(rename = "ModPackType")]
    pub modpack_type: ModPackType,
    /// 游戏发布类型，如正式版、快照
    #[serde(rename = "GameType")]
    pub game_type: VersionType,
    /// 整合包项目 ID，非整合包实例为空
    #[serde(rename = "PID")]
    pub pid: Option<String>,
    /// 整合包文件 ID，非整合包实例为空
    #[serde(rename = "FID")]
    pub fid: Option<String>,
    /// 图标
    #[serde(rename = "Icon")]
    pub icon: Option<String>,
    /// 服务器实例网址
    #[serde(rename = "ServerUrl")]
    pub server_url: Option<String>,
    /// 自定义模组加载器
    #[serde(rename = "CustomLoader")]
    pub custom_loader: Option<CustomLoaderObj>,
    /// 日志编码
    #[serde(rename = "Encoding")]
    pub encoding: LogEncoding,
}
