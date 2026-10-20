//! 游戏启动统计
//! 统计启动器的启动次数、成功/失败次数、总游戏时长以及每次启动的时间段
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration as StdDuration,
};

use chrono::{DateTime, Local, LocalResult, TimeDelta, TimeZone, Utc};
use mml_names::{
    i18,
    i18_items::{error_type::CoreResult, thread_type::ThreadType},
    names,
};
use mml_nbt::{
    NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types::{NbtByte, NbtCompound, NbtList, NbtLong, NbtString},
};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::{get_instance, get_instances, launcher::game_time_obj::GameTimeObj};

/// 统计数据共享句柄
pub type GameCountObj = Arc<RwLock<CountObj>>;

/// 统计数据文件路径
static COUNT_FILE: OnceLock<PathBuf> = OnceLock::new();
/// 统计线程运行标志
static IS_RUN: AtomicBool = AtomicBool::new(false);
/// 是否正在保存数据
static IS_SAVE: AtomicBool = AtomicBool::new(false);

/// 统计数据
static COUNT: LazyLock<GameCountObj> = LazyLock::new(|| Arc::new(RwLock::new(CountObj::default())));

/// 正在运行的游戏及其上次统计时间
static TIME_LIST: LazyLock<Mutex<HashMap<Uuid, DateTime<Local>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 本次运行累积的游戏时长
static SPAN_TIME_LIST: LazyLock<Mutex<HashMap<Uuid, TimeDelta>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 运行中游戏的启动数据缓存
static LAUNCH_DATA_MAP: LazyLock<Mutex<HashMap<Uuid, GameTimeObj>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 落盘到 COUNT_DATA_FILE 的统计总表
#[derive(Clone, Debug)]
pub struct CountObj {
    /// 累计启动次数，含成功与失败
    pub launch_count: i64,
    /// 启动成功次数
    pub launch_done_count: i64,
    /// 启动失败次数
    pub launch_error_count: i64,
    /// 所有实例累计的游戏时长
    pub all_time: TimeDelta,
    /// 每个实例的运行时间段，按 uuid 分组
    pub game_runs: HashMap<Uuid, Vec<GameTimeData>>,
    /// 每个实例的启动记录，按 uuid 分组
    pub launch_logs: HashMap<Uuid, Vec<LaunchLogData>>,
    /// 实例名快照，按 uuid 分组；实例删掉后仍能显示名字
    pub game_names: HashMap<Uuid, String>,
}

impl Default for CountObj {
    /// 全字段取默认值
    fn default() -> Self {
        Self {
            launch_count: 0,
            launch_done_count: 0,
            launch_error_count: 0,
            all_time: TimeDelta::zero(),
            game_runs: HashMap::new(),
            launch_logs: HashMap::new(),
            game_names: HashMap::new(),
        }
    }
}

/// 一次游戏运行的时间段
#[derive(Clone, Debug)]
pub struct GameTimeData {
    /// 是否仍在运行，关闭后置 `false`
    pub now: bool,
    /// 开始时间，本地时区
    pub start_time: DateTime<Local>,
    /// 结束时间，本地时区；仍在运行时等于开始时间
    pub stop_time: DateTime<Local>,
}

/// 一条启动记录
#[derive(Clone, Debug)]
pub struct LaunchLogData {
    /// 记录时间，本地时区
    pub time: DateTime<Local>,
    /// 是否为错误日志；当前与原版一致，失败也记 `false`
    pub error: bool,
}

/// 初始化游戏统计
///
/// - `dir`: 运行路径，统计文件落在它下面
pub fn init<P: AsRef<Path>>(dir: P) {
    let _ = COUNT_FILE.get_or_init(|| dir.as_ref().join(names::COUNT_DATA_FILE));

    if IS_RUN.swap(true, Ordering::Relaxed) {
        return;
    }

    thread::Builder::new()
        .name(i18::get_thread(ThreadType::GameCount))
        .spawn(run)
        .unwrap();

    read();
}

/// 停止统计线程
///
/// 退出 / 重启时调用，必须**同步**落盘：异步线程可能还没写完就被进程结束，
/// 直接写一半的 NBT 文件会坏。若已有一次异步保存在写，先等它收尾。
pub fn stop() {
    IS_RUN.store(false, Ordering::Relaxed);

    // 还在跑的会话先结掉，否则落盘时 stop_time 会停在开始时间，这次运行等于没时长
    let running: Vec<Uuid> = TIME_LIST.lock().unwrap().keys().copied().collect();
    for uuid in running {
        close_run(&uuid);
    }

    let start = std::time::Instant::now();
    while IS_SAVE.load(Ordering::SeqCst) && start.elapsed() < StdDuration::from_secs(2) {
        thread::sleep(StdDuration::from_millis(10));
    }
    save();
}

/// 获取统计数据
///
/// # 返回值
///
/// 返回统计数据共享句柄
pub fn get_count() -> GameCountObj {
    COUNT.clone()
}

/// 读取统计数据
///
/// 文件不存在或解析失败时退回空统计，错误只记日志不向外抛。
fn read() {
    let Some(file) = COUNT_FILE.get().cloned() else {
        return;
    };

    if !file.exists() {
        {
            let mut count = COUNT.write().unwrap();
            *count = CountObj::default();
        }
    } else {
        let result = (|| -> CoreResult<()> {
            let mut stream = path_helper::open_read(&file)?;
            let nbt_file = NbtFile::read(&mut stream)?;
            let Some(nbt) = nbt_file.nbt.as_compound() else {
                return Ok(());
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

            *COUNT.write().unwrap() = count;
            Ok(())
        })();

        if let Err(err) = result {
            mml_log::error_type(err);
        }
    }

    // 使用当前所有实例刷新游戏名字
    {
        let instances = get_instances();
        let mut count = COUNT.write().unwrap();
        for game in instances {
            let read = game.read().unwrap();
            count.game_names.insert(read.uuid, read.name.clone());
        }
    }

    save();
}

/// 统计线程主体：每 100 毫秒累加一次运行中实例的时长
fn run() {
    let mut a: u32 = 0;
    while IS_RUN.load(Ordering::Relaxed) {
        thread::sleep(StdDuration::from_millis(100));

        let now = Local::now();
        let keys: Vec<Uuid> = TIME_LIST.lock().unwrap().keys().copied().collect();

        let should_save = a >= 10;
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

            {
                let mut span_list = SPAN_TIME_LIST.lock().unwrap();
                if let Some(value) = span_list.get_mut(&uuid) {
                    *value += span;
                }
            }

            {
                let mut data_list = LAUNCH_DATA_MAP.lock().unwrap();
                if let Some(data) = data_list.get_mut(&uuid) {
                    data.game_time += span;
                }
            }

            {
                let mut count = COUNT.write().unwrap();
                count.all_time += span;
            }

            // 每 10 次统计约 1 秒，保存一次启动数据
            if should_save {
                let data = {
                    let data_list = LAUNCH_DATA_MAP.lock().unwrap();
                    data_list.get(&uuid).map(copy_game_time)
                };
                if let Some(data) = data
                    && let Some(game) = get_instance(&uuid)
                {
                    let read = game.read().unwrap();
                    read.save_launch_count_data(&data);
                }
            }
        }

        a = if should_save { 0 } else { a + 1 };
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
fn save() {
    if IS_SAVE.swap(true, Ordering::SeqCst) {
        return;
    }

    let result = (|| -> CoreResult<()> {
        let Some(file) = COUNT_FILE.get().cloned() else {
            return Ok(());
        };

        let count = COUNT.read().unwrap();

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

        let nbt_file = NbtFile::new(nbt.to_nbt(), CompressType::GZip);
        let stream = &mut path_helper::open_write(&file)?;
        nbt_file.write(stream)?;

        Ok(())
    })();

    IS_SAVE.store(false, Ordering::SeqCst);

    if let Err(err) = result {
        mml_log::error_type(err);
    }
}

/// 游戏实例启动完毕
///
/// - `uuid`: 游戏实例标识
pub fn launch_done(uuid: &Uuid) {
    let now = Local::now();
    {
        let mut time_list = TIME_LIST.lock().unwrap();
        time_list.insert(*uuid, now);
    }
    {
        let mut span_list = SPAN_TIME_LIST.lock().unwrap();
        span_list.insert(*uuid, TimeDelta::zero());
    }
    {
        let mut data_map = LAUNCH_DATA_MAP.lock().unwrap();
        let game = get_instance(uuid);
        let mut data = match &game {
            Some(game) => game.read().unwrap().read_launch_count_data(),
            None => GameTimeObj::default(),
        };
        // 上次启动时间与 ColorMC 一致：LaunchDone 里先写 LastTime，再落一份实例统计
        data.last_time = Local::now().fixed_offset();
        if let Some(game) = &game {
            game.read().unwrap().save_launch_count_data(&data);
        }
        data_map.insert(*uuid, data);
    }
    {
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
    }

    save_async();
}

/// 启动失败
///
/// - `uuid`: 游戏实例标识
pub fn launch_error(uuid: &Uuid) {
    let now = Local::now();
    {
        let mut count = COUNT.write().unwrap();
        count.launch_count += 1;
        count.launch_error_count += 1;

        // 与原版一致：错误日志同样记录为 Error=false
        let log = LaunchLogData {
            time: now,
            error: false,
        };
        count.launch_logs.entry(*uuid).or_default().push(log);
    }

    save_async();
}

/// 游戏实例关闭
///
/// - `uuid`: 游戏实例标识
pub fn game_close(uuid: &Uuid) {
    close_run(uuid);
    save_async();
}

/// 结掉一局运行：只改内存与实例的最后游玩，不负责落盘
///
/// - `uuid`: 游戏实例标识
fn close_run(uuid: &Uuid) {
    let now = Local::now();
    TIME_LIST.lock().unwrap().remove(uuid);

    if let Some(span) = SPAN_TIME_LIST.lock().unwrap().remove(uuid) {
        if let Some(game) = get_instance(uuid) {
            let data = {
                let mut data_map = LAUNCH_DATA_MAP.lock().unwrap();
                data_map.remove(uuid)
            };
            let mut data = match data {
                Some(data) => data,
                None => game.read().unwrap().read_launch_count_data(),
            };
            data.last_play = span;

            let read = game.read().unwrap();
            read.save_launch_count_data(&data);
        } else {
            LAUNCH_DATA_MAP.lock().unwrap().remove(uuid);
        }
    } else {
        LAUNCH_DATA_MAP.lock().unwrap().remove(uuid);
    }

    {
        let mut count = COUNT.write().unwrap();
        if let Some(list) = count.game_runs.get_mut(uuid) {
            if let Some(item) = list.iter_mut().find(|a| a.now) {
                item.now = false;
                item.stop_time = now;
            }
        }
    }
}

/// 复制一份启动数据
///
/// `GameTimeObj` 未实现 `Clone`，但其字段均为可复制类型，这里逐字段复制。
fn copy_game_time(data: &GameTimeObj) -> GameTimeObj {
    GameTimeObj {
        add_time: data.add_time,
        last_time: data.last_time,
        game_time: data.game_time,
        last_play: data.last_play,
    }
}

/// 将 .NET 刻度数转换为本地时间
///
/// ColorMC 存的是 `DateTime.Now.Ticks`，刻度里记的是**本地墙上时间**，不带时区；
/// 读回时先还原成同样的墙上时间，再挂到本地时区，不能再当 UTC 转一次 ——
/// 否则 22 点之后的记录会整体跨到第二天。
///
/// - `ticks`: .NET 刻度数，单位 100 纳秒
///
/// # 返回值
///
/// 返回本地时间；无效值返回当前时间
fn ticks_to_date(ticks: i64) -> DateTime<Local> {
    let mut secs = ticks / 10_000_000 - 62135596800;
    let mut nanos = (ticks % 10_000_000) * 100;
    if nanos < 0 {
        secs -= 1;
        nanos += 1_000_000_000;
    }
    let Some(utc) = DateTime::<Utc>::from_timestamp(secs, nanos as u32) else {
        return Local::now();
    };
    let naive = utc.naive_utc();
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(date) => date,
        // 夏令时回拨的重叠时刻取较早的一次，与 .NET 的 DateTime 行为一致
        LocalResult::Ambiguous(first, _) => first,
        // 夏令时跳过的时刻在本地时区不存在：按 UTC 偏移兜底，至少不 panic
        LocalResult::None => utc.with_timezone(&Local),
    }
}

/// 将本地时间转换为 .NET 刻度数
///
/// 与 ColorMC 同一口径：写的是本地墙上时间，不是 UTC 时间戳。
///
/// - `date`: 本地时间
///
/// # 返回值
///
/// 返回 .NET 刻度数，单位 100 纳秒
fn date_to_ticks(date: DateTime<Local>) -> i64 {
    let local_secs = date.timestamp() + date.offset().local_minus_utc() as i64;
    (local_secs + 62135596800) * 10_000_000 + date.timestamp_subsec_nanos() as i64 / 100
}

/// 将时长转换为 .NET 刻度数
///
/// - `duration`: 时长
///
/// # 返回值
///
/// 返回 .NET 刻度数，单位 100 纳秒
fn duration_to_ticks(duration: TimeDelta) -> i64 {
    duration.num_seconds().saturating_mul(10_000_000) + (duration.subsec_nanos() as i64) / 100
}

/// 将 .NET 刻度数转换为时长
///
/// - `ticks`: .NET 刻度数，单位 100 纳秒
///
/// # 返回值
///
/// 返回时长；负数或溢出返回零时长
fn duration_from_ticks(ticks: i64) -> TimeDelta {
    if ticks < 0 {
        return TimeDelta::zero();
    }
    let secs = ticks / 10_000_000;
    let nanos = ((ticks % 10_000_000) * 100) as u32;
    TimeDelta::new(secs, nanos).unwrap_or(TimeDelta::zero())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// 跨零点的刻度往返：日期与时刻都不能被时区换算挪走
    #[test]
    fn ticks_round_trip_across_midnight() {
        let before = Local
            .with_ymd_and_hms(2026, 9, 19, 23, 59, 58)
            .single()
            .unwrap();
        let after = Local
            .with_ymd_and_hms(2026, 9, 20, 0, 0, 3)
            .single()
            .unwrap();

        let back_before = ticks_to_date(date_to_ticks(before));
        let back_after = ticks_to_date(date_to_ticks(after));

        assert_eq!(back_before.naive_local(), before.naive_local());
        assert_eq!(back_after.naive_local(), after.naive_local());
        assert_eq!((back_after - back_before).num_seconds(), 5);
    }

    /// 读 ColorMC 生成的 count.dat：里面有一条 09-19 13:45 → 09-20 00:35 的记录，
    /// 按时区换算成 UTC 再转回来会整体偏一天，必须按本地墙上时间原样还原
    #[test]
    fn reads_color_mc_count_across_midnight() {
        let bytes = include_bytes!("../tests/count/count.dat");
        let file = NbtFile::read(&mut Cursor::new(bytes.as_slice())).unwrap();
        let nbt = file.nbt.as_compound().unwrap();

        assert_eq!(nbt.get_long("LaunchCount"), Some(1533));

        let runs = nbt.get_list("GameRuns").unwrap();
        let mut cross = None;
        for item in runs.iter() {
            let Some(com) = item.as_compound() else {
                continue;
            };
            if com.get_string("Key").as_deref() != Some("03962c67-c32d-40e8-81f3-42640ca062d4") {
                continue;
            }
            let Some(list) = com.get_list("List") else {
                continue;
            };
            for run in list.iter() {
                let Some(com1) = run.as_compound() else {
                    continue;
                };
                let start = ticks_to_date(com1.get_long("StartTime").unwrap_or(0));
                let stop = ticks_to_date(com1.get_long("StopTime").unwrap_or(0));
                if start.date_naive() != stop.date_naive() {
                    cross = Some((start, stop));
                }
            }
        }

        let (start, stop) = cross.expect("fixture 应有一条跨天记录");
        assert_eq!(
            start.naive_local().format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-09-19 13:45:58"
        );
        assert_eq!(
            stop.naive_local().format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-09-20 00:35:16"
        );
        assert!(stop > start);
    }
}
