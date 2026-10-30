//! 统计窗口 DTO —— `stats_get_data` / `stats_get_playtime` 的返回结构

use serde::Serialize;

/// 单个实例的统计汇总
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsInstanceDto {
    /// 实例标识
    pub uuid: String,
    /// 实例名字，已删除的实例取历史记录里的名字
    pub name: String,
    /// 启动次数，含成功与失败
    pub count: u64,
    /// 启动失败次数
    pub failed: u64,
    /// 累计游玩秒数，含正在运行的本次时长
    pub seconds: u64,
    /// 最早一次记录时间，epoch 毫秒；无记录为 None
    ///
    /// 统计窗口用它限制日期选择的下界：不能早于这个实例的第一条记录。
    pub first: Option<i64>,
    /// 最近一次启动时间，epoch 毫秒；无记录为 None
    pub last: Option<i64>,
    /// 是否正在运行
    pub running: bool,
    /// 是否已删除：当前实例列表里没有它，只保留历史统计
    pub deleted: bool,
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

/// 游玩时间图里的一天
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsPlayDayDto {
    /// 本地日期，格式 YYYY-MM-DD
    pub date: String,
    /// 当天游玩秒数
    pub seconds: u64,
    /// 当天启动次数，含成功与失败
    pub count: u32,
    /// 当天启动失败次数
    pub failed: u32,
}

/// 单日里一个实例的统计
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDayInstanceDto {
    /// 实例标识
    pub uuid: String,
    /// 实例名字，已删除的实例取历史记录里的名字
    pub name: String,
    /// 当天游玩秒数
    pub seconds: u64,
    /// 当天启动次数，含成功与失败
    pub count: u32,
    /// 当天启动失败次数
    pub failed: u32,
}

/// 游玩时间图：按天分桶，缺数据的日期补 0
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsPlaytimeDto {
    /// 每天的游玩时长，按日期升序；窗口内没有记录的日期也有一项
    pub days: Vec<StatsPlayDayDto>,
    /// 时间段内累计游玩秒数
    pub total_seconds: u64,
    /// 有游玩记录的天数
    pub active_days: u32,
    /// 范围内各实例的时长，按时长降序；没有记录时为空
    pub by_instance: Vec<StatsDayInstanceDto>,
}