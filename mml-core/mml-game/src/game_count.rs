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
    time::{Duration as StdDuration, SystemTime},
};

use chrono::{DateTime, Local, LocalResult, TimeDelta, TimeZone, Utc};
use mml_names::{
    i18,
    i18_items::{
        error_type::{CoreResult, ErrorType},
        info_type::InfoType,
        thread_type::ThreadType,
    },
    names,
};
use mml_nbt::{
    NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types::{NbtByte, NbtCompound, NbtList, NbtLong, NbtString},
};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::get_instances;

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

/// 上次读 / 写统计文件时的指纹：修改时间 + 长度
///
/// 用户可能在启动器运行期间手动替换 `count.dat`；落盘前对一下指纹，
/// 变了就不写，免得把人家的文件盖掉。
static FILE_STAMP: LazyLock<Mutex<Option<(SystemTime, u64)>>> = LazyLock::new(|| Mutex::new(None));

/// 启动时统计文件是否读成功
///
/// 读不出来时（文件坏了、或者正被别的程序替换）这次运行**不落盘**，
/// 否则退出时的保存会把用户原来的文件盖掉。
static READ_OK: AtomicBool = AtomicBool::new(true);

/// 正在运行的游戏及其上次统计时间
static TIME_LIST: LazyLock<Mutex<HashMap<Uuid, DateTime<Local>>>> =
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
    let dir = dir.as_ref().to_path_buf();
    let _ = COUNT_FILE.get_or_init(|| dir.join(names::COUNT_DATA_FILE));

    if IS_RUN.swap(true, Ordering::Relaxed) {
        return;
    }

    thread::Builder::new()
        .name(i18::get_thread(ThreadType::GameCount))
        .spawn(run)
        .unwrap();

    read();
    // 迁移过来的统计（ColorMC 的 count.dat）在读完自己的统计之后再合并
    merge_pending(&dir);
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

/// 记住实例名，实例删除后统计窗口仍能显示原来的名字
///
/// - `uuid`: 实例 uuid
/// - `name`: 实例名
pub(crate) fn remember_name(uuid: &Uuid, name: &str) {
    let mut count = COUNT.write().unwrap();
    if count
        .game_names
        .get(uuid)
        .is_some_and(|old| old.as_str() == name)
    {
        return;
    }
    count.game_names.insert(*uuid, name.to_string());
    drop(count);
    save_async();
}

/// 读取统计数据
///
/// 文件不存在或解析失败时退回空统计，错误只记日志不向外抛。
fn read() {
    let Some(file) = COUNT_FILE.get().cloned() else {
        return;
    };

    // 解析失败时不落盘：文件可能正被别的程序替换（读到的是一半），
    // 这时写回去就把用户原来的数据毁了，见下面的 backup_file
    let mut loaded = true;
    if !file.exists() {
        let mut count = COUNT.write().unwrap();
        *count = CountObj::default();
    } else {
        match read_with_retry(&file) {
            Ok(data) => {
                // 读到的是哪一份文件要留痕：统计被空文件盖掉时，这行能直接看出读到的内容
                mml_log::info_type(InfoType::GameCountLoad {
                    size: std::fs::metadata(&file).map(|meta| meta.len()).unwrap_or(0),
                    launch: data.launch_count,
                    runs: data.game_runs.values().map(|list| list.len()).sum(),
                });
                // 指纹已经在 read_with_retry 里按"读之前那一版"记好了
                *COUNT.write().unwrap() = data;
            }
            Err(err) => {
                loaded = false;
                READ_OK.store(false, Ordering::Relaxed);
                mml_log::error_type(err);
                backup_file(&file);
            }
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

    if loaded {
        save();
    }
}

/// 读统计文件，失败时短暂重试
///
/// 用户手动替换 `count.dat` 时，文件可能还没复制完：这时解压会报
/// "incomplete deflate stream"。等一会儿再读一次，别把这种情况当成坏文件。
///
/// - `file`: 统计文件
///
/// # 返回值
///
/// 解析出的统计；重试后仍读不出来返回最后一次的错误
fn read_with_retry(file: &Path) -> CoreResult<CountObj> {
    let mut last = None;
    // 5 次、每次间隔 400ms：杀毒软件扫描新文件时，复制到一半的状态可能持续一两秒
    for attempt in 0..5 {
        if attempt > 0 {
            thread::sleep(StdDuration::from_millis(400));
        }
        // 先记指纹再解析，两次一致才算这一份：用户可能正好在启动器读文件的同时把
        // count.dat 换掉 —— 读到旧内容、却把新文件的指纹记成"自己读的那一版"，
        // 随后那次启动保存就会拿空统计把刚换进来的文件盖掉
        let before = file_stamp(file);
        match parse_file(file) {
            Ok(data) => {
                if before.is_some() && before == file_stamp(file) {
                    *FILE_STAMP.lock().unwrap() = before;
                    return Ok(data);
                }
                last = Some(ErrorType::NbtReadError);
            }
            Err(err) => last = Some(err),
        }
    }
    Err(last.unwrap_or(ErrorType::NbtReadError))
}

/// 统计文件的指纹：修改时间 + 长度
///
/// - `file`: 统计文件
///
/// # 返回值
///
/// 文件存在且拿得到修改时间时返回指纹，否则 `None`
fn file_stamp(file: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// 磁盘上那份统计是否比内存里这份多
///
/// 启动次数与运行段只增不减，所以"磁盘上的更多"只可能意味着内存里这份是空的或旧的
/// （比如启动器读文件的时候，文件正好被用户换掉了）。这时候写回去就是拿空统计盖掉
/// 用户的数据，宁可这一次不保存。
///
/// - `file`: 统计文件
/// - `count`: 内存里的统计
///
/// # 返回值
///
/// 磁盘上那份更多时返回它的累计启动次数与内存里这份的次数，否则 `None`
fn disk_more_than(file: &Path, count: &CountObj) -> Option<(i64, i64)> {
    // 读不出来就当没有证据：坏文件本来就该被这份好数据覆盖
    let disk = parse_file(file).ok()?;
    let disk_runs: usize = disk.game_runs.values().map(|list| list.len()).sum();
    let mem_runs: usize = count.game_runs.values().map(|list| list.len()).sum();
    if disk.launch_count > count.launch_count || disk_runs > mem_runs {
        Some((disk.launch_count, count.launch_count))
    } else {
        None
    }
}

/// 把统计文件备份成 `count.dat.bak`
///
/// 两处用到：读不出来时留一份坏文件方便排查；每次覆盖前留一份上一版，
/// 万一还是被空统计盖掉，用户能手动换回来。
///
/// - `file`: 统计文件
fn backup_file(file: &Path) {
    let bak = file.with_file_name(format!("{}.bak", names::COUNT_DATA_FILE));
    let _ = std::fs::copy(file, bak);
}

/// 解析一份统计文件
///
/// 结构与 ColorMC 的 `count.dat` 是同一套 NBT，见 [`CountObj`]。
///
/// - `file`: 统计文件
///
/// # 返回值
///
/// 解析出的统计；文件读不出来或不是 NBT 时返回错误
fn parse_file(file: &Path) -> CoreResult<CountObj> {
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
    {
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
    }

    save();
    Ok(())
}

/// 合并迁移时留下的统计旁路文件
///
/// ColorMC 的 `count.dat` 不会直接覆盖 M²L 的统计，迁移时先落到
/// [`names::COUNT_IMPORT_FILE`]，下次启动读完自己的统计后再合并。
///
/// - `dir`: 运行目录
fn merge_pending(dir: &Path) {
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

/// 统计线程主体：每 100 毫秒累加一次运行中实例的时长
fn run() {
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
fn save() {
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

/// 结掉一局运行：只改内存，不负责落盘
///
/// - `uuid`: 游戏实例标识
fn close_run(uuid: &Uuid) {
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

    /// 磁盘上那份统计比内存里多时不能落盘
    ///
    /// 统计只增不减：内存里这份要是比磁盘上的少，说明读到的是空的 / 旧的那一份
    /// （文件在启动器读它的时候被换掉了），写回去就把用户的统计盖没了。
    #[test]
    fn disk_with_more_data_blocks_save() {
        let dir = mml_testutil::temp_path(format!("count-guard-{}", Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(names::COUNT_DATA_FILE);
        std::fs::write(&file, include_bytes!("../tests/count/count.dat")).unwrap();

        // 内存里是空的：磁盘上那份（1533 次启动）更多，必须拦住
        let empty = CountObj::default();
        let (disk, mem) = disk_more_than(&file, &empty).expect("磁盘上那份更多时应当拦住");
        assert_eq!(mem, 0);
        assert!(disk > 0);

        // 内存里就是这份文件读出来的：不拦
        let same = parse_file(&file).unwrap();
        assert!(disk_more_than(&file, &same).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 读不出来的统计文件不能被空统计覆盖
    ///
    /// 用户手动替换 `count.dat` 时，读到的可能是一半的文件；以前这里会退回空统计，
    /// 退出时再把空统计写回去，把用户原来的数据毁掉。
    #[test]
    fn unreadable_count_is_not_overwritten() {
        // 读失败要写日志，日志系统是单例，得先起来
        crate::test_support::boot();
        let dir = mml_testutil::temp_path(format!("count-broken-{}", Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(names::COUNT_DATA_FILE);
        // 半截 gzip：模拟复制到一半的文件
        std::fs::write(&file, [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]).unwrap();

        init(&dir);
        stop();

        assert_eq!(
            std::fs::read(&file).unwrap().len(),
            8,
            "读不出来的文件不该被覆盖"
        );
        assert!(dir.join(format!("{}.bak", names::COUNT_DATA_FILE)).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
