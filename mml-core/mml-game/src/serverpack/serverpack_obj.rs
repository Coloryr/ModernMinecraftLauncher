//! 服务器整合包信息 DTO

use mml_config::config_obj::{RunArgObj, WindowSettingObj};
use serde::{Deserialize, Serialize};

use crate::{
    launcher::instance_setting_obj::{AdvanceJvmObj, ServerObj},
    loader::LoaderType,
};

/// 服务器实例
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ServerPackObj {
    /// 实例名
    #[serde(rename = "Name")]
    pub name: String,
    /// 图标文件名，相对发布目录，如 `icon.png`；没有图标时为空
    #[serde(rename = "Icon")]
    pub icon: Option<String>,
    /// 游戏版本
    #[serde(rename = "Version")]
    pub version: String,
    /// 加载器类型
    #[serde(rename = "Loader")]
    pub loader: LoaderType,
    /// 加载器版本
    #[serde(rename = "LoaderVersion")]
    pub loader_version: Option<String>,
    /// Jvm参数
    #[serde(rename = "JvmArg")]
    pub run_arg: Option<RunArgObj>,
    /// 窗口设置
    #[serde(rename = "Window")]
    pub window: Option<WindowSettingObj>,
    /// 加入服务器设置
    #[serde(rename = "StartServer")]
    pub start_server: Option<ServerObj>,
    /// 高级Jvm设置
    #[serde(rename = "AdvanceJvm")]
    pub advance_jvm: Option<AdvanceJvmObj>,

    /// 服务器包信息
    #[serde(rename = "Text")]
    pub text: String,
    /// 服务器包版本
    #[serde(rename = "PackVersion")]
    pub pack_version: String,
    /// 模组列表
    #[serde(rename = "Files")]
    pub online_list: Vec<ServerItemObj>,
    /// 配置文件列表
    #[serde(rename = "Archives")]
    pub archive_list: Vec<ServerArchiveItemObj>,
    /// 基础文件包：解压到实例根目录，导入时下载一次，后续版本不再下载
    #[serde(rename = "Base", skip_serializing_if = "Option::is_none")]
    pub base: Option<ServerBaseObj>,
}

/// 基础文件压缩包
///
/// 内容按实例根目录的相对路径打包，解压目标就是 .minecraft 所在目录；
/// 只在实例第一次同步时下发，后续版本更新不再拉取。
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ServerBaseObj {
    /// 压缩包文件名，如 base.zip
    #[serde(rename = "File")]
    pub file: String,
    /// 下载地址
    #[serde(rename = "Url")]
    pub url: String,
    /// SHA1 校验
    #[serde(rename = "Sha1")]
    pub sha1: Option<String>,
    /// SHA256 校验
    #[serde(rename = "Sha256")]
    pub sha256: Option<String>,
}

/// 服务器包里的配置文件压缩包条目
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ServerArchiveItemObj {
    /// 文件名
    #[serde(rename = "File")]
    pub file: String,
    /// 解压到的位置
    #[serde(rename = "Path")]
    pub dir: String,
    /// 是否删除旧的路径
    #[serde(rename = "Over")]
    pub delete_old: bool,
    /// 下载地址
    #[serde(rename = "Url")]
    pub url: String,
    /// SHA1 校验
    #[serde(rename = "Sha1")]
    pub sha1: Option<String>,
    /// SHA256 校验
    #[serde(rename = "Sha256")]
    pub sha256: Option<String>,
}

/// 服务器包里的模组文件条目
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct ServerItemObj {
    /// 游戏目录下的相对路径，如 `mods/foo.jar`、`config/bar.toml`
    #[serde(rename = "File")]
    pub file: String,
    /// 项目编号
    #[serde(rename = "ProjectId")]
    pub pid: Option<String>,
    /// 文件编号
    #[serde(rename = "FileId")]
    pub fid: Option<String>,
    /// 下载地址
    #[serde(rename = "Url")]
    pub url: Option<String>,
    /// SHA1 校验
    #[serde(rename = "Sha1")]
    pub sha1: Option<String>,
    /// SHA256 校验
    #[serde(rename = "Sha256")]
    pub sha256: Option<String>,
    /// 是否为自动生成的自定义编号；true 时 ProjectId / FileId 不按平台编号解析
    #[serde(rename = "Custom", default, skip_serializing_if = "is_false")]
    pub custom: bool,
}

/// 序列化时省略 false
///
/// - `value`: 字段值
///
/// # 返回值
///
/// 值为 false 时返回 true
fn is_false(value: &bool) -> bool {
    !*value
}
