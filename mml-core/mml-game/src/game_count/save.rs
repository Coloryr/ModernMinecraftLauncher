//! 落盘
//!
//! 后台定时保存与退出时保存，写盘前先比对文件时间戳。

use std::{sync::atomic::Ordering, thread, time::Duration as StdDuration};

use chrono::{Local, TimeDelta};
use mml_names::{
    i18,
    i18_items::{error_type::CoreResult, info_type::InfoType, thread_type::ThreadType},
};
use mml_nbt::{
    NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types::{NbtByte, NbtCompound, NbtList, NbtLong, NbtString},
};
use mml_sys::path_helper;
use uuid::Uuid;

use super::lifecycle::{backup_file, disk_more_than, file_stamp};
use super::time::{date_to_ticks, duration_to_ticks};
use super::{COUNT, COUNT_FILE, FILE_STAMP, IS_RUN, IS_SAVE, READ_OK, TIME_LIST};

/// 统计线程主体：每 100 毫秒累加一次运行中实例的时长
pub(super) fn run() {
    while IS_RUN.load(Ordering::Relaxed) {
        thread::sleep(StdDuration::from_millis(100));

        let now = Local::now();
        let keys: Vec<Uuid> = TIME_LIST.lock().unwrap().keys().copied().collect();

        let mut total = TimeDelta::zero();
        for uuid in keys {
            let span = {
                let mut time_list = TIME_LIST.lock().unwrap();
                let Some(prev) = time_list.get_mut(&uuid) else {
                    continue;
                };
                let span = now - *prev;
                *prev = now;
                span
            };
            total += span;
        }

        if total > TimeDelta::zero() {
            COUNT.write().unwrap().all_time += total;
        }
    }
}

/// 异步保存统计数据
///
/// 用普通线程而不是 tokio 的 `spawn_blocking`：退出轮询线程与窗口关闭回调都是裸线程，
/// 没有 tokio 运行时，在那里调 `spawn_blocking` 会直接 panic。
pub fn save_async() {
    if IS_SAVE.load(Ordering::Relaxed) {
        return;
    }
    let _ = thread::Builder::new()
        .name(i18::get_thread(ThreadType::GameCount))
        .spawn(save);
}

/// 保存统计数据到 COUNT_DATA_FILE，重入时直接返回
pub(super) fn save() {
    if IS_SAVE.swap(true, Ordering::SeqCst) {
        return;
    }

    // 启动时没读到统计：这次运行整场不落盘，别把用户原来的文件盖掉
    if !READ_OK.load(Ordering::Relaxed) {
        IS_SAVE.store(false, Ordering::SeqCst);
        return;
    }

    // 文件在运行期间被外部换过（用户手动替换）：以外部为准，这次不写
    if let Some(old) = *FILE_STAMP.lock().unwrap()
        && let Some(file) = COUNT_FILE.get()
        && let Some(now) = file_stamp(file)
        && now != old
    {
        mml_log::info_type(InfoType::GameCountKeepOuter);
        IS_SAVE.store(false, Ordering::SeqCst);
        return;
    }

    let result = (|| -> CoreResult<()> {
        let Some(file) = COUNT_FILE.get().cloned() else {
            return Ok(());
        };

        let count = COUNT.read().unwrap();

        // 最后一道闸：统计只增不减，磁盘上那份更多就说明内存里这份是空的 / 旧的
        if let Some((disk, mem)) = disk_more_than(&file, &count) {
            mml_log::info_type(InfoType::GameCountKeepBigger { disk, mem });
            return Ok(());
        }

        let mut nbt = NbtCompound::new();
        nbt.data.insert(
            "LaunchCount".to_string(),
            NbtLong::new(count.launch_count).to_nbt(),
        );
        nbt.data.insert(
            "LaunchDoneCount".to_string(),
            NbtLong::new(count.launch_done_count).to_nbt(),
        );
        nbt.data.insert(
            "LaunchErrorCount".to_string(),
            NbtLong::new(count.launch_error_count).to_nbt(),
        );
        nbt.data.insert(
            "AllTime".to_string(),
            NbtLong::new(duration_to_ticks(count.all_time)).to_nbt(),
        );

        let mut game_runs = NbtList::new(NbtType::compound().get_num());
        for (key, list) in &count.game_runs {
            let mut com = NbtCompound::new();
            com.data
                .insert("Key".to_string(), NbtString::new(key.to_string()).to_nbt());

            let mut list1 = NbtList::new(NbtType::compound().get_num());
            for item in list {
                let mut com1 = NbtCompound::new();
                com1.data.insert(
                    "StartTime".to_string(),
                    NbtLong::new(date_to_ticks(item.start_time)).to_nbt(),
                );
                com1.data.insert(
                    "StopTime".to_string(),
                    NbtLong::new(date_to_ticks(item.stop_time)).to_nbt(),
                );
                list1.add_item(com1.to_nbt());
            }
            com.data.insert("List".to_string(), list1.to_nbt());
            game_runs.add_item(com.to_nbt());
        }
        nbt.data.insert("GameRuns".to_string(), game_runs.to_nbt());

        let mut launch_logs = NbtList::new(NbtType::compound().get_num());
        for (key, list) in &count.launch_logs {
            let mut com = NbtCompound::new();
            com.data
                .insert("Key".to_string(), NbtString::new(key.to_string()).to_nbt());

            let mut list1 = NbtList::new(NbtType::compound().get_num());
            for item in list {
                let mut com1 = NbtCompound::new();
                com1.data.insert(
                    "Time".to_string(),
                    NbtLong::new(date_to_ticks(item.time)).to_nbt(),
                );
                com1.data.insert(
                    "Error".to_string(),
                    NbtByte::new(if item.error { 1 } else { 0 }).to_nbt(),
                );
                list1.add_item(com1.to_nbt());
            }
            com.data.insert("List".to_string(), list1.to_nbt());
            launch_logs.add_item(com.to_nbt());
        }
        nbt.data
            .insert("LaunchLogs".to_string(), launch_logs.to_nbt());

        let mut game_names = NbtCompound::new();
        for (key, value) in &count.game_names {
            game_names
                .data
                .insert(key.to_string(), NbtString::new(value.clone()).to_nbt());
        }
        nbt.data
            .insert("GameNames".to_string(), game_names.to_nbt());

        drop(count);

        // 覆盖前把上一版留成 count.dat.bak：真被空统计盖掉时还能捞回来
        if file.is_file() {
            backup_file(&file);
        }

        let nbt_file = NbtFile::new(nbt.to_nbt(), CompressType::GZip);
        let stream = &mut path_helper::open_write(&file)?;
        nbt_file.write(stream)?;
        // 记下自己这次写出去的指纹，下次保存才分得清"外部改过"与"自己写的"
        let stamp = file_stamp(&file);
        *FILE_STAMP.lock().unwrap() = stamp;
        mml_log::info_type(InfoType::GameCountSave {
            size: stamp.map(|(_, len)| len).unwrap_or(0),
        });

        Ok(())
    })();

    IS_SAVE.store(false, Ordering::SeqCst);

    if let Err(err) = result {
        mml_log::error_type(err);
    }
}
