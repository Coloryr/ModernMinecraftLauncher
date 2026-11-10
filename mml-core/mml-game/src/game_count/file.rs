//! 统计文件解析与合并
//!
//! NBT 文件与旧版待合并文件的读写。

use std::{path::Path, sync::atomic::Ordering};

use mml_names::{i18_items::error_type::CoreResult, names};
use mml_nbt::nbt_file::NbtFile;
use mml_sys::path_helper;
use uuid::Uuid;

use super::save::save;
use super::time::{duration_from_ticks, ticks_to_date};
use super::{COUNT, READ_OK};
use super::{CountObj, GameTimeData, LaunchLogData};

/// 解析一份统计文件
///
/// 结构与 ColorMC 的 `count.dat` 是同一套 NBT，见 [`CountObj`]。
///
/// - `file`: 统计文件
///
/// # 返回值
///
/// 解析出的统计；文件读不出来或不是 NBT 时返回错误
pub(super) fn parse_file(file: &Path) -> CoreResult<CountObj> {
    let mut stream = path_helper::open_read(file)?;
    let nbt_file = NbtFile::read(&mut stream)?;
    let Some(nbt) = nbt_file.nbt.as_compound() else {
        return Ok(CountObj::default());
    };

    let mut count = CountObj::default();
    count.launch_count = nbt.get_long("LaunchCount").unwrap_or(0);
    count.launch_done_count = nbt.get_long("LaunchDoneCount").unwrap_or(0);
    count.launch_error_count = nbt.get_long("LaunchErrorCount").unwrap_or(0);
    count.all_time = duration_from_ticks(nbt.get_long("AllTime").unwrap_or(0));

    if let Some(list) = nbt.get_list("GameRuns") {
        for item in list.iter() {
            let Some(com) = item.as_compound() else {
                continue;
            };
            let Some(key) = com.get_string("Key") else {
                continue;
            };
            let Ok(uuid) = Uuid::parse_str(&key) else {
                continue;
            };

            let mut times = Vec::new();
            if let Some(list1) = com.get_list("List") {
                for item1 in list1.iter() {
                    let Some(com1) = item1.as_compound() else {
                        continue;
                    };
                    let start = com1.get_long("StartTime").unwrap_or(0);
                    let stop = com1.get_long("StopTime").unwrap_or(0);
                    times.push(GameTimeData {
                        now: false,
                        start_time: ticks_to_date(start),
                        stop_time: ticks_to_date(stop),
                    });
                }
            }
            count.game_runs.insert(uuid, times);
        }
    }

    if let Some(list) = nbt.get_list("LaunchLogs") {
        for item in list.iter() {
            let Some(com) = item.as_compound() else {
                continue;
            };
            let Some(key) = com.get_string("Key") else {
                continue;
            };
            let Ok(uuid) = Uuid::parse_str(&key) else {
                continue;
            };

            let mut logs = Vec::new();
            if let Some(list1) = com.get_list("List") {
                for item1 in list1.iter() {
                    let Some(com1) = item1.as_compound() else {
                        continue;
                    };
                    let time = com1.get_long("Time").unwrap_or(0);
                    let error = com1.get_byte("Error").unwrap_or(0) == 1;
                    logs.push(LaunchLogData {
                        time: ticks_to_date(time),
                        error,
                    });
                }
            }
            count.launch_logs.insert(uuid, logs);
        }
    }

    if let Some(game_names) = nbt.get_compound("GameNames") {
        for (key, value) in &game_names.data {
            if let Some(str) = value.as_string()
                && let Ok(uuid) = Uuid::parse_str(key)
            {
                count.game_names.insert(uuid, str.data.clone());
            }
        }
    }

    Ok(count)
}

/// 把另一份统计文件合并进当前统计
///
/// 从 ColorMC 迁移用：两边是同一套 NBT 结构，计数与总时长相加，运行段与启动记录
/// 按 uuid 合并并去重，实例名以已有的为准。合并后立即落盘。
///
/// - `file`: 要合并的统计文件
///
/// # 返回值
///
/// 合并成功返回 `Ok(())`；文件读不出来返回错误
pub fn merge_file(file: &Path) -> CoreResult<()> {
    let other = parse_file(file)?;
    let mut count = COUNT.write().unwrap();
    count.launch_count += other.launch_count;
    count.launch_done_count += other.launch_done_count;
    count.launch_error_count += other.launch_error_count;
    count.all_time += other.all_time;

    for (uuid, runs) in other.game_runs {
        let list = count.game_runs.entry(uuid).or_default();
        for run in runs {
            // 同一段时间重复导入时不重复计，删掉迁移标记再搬一次也不会翻倍
            let dup = list
                .iter()
                .any(|old| old.start_time == run.start_time && old.stop_time == run.stop_time);
            if !dup {
                list.push(run);
            }
        }
    }

    for (uuid, logs) in other.launch_logs {
        let list = count.launch_logs.entry(uuid).or_default();
        for log in logs {
            let dup = list
                .iter()
                .any(|old| old.time == log.time && old.error == log.error);
            if !dup {
                list.push(log);
            }
        }
    }

    for (uuid, name) in other.game_names {
        count.game_names.entry(uuid).or_insert(name);
    }

    drop(count);

    save();
    Ok(())
}

/// 合并迁移时留下的统计旁路文件
///
/// ColorMC 的 `count.dat` 不会直接覆盖 M²L 的统计，迁移时先落到
/// [`names::COUNT_IMPORT_FILE`]，下次启动读完自己的统计后再合并。
///
/// - `dir`: 运行目录
pub(super) fn merge_pending(dir: &Path) {
    let file = dir.join(names::COUNT_IMPORT_FILE);
    if !file.is_file() {
        return;
    }
    match merge_file(&file) {
        // 自己那份统计没读出来时不会落盘，旁路文件也得留着，下次启动再合
        Ok(()) if READ_OK.load(Ordering::Relaxed) => {
            let _ = std::fs::remove_file(&file);
        }
        Ok(()) => {}
        // 合并不了就把旁路文件留着，下次启动再试，别把数据删了
        Err(err) => mml_log::error_type(err),
    }
}
