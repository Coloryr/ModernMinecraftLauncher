//! 启动器日志系统
//!
//! 本模块实现了启动器的异步日志系统，支持多级别日志记录和
//! 基于信号量的后台写入。
//!
//! # 日志级别
//!
//! | 级别 | 函数 | 用途 |
//! |------|------|------|
//! | Info | [`info()`] / [`info_type()`] | 一般信息 |
//! | Warn | [`warn()`] | 警告信息 |
//! | Error | [`error()`] / [`error_type()`] | 错误信息 |
//! | Fault | [`failt()`] | 严重错误/崩溃 |
//!
//! # 工作原理
//!
//! 1. 业务代码调用日志函数，将日志项加入无锁队列 [`SegQueue`]
//! 2. 通过信号量唤醒后台日志线程
//! 3. 日志线程批量将队列中的日志写入文件，带缓冲 [`BufWriter`]
//!
//! # 生命周期
//!
//! - [`start()`] — 程序初始化时调用，启动日志线程
//! - [`stop()`] — 程序退出时调用，停止线程并执行最终刷写

pub mod log_item;

use crossbeam_queue::SegQueue;
use mml_names::{
    i18,
    i18_items::{
        error_type::{CoreResult, ErrorType},
        info_type::InfoType,
        panic_type::PanicType,
        thread_type::ThreadType,
    },
    names,
};
use semrs::Semaphore;

use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self},
};

use crate::log_item::{LogItem, LogLevel};

/// 日志写入队列，无锁分段队列，支持高并发推送
static QUEUE: RwLock<SegQueue<LogItem>> = RwLock::new(SegQueue::new());
/// 日志文件写入流，带缓冲
static STREAM: OnceLock<BufWriter<File>> = OnceLock::new();
/// 日志线程运行标志
static IS_RUN: AtomicBool = AtomicBool::new(true);
/// 唤醒日志线程的信号量
static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// 启动日志系统
///
/// 创建日志文件，追加模式，并启动后台日志写入线程。
///
/// - `local`: 日志文件存储目录
///
/// # 返回值
///
/// 日志文件打不开时返回 `ErrorType::Panic`；重复调用会 panic
pub fn start<P: AsRef<Path>>(local: P) -> CoreResult<()> {
    SEM.get_or_init(|| Arc::new(Semaphore::new(0)));

    let log_path = local.as_ref().join(names::LOG_FILE);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(&log_path)
        .map_err(|err| {
            ErrorType::Panic(PanicType::LogOpenFail(
                log_path.display().to_string(),
                err.to_string(),
            ))
        })?;

    STREAM.set(BufWriter::new(file)).unwrap();

    thread::Builder::new()
        .name(i18::get_thread(ThreadType::LogThread))
        .spawn(|| {
            while IS_RUN.load(Ordering::Acquire) {
                SEM.get().unwrap().down();
                save();
            }
            // 退出前最后一次刷写
            save();
        })
        .unwrap();

    Ok(())
}

/// 停止日志系统
///
/// 设置运行标志为 false，线程将在下一次被唤醒后退出。
pub fn stop() {
    IS_RUN.store(false, Ordering::Release);
}

/// 将队列中的所有日志写入文件
fn save() {
    let log = QUEUE.read().unwrap();
    let mut file = STREAM.get().unwrap().get_ref();

    while !log.is_empty() {
        let item = log.pop();
        if item.is_some() {
            let item1 = item.unwrap();
            file.write_fmt(format_args!(
                "[{}][{}]{}{}",
                item1.get_time(),
                item1.get_level(),
                item1.log,
                mml_names::get_line_ending()
            ))
            .unwrap();
            file.flush().unwrap();
        }
    }
}

/// 记录信息级别日志
///
/// - `text`: 日志内容
pub fn info(text: String) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(text, LogLevel::Info));
    SEM.get().unwrap().up();
}

/// 记录信息级别日志，内容取自国际化类型
///
/// - `info`: 预定义的国际化信息类型
pub fn info_type(info: InfoType) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(i18::get_info(info), LogLevel::Info));
    SEM.get().unwrap().up();
}

/// 记录警告级别日志
///
/// - `text`: 日志内容
pub fn warn(text: String) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(text, LogLevel::Warn));
    SEM.get().unwrap().up();
}

/// 记录警告级别日志，内容取自国际化类型
///
/// - `info`: 预定义的国际化信息类型
pub fn warn_type(info: InfoType) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(i18::get_info(info), LogLevel::Warn));
    SEM.get().unwrap().up();
}

/// 记录错误级别日志
///
/// - `text`: 日志内容
pub fn error(text: String) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(text, LogLevel::Error));
    SEM.get().unwrap().up();
}

/// 记录错误级别日志，内容取自国际化错误类型
///
/// - `error`: 预定义的国际化错误类型
pub fn error_type(error: ErrorType) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(i18::get_error(error), LogLevel::Error));
    SEM.get().unwrap().up();
}

/// 记录严重错误 Fault 级别日志
///
/// 用于记录启动器崩溃等严重事件。
///
/// - `text`: 日志内容
pub fn failt(text: String) {
    QUEUE
        .write()
        .unwrap()
        .push(LogItem::new(text, LogLevel::Fault));
    SEM.get().unwrap().up();
}
