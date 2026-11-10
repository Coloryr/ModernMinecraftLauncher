//! 统计的启停与读盘
//!
//! 起后台统计线程、读盘、文件时间戳与备份。

use std::{
    path::Path,
    sync::atomic::Ordering,
    thread,
    time::{Duration as StdDuration, SystemTime},
};

use mml_names::{
    i18,
    i18_items::{
        error_type::{CoreResult, ErrorType},
        info_type::InfoType,
        thread_type::ThreadType,
    },
    names,
};
use uuid::Uuid;

use crate::get_instances;

use super::events::close_run;
use super::file::{merge_pending, parse_file};
use super::save::{run, save, save_async};
use super::{
    COUNT, COUNT_FILE, CountObj, FILE_STAMP, GameCountObj, IS_RUN, IS_SAVE, READ_OK, TIME_LIST,
};

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
pub(super) fn read() {
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
pub(super) fn read_with_retry(file: &Path) -> CoreResult<CountObj> {
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
pub(super) fn file_stamp(file: &Path) -> Option<(SystemTime, u64)> {
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
pub(super) fn disk_more_than(file: &Path, count: &CountObj) -> Option<(i64, i64)> {
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
pub(super) fn backup_file(file: &Path) {
    let bak = file.with_file_name(format!("{}.bak", names::COUNT_DATA_FILE));
    let _ = std::fs::copy(file, bak);
}
