//! 自定义主页面 DTO：TS 命名，camelCase wire
//!
//! 自定义主页面的本体是 `base_dir/custom_home.zip`，即服主导入的整页 HTML 压缩包，
//! 启动时打开成常驻句柄、请求时按条目现读，磁盘上不存任何派生状态，
//! 这里的状态每次都由 `custom_home_status` 现算。
use serde::{Deserialize, Serialize};

/// 自定义主页面状态，`custom_home_status` 与 `custom_home_import` 的返回值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomHomeInfoDto {
    /// 是否已导入，即压缩包存在、能打开且入口页在
    pub installed: bool,
    /// 配置文件里的启用开关，对应 client.custom_home
    pub enabled: bool,
    /// 入口页完整 URL，未导入时为空串；已带破缓存参数
    pub entry_url: String,
    /// 内容版本号，取压缩包 mtime，用于 iframe 破缓存
    pub version: String,
    /// 包内文件数，不含目录条目
    pub file_count: u32,
}
