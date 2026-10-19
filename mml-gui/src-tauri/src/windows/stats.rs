//! 统计窗口：规格 + 数据读取
//!
//! 统计数据由内核 `mml_game::game_count` 维护，落盘结构是 `CountObj`，走 NBT，
//! 这里只做快照聚合：全局计数 + 每实例启动次数 / 累计时长 / 最近启动。

use std::collections::{HashMap, HashSet};

use chrono::Local;
use mml_game::game_count;
use uuid::Uuid;

use crate::dtos::stats_dto::{StatsDataDto, StatsInstanceDto};

/// 单实例的统计汇总，聚合期间的中间值
#[derive(Clone, Copy, Default)]
struct InstanceAgg {
    /// 启动次数
    count: u64,
    /// 累计运行时长，单位秒
    seconds: u64,
    /// 最近一次启动的时间戳，单位毫秒；没有记录时为 `None`
    last: Option<i64>,
    /// 当前是否正在运行
    running: bool,
}

/// 获取统计快照：全局计数加每实例汇总
///
/// # 返回值
///
/// 全局启动计数与每实例的启动次数 / 累计时长 / 最近启动；失败返回错误文案
#[gui_macros::command]
pub fn stats_get_data() -> Result<StatsDataDto, String> {
    let count = game_count::get_count();
    let count = count.read().unwrap();

    // 每实例：启动次数 = game_runs 条数；时长 = 各次运行起止差之和；
    // 正在运行的那次 stop_time 未更新，把从开始到现在的时长补上
    let now = Local::now();
    let mut agg: HashMap<Uuid, InstanceAgg> = HashMap::new();
    for (uuid, runs) in &count.game_runs {
        let mut secs = 0u64;
        let mut last: Option<i64> = None;
        let mut running = false;
        for run in runs {
            let end = if run.now { now } else { run.stop_time };
            secs += (end - run.start_time).num_seconds().max(0) as u64;

            let start_ms = run.start_time.timestamp_millis();
            if last.is_none_or(|v| start_ms > v) {
                last = Some(start_ms);
            }
            if run.now {
                running = true;
            }
        }
        agg.insert(
            *uuid,
            InstanceAgg {
                count: runs.len() as u64,
                seconds: secs,
                last,
                running,
            },
        );
    }

    // 实例列表 = 现有实例 ∪ 有历史记录的实例：已删除的实例保留历史，名字取 game_names
    let mut items: Vec<StatsInstanceDto> = Vec::new();
    let mut seen: HashSet<Uuid> = HashSet::new();

    for game in mml_game::get_instances() {
        let g = game.read().unwrap();
        seen.insert(g.uuid);
        let a = agg.get(&g.uuid).copied().unwrap_or_default();
        items.push(StatsInstanceDto {
            uuid: g.uuid.to_string(),
            name: g.name.clone(),
            count: a.count,
            seconds: a.seconds,
            last: a.last,
            running: a.running,
        });
    }
    for uuid in agg.keys() {
        if !seen.contains(uuid) {
            let a = agg[uuid];
            items.push(StatsInstanceDto {
                uuid: uuid.to_string(),
                name: count
                    .game_names
                    .get(uuid)
                    .cloned()
                    .unwrap_or_else(|| uuid.to_string()),
                count: a.count,
                seconds: a.seconds,
                last: a.last,
                running: a.running,
            });
        }
    }
    // 最近游玩的排前面，没记录的按名字排
    items.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.name.cmp(&b.name)));

    Ok(StatsDataDto {
        launch_count: count.launch_count,
        launch_done_count: count.launch_done_count,
        launch_error_count: count.launch_error_count,
        total_seconds: count.all_time.num_seconds().max(0) as u64,
        instances: items,
    })
}
