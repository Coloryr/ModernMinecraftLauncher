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
}
