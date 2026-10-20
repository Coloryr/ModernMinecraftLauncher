//! 自定义主页面 DTO：TS 命名，camelCase wire
//!
//! 自定义主页面的本体是 `base_dir/custom_home.zip`，即服主导入的整页 HTML 压缩包，
//! 启动时打开成常驻句柄、请求时按条目现读，磁盘上不存任何派生状态，
//! 这里的状态每次都由 `custom_home_status` 现算。
use serde::{Deserialize, Serialize};

use super::ThemeKindDto;

/// 自定义主页面状态，`custom_home_status` 与 `custom_home_import` 的返回值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomHomeInfoDto {
    /// 是否已导入，即压缩包存在、能打开且有 index.html 或 custom.js
    pub installed: bool,
    /// 配置文件里的启用开关，对应 client.custom_home
    pub enabled: bool,
    /// 实际类型；没有清单时按导入入口推断，对主页面就是 home
    pub kind: ThemeKindDto,
    /// 主题名，来自包根 theme.json；没有清单或没写 name 时为空串
    pub name: String,
    /// 主题版本，来自包根 theme.json；没有清单或没写 version 时为空串
    pub version: String,
    /// 是否由 theme.json 声明了类型；为 false 时设置页提示「未声明类型」
    pub declared: bool,
    /// 入口页完整 URL，未导入或包里没有 index.html 时为空串；已带破缓存参数
    pub entry_url: String,
    /// 窗口页面组件入口脚本 URL；包里没有 custom.js 时为空串
    pub script_url: String,
    /// 组件入口的附加样式 URL；包里没有 custom.css 时为空串
    pub style_url: String,
    /// 内容版本号，取压缩包 mtime，用于 iframe 破缓存
    pub content_version: String,
    /// 包内文件数，不含目录条目
    pub file_count: u32,
}
