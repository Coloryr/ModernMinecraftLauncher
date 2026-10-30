//! 统计窗口：规格 + 数据读取
//!
//! 统计数据由内核 `mml_game::game_count` 维护，落盘结构是 `CountObj`，走 NBT，
//! 这里只做快照聚合：全局计数 + 每实例启动次数 / 累计时长 / 最近启动，
//! 以及按实例与时间段聚合的每日游玩时长。

use std::collections::{HashMap, HashSet};

use chrono::{Days, Local, NaiveDate, NaiveTime};
use mml_game::game_count;
use uuid::Uuid;

use crate::dtos::stats_dto::{
    StatsDataDto, StatsDayInstanceDto, StatsInstanceDto, StatsPlayDayDto, StatsPlaytimeDto,
};
use crate::err_keys;

/// 单实例的统计汇总，聚合期间的中间值
#[derive(Clone, Copy, Default)]
struct InstanceAgg {
    /// 启动次数，含成功与失败
    count: u64,
    /// 启动失败次数
    failed: u64,
    /// 累计运行时长，单位秒
    seconds: u64,
    /// 最早一次记录的时间戳，单位毫秒；没有记录时为 `None`
    first: Option<i64>,
    /// 最近一次启动的时间戳，单位毫秒；没有记录时为 `None`
    last: Option<i64>,
    /// 当前是否正在运行
    running: bool,
}

/// 获取统计快照：全局计数加每实例汇总
///
/// # 返回值
///
/// 全局启动计数与每实例的启动次数 / 失败次数 / 累计时长 / 最近启动；失败返回错误文案
#[gui_macros::command]
pub fn stats_get_data() -> Result<StatsDataDto, String> {
    let count = game_count::get_count();
    let count = count.read().unwrap();

    // 每实例：先按 game_runs 算有效运行；启动次数稍后用启动日志补齐，
    // 时长 = 各次运行起止差之和，正在运行的那次把从开始到现在的时长补上
    let now = Local::now();
    let mut agg: HashMap<Uuid, InstanceAgg> = HashMap::new();
    for (uuid, runs) in &count.game_runs {
        let mut secs = 0u64;
        let mut first: Option<i64> = None;
        let mut last: Option<i64> = None;
        let mut running = false;
        for run in runs {
            let end = if run.now { now } else { run.stop_time };
            secs += (end - run.start_time).num_seconds().max(0) as u64;

            let start_ms = run.start_time.timestamp_millis();
            if first.is_none_or(|v| start_ms < v) {
                first = Some(start_ms);
            }
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
                failed: 0,
                seconds: secs,
                first,
                last,
                running,
            },
        );
    }

    // 启动日志一条对应一次启动（成功、失败都记），把它当总次数：
    // 多出来的部分就是失败次数；没有成功运行的实例也能靠它出现在表里
    for (uuid, logs) in &count.launch_logs {
        let total = logs.len() as u64;
        let entry = agg.entry(*uuid).or_default();
        if total > entry.count {
            entry.failed = total - entry.count;
            entry.count = total;
        }
        // 失败启动只有日志没有运行段，下界也要算上它们
        for log in logs {
            let time_ms = log.time.timestamp_millis();
            if entry.first.is_none_or(|v| time_ms < v) {
                entry.first = Some(time_ms);
            }
        }
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
            failed: a.failed,
            seconds: a.seconds,
            first: a.first,
            last: a.last,
            running: a.running,
            deleted: false,
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
                failed: a.failed,
                seconds: a.seconds,
                first: a.first,
                last: a.last,
                running: a.running,
                deleted: true,
            });
        }
    }
    // 只有名字快照、没有运行或启动记录的已删除实例也列出来：
    // 名字是在创建/删除时写进 game_names 的，这里不读就永远不会显示
    for (uuid, name) in &count.game_names {
        if seen.contains(uuid) || agg.contains_key(uuid) {
            continue;
        }
        items.push(StatsInstanceDto {
            uuid: uuid.to_string(),
            name: name.clone(),
            count: 0,
            failed: 0,
            seconds: 0,
            first: None,
            last: None,
            running: false,
            deleted: true,
        });
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

/// 获取某实例在指定日期范围内的每日游玩时长
///
/// 跨天的运行会按本地零点切开，分别计入两天。范围包含 `from` / `to` 两天，
/// 没有记录的日期补 0，前端不用再补齐横轴；范围上限十年，避免极端输入生成过长的横轴。
///
/// - `uuid`: 实例 uuid；`None` 或空串表示所有实例
/// - `from`: 起始日期，`YYYY-MM-DD`
/// - `to`: 结束日期，`YYYY-MM-DD`；超过今天时截到今天
///
/// # 返回值
///
/// 见 [`StatsPlaytimeDto`]；uuid 或日期非法时返回错误文案
#[gui_macros::command]
pub fn stats_get_playtime(
    uuid: Option<String>,
    from: String,
    to: String,
) -> Result<StatsPlaytimeDto, String> {
    let filter = match uuid.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(Uuid::parse_str(s).map_err(|_| err_keys::UUID.to_string())?),
        None => None,
    };

    let mut from_date = NaiveDate::parse_from_str(&from, "%Y-%m-%d")
        .map_err(|_| err_keys::DATE_INVALID.to_string())?;
    let mut to_date = NaiveDate::parse_from_str(&to, "%Y-%m-%d")
        .map_err(|_| err_keys::DATE_INVALID.to_string())?;
    if from_date > to_date {
        std::mem::swap(&mut from_date, &mut to_date);
    }

    let now = Local::now().naive_local();
    let today = now.date();
    if from_date > today {
        return Ok(StatsPlaytimeDto {
            days: Vec::new(),
            total_seconds: 0,
            active_days: 0,
            by_instance: Vec::new(),
        });
    }
    if to_date > today {
        to_date = today;
    }
    // 上限十年：极端输入不至于生成几十年的横轴
    if (to_date - from_date).num_days() > 3659 {
        from_date = to_date - Days::new(3659);
    }

    let day_count = (to_date - from_date).num_days() as u64 + 1;
    let range_start = from_date.and_time(NaiveTime::MIN);
    let range_end = (to_date + Days::new(1)).and_time(NaiveTime::MIN).min(now);

    let mut buckets: Vec<StatsPlayDayDto> = Vec::with_capacity(day_count as usize);
    let mut index: HashMap<NaiveDate, usize> = HashMap::with_capacity(day_count as usize);
    let mut date = from_date;
    loop {
        index.insert(date, buckets.len());
        buckets.push(StatsPlayDayDto {
            date: date.format("%Y-%m-%d").to_string(),
            seconds: 0,
            count: 0,
            failed: 0,
        });
        if date == to_date {
            break;
        }
        date = date + Days::new(1);
    }

    let count = game_count::get_count();
    let count = count.read().unwrap();
    // 按实例拆分：饼图要看这段时间里各实例的占比，单日与多日都算
    let mut per_instance: HashMap<Uuid, u64> = HashMap::new();
    // 当天各实例的启动次数与成功次数；launch_error 不会 push game_run，两者相减即失败数
    let mut starts_per_instance: HashMap<Uuid, u32> = HashMap::new();
    let mut dones_per_instance: HashMap<Uuid, u32> = HashMap::new();

    // 每天的成功启动次数：launch_done 会 push 一条 game_run，launch_error 不会，
    // 所以「启动日志条数 − 成功次数」就是当天的失败次数
    let mut done_per_day = vec![0u32; day_count as usize];
    for (inst_uuid, runs) in &count.game_runs {
        if filter.is_some_and(|want| *inst_uuid != want) {
            continue;
        }
        for run in runs {
            if let Some(i) = index.get(&run.start_time.naive_local().date()) {
                done_per_day[*i] += 1;
                *dones_per_instance.entry(*inst_uuid).or_default() += 1;
            }
            let end = if run.now {
                now
            } else {
                run.stop_time.naive_local()
            };
            let start = run.start_time.naive_local();
            if end <= range_start || start >= range_end || end <= start {
                continue;
            }
            let start = start.max(range_start);
            let end = end.min(range_end);
            let mut cursor = start;
            while cursor < end {
                let date = cursor.date();
                let next = (date + Days::new(1)).and_time(NaiveTime::MIN);
                let chunk_end = end.min(next);
                let secs = (chunk_end - cursor).num_seconds().max(0) as u64;
                if let Some(i) = index.get(&date) {
                    buckets[*i].seconds += secs;
                    *per_instance.entry(*inst_uuid).or_default() += secs;
                }
                cursor = chunk_end;
            }
        }
    }

    // 启动次数按天分桶，日期取日志的记录时间
    for (inst_uuid, logs) in &count.launch_logs {
        if filter.is_some_and(|want| *inst_uuid != want) {
            continue;
        }
        for log in logs {
            if let Some(i) = index.get(&log.time.naive_local().date()) {
                buckets[*i].count += 1;
                *starts_per_instance.entry(*inst_uuid).or_default() += 1;
            }
        }
    }
    for (i, day) in buckets.iter_mut().enumerate() {
        day.failed = day.count.saturating_sub(done_per_day[i]);
    }

    // 补实例名：现有实例取当前名字，已删除的取历史快照
    let mut by_instance: Vec<StatsDayInstanceDto> = Vec::new();
    if !per_instance.is_empty() {
        let mut names: HashMap<Uuid, String> = count.game_names.clone();
        for game in mml_game::get_instances() {
            let g = game.read().unwrap();
            names.insert(g.uuid, g.name.clone());
        }
        by_instance = per_instance
            .into_iter()
            .filter(|(_, seconds)| *seconds > 0)
            .map(|(uuid, seconds)| {
                let count = starts_per_instance.get(&uuid).copied().unwrap_or(0);
                let done = dones_per_instance.get(&uuid).copied().unwrap_or(0);
                StatsDayInstanceDto {
                    uuid: uuid.to_string(),
                    name: names
                        .get(&uuid)
                        .cloned()
                        .unwrap_or_else(|| uuid.to_string()),
                    seconds,
                    count,
                    failed: count.saturating_sub(done),
                }
            })
            .collect();
        by_instance.sort_by(|a, b| b.seconds.cmp(&a.seconds));
    }

    let total_seconds = buckets.iter().map(|day| day.seconds).sum();
    let active_days = buckets.iter().filter(|day| day.seconds > 0).count() as u32;
    Ok(StatsPlaytimeDto {
        days: buckets,
        total_seconds,
        active_days,
        by_instance,
    })
}