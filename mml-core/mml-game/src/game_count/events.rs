//! 启动与退出事件
//!
//! 记录一次启动的开始、成功、失败与结束。

use chrono::Local;
use uuid::Uuid;

use super::save::save_async;
use super::{COUNT, GameTimeData, LaunchLogData, TIME_LIST};

/// 游戏实例启动完毕
///
/// - `uuid`: 游戏实例标识
pub fn launch_done(uuid: &Uuid) {
    let now = Local::now();
    let mut time_list = TIME_LIST.lock().unwrap();
    time_list.insert(*uuid, now);

    drop(time_list);
    let mut count = COUNT.write().unwrap();
    count.launch_count += 1;
    count.launch_done_count += 1;

    let time = GameTimeData {
        now: true,
        start_time: now,
        stop_time: now,
    };
    count.game_runs.entry(*uuid).or_default().push(time);

    let log = LaunchLogData {
        time: now,
        error: false,
    };
    count.launch_logs.entry(*uuid).or_default().push(log);

    drop(count);

    save_async();
}

/// 启动失败
///
/// - `uuid`: 游戏实例标识
pub fn launch_error(uuid: &Uuid) {
    let now = Local::now();
    let mut count = COUNT.write().unwrap();
    count.launch_count += 1;
    count.launch_error_count += 1;

    // 与原版一致：错误日志同样记录为 Error=false
    let log = LaunchLogData {
        time: now,
        error: false,
    };
    count.launch_logs.entry(*uuid).or_default().push(log);

    drop(count);

    save_async();
}

/// 游戏实例关闭
///
/// - `uuid`: 游戏实例标识
pub fn game_close(uuid: &Uuid) {
    close_run(uuid);
    save_async();
}

/// 结掉一局运行：只改内存，不负责落盘
///
/// - `uuid`: 游戏实例标识
pub(super) fn close_run(uuid: &Uuid) {
    let now = Local::now();
    TIME_LIST.lock().unwrap().remove(uuid);

    let mut count = COUNT.write().unwrap();
    if let Some(list) = count.game_runs.get_mut(uuid) {
        if let Some(item) = list.iter_mut().find(|a| a.now) {
            item.now = false;
            item.stop_time = now;
        }
    }
}
