//! 统计窗口 DTO —— `stats_get_data` 的返回结构

use serde::Serialize;

/// 单个实例的统计汇总
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsInstanceDto {
    /// 实例标识
    pub uuid: String,
    /// 实例名字，已删除的实例取历史记录里的名字
    pub name: String,
    /// 启动次数
    pub count: u64,
    /// 累计游玩秒数，含正在运行的本次时长
    pub seconds: u64,
    /// 最近一次启动时间，epoch 毫秒；无记录为 None
    pub last: Option<i64>,
    /// 是否正在运行
    pub running: bool,
}

/// 统计快照，全局计数 + 每实例汇总
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDataDto {
    /// 启动次数
    pub launch_count: i64,
    /// 启动完成次数
    pub launch_done_count: i64,
    /// 启动失败次数
    pub launch_error_count: i64,
    /// 总游戏时长，单位秒
    pub total_seconds: u64,
    /// 每实例统计，现有实例 + 有历史记录的实例
    pub instances: Vec<StatsInstanceDto>,
}
