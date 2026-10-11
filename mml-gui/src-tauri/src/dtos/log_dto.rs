//! 游戏日志窗口 DTO：log-focus 事件负载，前端 wire，camelCase

use serde::Serialize;

/// 切换目标实例事件：日志窗口已存在时再次打开，壳层推送 `log-focus` 事件让已开窗口切换实例
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFocusDto {
    /// 目标实例 UUID
    pub uuid: String,
}
