//! 实例导出窗口 DTO：前端 wire，camelCase
//!
//! 导出信息 / 目录浏览 / 导出配置 / 导出进度事件的负载。

use serde::{Deserialize, Serialize};

/// 导出时可勾选的一个条目，模组与普通文件共用
///
/// 路径一律**相对游戏目录**（`.minecraft`），如 `mods/xxx.jar`、`config/abc.toml`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntryDto {
    /// 相对游戏目录的路径
    pub path: String,
    /// 是否目录
    pub dir: bool,
    /// 文件大小，单位字节；目录为 0
    pub size: u64,
    /// 下载地址；没有在线来源时是空串
    pub url: String,
    /// 项目 ID，只有在线来源的模组才有
    pub mod_id: Option<String>,
    /// 文件 ID，只有在线来源的模组才有
    pub file_id: Option<String>,
    /// 实例记录的 SHA1；没有在线来源时是空串，服务器实例窗口的表格用它显示哈希值
    pub sha1: String,
}

/// 实例导出信息，即 `export_get_info` 的返回值
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfoDto {
    /// 当前平台是否允许将启动器程序放入服务器包。
    pub can_bundle_launcher: bool,
    /// 实例名
    pub name: String,
    /// 实例用的是自定义加载器：MMC / HMCL 的包内没有它的组件位，导出的包里只有游戏版本
    pub custom_loader: bool,
    /// `mods` 目录下的模组，有在线来源的带上项目号与下载地址
    pub mods: Vec<ExportEntryDto>,
}

/// 前端勾选的一个条目，即 `export_run` 入参里的一份
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPickDto {
    /// 相对游戏目录的路径；目录会被展开成其中的文件
    pub path: String,
    /// 下载地址；Modrinth 格式下填了才写进 index，空串进 overrides
    pub url: String,
    /// 项目号；模组页可改，给了就以它为准，不给用实例里的在线信息
    pub mod_id: Option<String>,
    /// 文件号；同上
    pub file_id: Option<String>,
    /// 这一条是取消勾选，把该目录或文件从勾选里去掉；默认是勾选
    #[serde(default)]
    pub exclude: bool,
}

/// 服务器包要带哪些内容，即 `ExportConfigDto.server`
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerPackOptionsDto {
    /// 是否导出启动器配置 config.json
    pub config: bool,
    /// 是否导出界面配置 gui_config.json
    pub gui_config: bool,
    /// 是否导出整页主题 custom_ui/page.zip
    pub page: bool,
    /// 是否导出全部自定义样式 custom_ui/css/*.zip
    pub css: bool,
    /// 是否导出自定义主页面 home
    pub home: bool,
    /// 是否导出启动器背景图 `bg_image`
    pub bg: bool,
}

impl Default for ServerPackOptionsDto {
    /// 默认带两份配置，自定义内容不选
    fn default() -> Self {
        Self {
            config: true,
            gui_config: true,
            page: false,
            css: false,
            home: false,
            bg: false,
        }
    }
}

/// 前端提交的导出配置，即 `export_run` 的入参
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfigDto {
    /// Windows 服务器包是否附带启动器程序。
    #[serde(default)]
    pub include_launcher: bool,
    /// 导出格式：curseforge / modrinth
    pub pack: String,
    /// 压缩档位：fast / normal / solid，见 `ZipLevel`；缺省按 fast
    #[serde(default)]
    pub compress: String,
    /// 保存位置，前端 save 对话框选好的完整路径
    pub file: String,
    /// 勾选的模组
    pub mods: Vec<ExportPickDto>,
    /// 勾选的文件与目录，以及父目录勾着时单独取消掉的条目
    pub files: Vec<ExportPickDto>,
    /// 整合包名
    pub name: String,
    /// 作者
    pub author: String,
    /// 版本
    pub version: String,
    /// 简介
    pub summary: String,
    /// 服务器包要带上的实例 uuid；空表示只带 `export_run` 传进来的那一个，其它格式不看
    #[serde(default)]
    pub instances: Vec<String>,
    /// 服务器包选项；其它格式不看这个字段
    #[serde(default)]
    pub server: ServerPackOptionsDto,
}

/// 导出进度事件，事件名为 `export-progress`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressDto {
    /// 状态：running / done / failed
    pub state: String,
    /// 当前处理数
    pub now: u64,
    /// 总数
    pub total: u64,
    /// 当前处理的文件名
    pub text: String,
    /// 失败原因，failed 时有值
    pub error: Option<String>,
    /// 导出文件保存路径，done 时前端可提示
    pub file: String,
}
