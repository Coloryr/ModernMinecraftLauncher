//! 启动时旧数据升级的状态 DTO

use serde::{Deserialize, Serialize};

/// 旧数据升级状态
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeStatusDto {
    /// 是否正在升级
    pub running: bool,
    /// 是否已经结束；没有可升级的数据时一上来就是 `true`
    pub done: bool,
    /// 是否被用户叫停
    pub cancelled: bool,
    /// 已写入条数
    pub now: u32,
    /// 总条数；`0` 表示还没统计出来
    pub total: u32,
}
