//! 下载器模块
//!
//! 本模块实现了启动器的文件下载引擎，支持多线程并发下载、
//! 断点续传、哈希校验、下载进度回调等功能。
//!
//! # 架构概述
//!
//! ```text
//! start_download_task()          —— 创建下载任务并入队
//!     │
//!     ▼
//! DownloadTask::new()            —— 将文件列表包装为任务
//!     │
//!     ▼
//! DownloadThread (× N)           —— N 个工作线程从队列取文件下载
//!     │
//!     ▼
//! download()                     —— 单文件下载流程，断点续传 + 哈希校验
//!     │
//!     ▼
//! later_tasks::unpack_native()   —— 下载后处理，解压 native 库等
//! ```
//!
//! # 核心类型
//!
//! | 类型 | 用途 |
//! |------|------|
//! | [`DownloadItem`] | 单个下载文件的状态跟踪 |
//! | [`DownloadTask`] | 一组下载文件的批量任务管理 |
//! | [`DownloadHandle`] | 任务句柄：查状态 / 取结果 / 取消 |
//! | [`DownloadThread`] | 下载工作线程封装 |
//! | [`IDownloadGui`] | UI 更新回调接口 |
//!
//! # 下载流程
//!
//! 1. 每个文件首先检查是否已存在且哈希匹配 → 跳过下载
//! 2. 支持 `Range` 断点续传，需服务器支持
//! 3. 下载完成后校验文件大小和哈希值
//! 4. 超过 5 次错误自动放弃当前文件
//! 5. 下载后支持解压 native 库和存档文件

pub mod download_item;
pub mod download_task;

mod download_thread;
pub mod later_tasks;

pub use download_task::TaskOutcome;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use mml_base::file_item::FileItemObj;
use mml_names::{i18_items::error_type::CoreResult, names};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    download_item::DownloadItem, download_task::DownloadTask, download_thread::DownloadThread,
};

/// 下载任务进度快照
pub struct TaskStateObj {
    /// 任务编号，全局唯一自增 ID
    pub id: u64,
    /// 下载进度，范围 0.0–100.0
    pub progress: f64,
}

/// 下载任务状态变更事件
pub enum DownloadTaskState {
    /// 新任务已添加，携带任务编号
    AddTask(u64),
    /// 任务已移除，携带任务编号
    RemoveTask(u64),
    /// 任务进度更新
    UpdateTask(TaskStateObj),
}

/// 下载器 UI 回调接口
///
/// 实现此 trait 以接收下载引擎的状态更新通知。
pub trait IDownloadGui {
    /// 单个文件下载状态更新
    ///
    /// - `thread`: 下载线程序号
    /// - `file`: 当前正在下载的文件信息
    fn update(&self, thread: u32, file: &Arc<DownloadItem>);

    /// 下载任务进度更新
    ///
    /// - `state`: 任务状态变更类型
    fn update_task(&self, state: DownloadTaskState);
}

/// 下载项目，关联任务和具体文件
pub(crate) struct DownloadObj {
    /// 所属下载任务
    pub task: Arc<DownloadTask>,
    /// 当前下载的文件项
    pub item: Arc<DownloadItem>,
}

/// 下载线程列表
static THREADS: RwLock<Vec<DownloadThread>> = RwLock::new(Vec::new());
/// 下载任务队列
static TASKS: RwLock<Vec<Arc<DownloadTask>>> = RwLock::new(Vec::new());

/// 下载器 UI 回调，全局单例
static DOWNLOAD_GUI: OnceLock<Box<dyn IDownloadGui + Sync + Send>> = OnceLock::new();

/// 下载器停止标志
static STOP: AtomicBool = AtomicBool::new(false);

/// 全局暂停标志，暂停期间不分配任何文件，后续新增任务同样暂停
static PAUSED: AtomicBool = AtomicBool::new(false);

/// 自增任务编号
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// 临时下载文件夹路径
static CACHE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 文件下载路径
static DOWNLOAD_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 初始化下载文件夹
///
/// - `path`: 程序运行根目录，下载文件夹将创建在 `{path}/downloads/` 下
///
/// # 返回值
///
/// 初始化成功返回 `Ok(())`；创建目录失败返回对应错误
pub fn init<P: AsRef<Path>>(path: P) -> CoreResult<()> {
    let cache = CACHE_PATH.get_or_init(|| path.as_ref().join(names::CACHE_DIR));
    if !cache.exists() {
        path_helper::create_dir_all(cache)?;
    } else {
        // 启动时清空下载临时目录：目录里只会有上次运行残留的临时文件，不清会一直占空间。
        // 但必须跳过 webview 子目录：界面的 EBWebView 档案，含 localStorage 与缓存，
        // 也放在 cache 下，清了等于每次启动都重装一遍界面状态，见 names::WEBVIEW_DIR。
        if let Ok(entries) = std::fs::read_dir(cache) {
            for entry in entries.flatten() {
                if entry.file_name() == names::WEBVIEW_DIR {
                    continue;
                }
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(path);
                } else {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    let dir = DOWNLOAD_PATH.get_or_init(|| path.as_ref().join(names::DOWNLOAD_DIR));
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    Ok(())
}

/// 获取下载临时文件夹路径
///
/// # 返回值
///
/// 下载临时文件夹路径；未 `init` 会 panic
pub fn get_cache_path() -> PathBuf {
    CACHE_PATH.get().unwrap().clone()
}

/// 获取下载文件夹路径
///
/// # 返回值
///
/// 下载文件夹路径；未 `init` 会 panic
pub fn get_download_path() -> PathBuf {
    DOWNLOAD_PATH.get().unwrap().clone()
}

/// 生成一个随机的临时文件路径，UUID v4
///
/// # 返回值
///
/// 返回下载临时目录下尚不存在的路径，保证不与已有文件冲突
pub fn gen_temp_file() -> PathBuf {
    loop {
        let file = CACHE_PATH.get().unwrap().join(Uuid::new_v4().to_string());
        if file.exists() {
            continue;
        }
        return file;
    }
}

/// 设置下载器 UI 回调
///
/// 应在启动下载前调用一次。
///
/// - `gui`: 下载器 UI 回调实现，全局只认第一次传入的那个
pub fn set_gui_handel(gui: Box<dyn IDownloadGui + Sync + Send>) {
    DOWNLOAD_GUI.get_or_init(|| gui);
}

/// 通知 UI：文件下载进度更新
///
/// - `thread`: 下载线程序号
/// - `file`: 当前正在下载的文件信息
pub(crate) fn update(thread: u32, file: &Arc<DownloadItem>) {
    if let Some(gui) = DOWNLOAD_GUI.get() {
        gui.as_ref().update(thread, file);
    }
}

/// 通知 UI：任务进度更新
///
/// - `id`: 任务编号
/// - `progress`: 任务进度，范围 0.0–100.0
pub(crate) fn update_task(id: u64, progress: f64) {
    if let Some(gui) = DOWNLOAD_GUI.get() {
        gui.as_ref()
            .update_task(DownloadTaskState::UpdateTask(TaskStateObj { id, progress }));
    }
}

/// 通知 UI：新任务已添加
///
/// - `id`: 任务编号
pub(crate) fn add_task(id: u64) {
    if let Some(gui) = DOWNLOAD_GUI.get() {
        gui.as_ref().update_task(DownloadTaskState::AddTask(id));
    }
}

/// 通知 UI：任务已移除
///
/// - `id`: 任务编号
pub(crate) fn remove_task(id: u64) {
    if let Some(gui) = DOWNLOAD_GUI.get() {
        gui.as_ref().update_task(DownloadTaskState::RemoveTask(id));
    }
}

/// 生成下一个任务编号，原子自增
///
/// # 返回值
///
/// 返回全局唯一且递增的任务编号，从 1 开始
pub(crate) fn gen_task_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst)
}

/// 从任务队列中获取一个待下载的文件项
///
/// 遍历所有未完成任务，返回第一个有可用下载项的任务。
/// 暂停中的任务不分配新文件，在途文件由下载线程阻塞等待恢复。
///
/// # 返回值
///
/// 无可用下载项或全局暂停时返回 `None`
pub(crate) fn get_item() -> Option<DownloadObj> {
    if PAUSED.load(Ordering::SeqCst) {
        return None;
    }
    let read = TASKS.read().unwrap();
    for task in read.iter() {
        if task.is_paused() {
            continue;
        }
        if let Some(item) = task.get_item() {
            return Some(DownloadObj {
                task: task.clone(),
                item,
            });
        }
    }
    return None;
}

/// 标记任务完成并从队列中移除
///
/// - `task`: 已完成的任务
pub(crate) fn task_done(task: &DownloadTask) {
    let mut tasks = TASKS.write().unwrap();
    let id = task.id;

    tasks.retain(|t| t.id != task.id);

    remove_task(id);
}

/// 启动下载器
///
/// 按配置中的下载线程数创建工作线程池，线程数非法时按 5 个处理。
/// 应在下载前调用一次。
pub fn start() {
    let config = mml_config::read_config();
    let mut thread = config.http.download_thread;
    if thread <= 0 {
        thread = 5;
    }

    let mut list = THREADS.write().unwrap();
    for index in 0..thread {
        list.push(DownloadThread::new(index));
    }
}

/// 停止下载器
///
/// 设置停止标志，取消所有正在进行的任务，等待所有线程退出。
pub fn stop() {
    if STOP.load(Ordering::SeqCst) {
        return;
    }
    STOP.store(true, Ordering::SeqCst);
    for item in TASKS.write().unwrap().iter() {
        item.cancel();
    }
    for item in THREADS.write().unwrap().iter_mut() {
        item.stop();
    }
}

/// 下载任务快照，供 GUI 查询
#[derive(Debug, Clone)]
pub struct TaskSnapshot {
    /// 任务编号
    pub id: u64,
    /// 文件总数
    pub total: usize,
    /// 已完成数
    pub completed: usize,
    /// 失败数
    pub failed: usize,
    /// 总大小，单位字节；元信息未知的文件按 0 计
    pub all_bytes: u64,
    /// 已下载大小，单位字节
    pub now_bytes: u64,
    /// 已进行时间，单位毫秒
    pub elapsed_ms: u64,
    /// 是否暂停
    pub paused: bool,
}

/// 获取当前进行中任务的快照列表，供下载窗口查询
///
/// # 返回值
///
/// 返回队列中每个任务的进度快照，无进行中任务时为空列表
pub fn get_tasks() -> Vec<TaskSnapshot> {
    let read = TASKS.read().unwrap();
    read.iter().map(|t| t.snapshot()).collect()
}

/// 是否有下载任务的文件正落在某个目录下
///
/// 删实例前用它判断"这个实例还在不在下东西"，别把正在写的文件删掉。
///
/// - `dir`: 目录，比如某个实例的根目录
///
/// # 返回值
///
/// 有任务在跑、且它的文件目标路径在 `dir` 里时返回 `true`
pub fn is_busy_under(dir: impl AsRef<std::path::Path>) -> bool {
    let dir = dir.as_ref();
    let tasks = TASKS.read().unwrap();
    tasks.iter().any(|task| task.has_file_under(dir))
}

/// 暂停指定任务：不再分配新文件，在途文件阻塞等待恢复
///
/// - `id`: 任务编号
///
/// # 返回值
///
/// 成功返回 `true`，任务不存在、已完成或已取消时返回 `false`
pub fn pause_task(id: u64) -> bool {
    let read = TASKS.read().unwrap();
    match read.iter().find(|t| t.id == id) {
        Some(task) => {
            task.pause();
            true
        }
        None => false,
    }
}

/// 恢复指定任务并唤醒下载线程
///
/// - `id`: 任务编号
///
/// # 返回值
///
/// 成功返回 `true`，任务不存在、已完成或已取消时返回 `false`
pub fn resume_task(id: u64) -> bool {
    {
        let read = TASKS.read().unwrap();
        match read.iter().find(|t| t.id == id) {
            Some(task) => task.resume(),
            None => return false,
        }
    }

    // 暂停期间线程可能已空闲，恢复后需要重新唤醒
    for item in THREADS.read().unwrap().iter() {
        item.run();
    }
    true
}

/// 当前是否处于全局暂停
///
/// # 返回值
///
/// 全局暂停时为 `true`
pub fn is_paused_all() -> bool {
    PAUSED.load(Ordering::SeqCst)
}

/// 全局暂停：暂停所有任务，期间新增的任务也保持暂停
///
/// # 返回值
///
/// 返回被暂停的任务数
pub fn pause_all() -> usize {
    PAUSED.store(true, Ordering::SeqCst);
    let read = TASKS.read().unwrap();
    for task in read.iter() {
        task.pause();
    }
    read.len()
}

/// 全局恢复：恢复所有任务并唤醒下载线程
///
/// # 返回值
///
/// 返回被恢复的任务数
pub fn resume_all() -> usize {
    PAUSED.store(false, Ordering::SeqCst);
    let count = {
        let read = TASKS.read().unwrap();
        for task in read.iter() {
            task.resume();
        }
        read.len()
    };

    for item in THREADS.read().unwrap().iter() {
        item.run();
    }
    count
}

/// 全局停止：取消所有任务并清空任务队列
///
/// # 返回值
///
/// 返回被停止的任务数
pub fn cancel_all() -> usize {
    let tasks: Vec<Arc<DownloadTask>> = {
        let mut list = TASKS.write().unwrap();
        std::mem::take(&mut *list)
    };

    let count = tasks.len();
    for task in &tasks {
        task.cancel();
        task_done(task);
    }
    count
}

/// 取消指定任务：移出队列并唤醒完成等待，在途文件自然结束，剩余文件不再下载
///
/// - `id`: 任务编号
///
/// # 返回值
///
/// 取消成功返回 `true`，任务不存在、已完成或已取消时返回 `false`
pub fn cancel_task(id: u64) -> bool {
    let task = {
        let mut tasks = TASKS.write().unwrap();
        let pos = match tasks.iter().position(|t| t.id == id) {
            Some(pos) => pos,
            None => return false,
        };
        tasks.remove(pos)
    };
    task.cancel();
    task_done(&task);
    true
}

/// 创建新下载任务并开始下载
///
/// - `items`: 需要下载的文件列表
///
/// # 返回值
///
/// `true` — 全部下载成功
/// `false` — 下载被停止、被取消或有文件下载失败
pub async fn start_download_task(items: Vec<FileItemObj>) -> bool {
    start_download_task_outcome(items).await.is_done()
}

/// 创建新下载任务并开始下载，区分成功 / 失败 / 取消
///
/// - `items`: 需要下载的文件列表
///
/// # 返回值
///
/// 见 [`TaskOutcome`]
pub async fn start_download_task_outcome(items: Vec<FileItemObj>) -> TaskOutcome {
    start_download_task_cancellable(items, CancellationToken::new()).await
}

/// 创建新下载任务并开始下载，外部可通过 `cancel` 取消
///
/// - `items`: 需要下载的文件列表
/// - `cancel`: 外部取消令牌
///
/// # 返回值
///
/// 见 [`TaskOutcome`]；取消后任务会被移出队列
pub async fn start_download_task_cancellable(
    items: Vec<FileItemObj>,
    cancel: CancellationToken,
) -> TaskOutcome {
    match start_download_task_handle(items, cancel) {
        Some(handle) => handle.wait().await,
        None => TaskOutcome::Cancelled,
    }
}

/// 创建新下载任务并返回句柄
///
/// 句柄拿到时任务已经在队列里，可以立刻查进度；之后查状态、等结果、取消都走它，
/// 不用再去 [`get_tasks`] 里按编号找。
///
/// - `items`: 需要下载的文件列表
/// - `cancel`: 外部取消令牌，与 [`DownloadHandle::cancel`] 是同一批任务的两个叫停入口；
///   它只在 [`DownloadHandle::wait`] 期间生效，只查状态不等待时请用 `cancel` 叫停
///
/// # 返回值
///
/// 见 [`DownloadHandle`]；下载器已停止（进程正在退出）时返回 `None`
pub fn start_download_task_handle(
    items: Vec<FileItemObj>,
    cancel: CancellationToken,
) -> Option<DownloadHandle> {
    if STOP.load(Ordering::SeqCst) {
        return None;
    }
    let task = DownloadTask::new(items);
    // 全局暂停期间新增的任务同样保持暂停
    if PAUSED.load(Ordering::SeqCst) {
        task.pause();
    }
    let task = Arc::new(task);
    let id = task.id;

    TASKS.write().unwrap().push(task.clone());

    add_task(id);

    // 唤醒所有工作线程
    for item in THREADS.read().unwrap().iter() {
        item.run();
    }

    Some(DownloadHandle { id, task, cancel })
}

/// 下载任务句柄
///
/// 一批文件一个句柄：进度看 [`status`](Self::status)，结果用 [`wait`](Self::wait)，
/// 中途叫停用 [`cancel`](Self::cancel)。
pub struct DownloadHandle {
    /// 任务编号，与 [`get_tasks`] 里的编号一致
    id: u64,
    /// 任务本体：进度与完成信号都在它身上
    task: Arc<DownloadTask>,
    /// 外部取消令牌：调用方那一侧的叫停入口
    cancel: CancellationToken,
}

impl DownloadHandle {
    /// 任务编号
    ///
    /// # 返回值
    ///
    /// 与 [`get_tasks`] / [`cancel_task`] 同一套编号
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 任务当前状态
    ///
    /// # 返回值
    ///
    /// 见 [`TaskSnapshot`]；任务结束后仍可读到最后一组计数
    pub fn status(&self) -> TaskSnapshot {
        self.task.snapshot()
    }

    /// 任务是否已经结束
    ///
    /// # 返回值
    ///
    /// 任务已从任务表里摘掉（下完 / 失败 / 取消）时返回 `true`
    pub fn is_finished(&self) -> bool {
        let tasks = TASKS.read().unwrap();
        !tasks.iter().any(|task| task.id == self.id)
    }

    /// 等任务结束并取结果
    ///
    /// 外部取消令牌置位时与 [`cancel_task`] 同一套收尾：移出队列、唤醒等待方。
    ///
    /// # 返回值
    ///
    /// 见 [`TaskOutcome`]
    pub async fn wait(&self) -> TaskOutcome {
        tokio::select! {
            res = self.task.wait_done() => res,
            _ = self.cancel.cancelled() => {
                // 与 cancel_task 相同：移出队列并唤醒完成等待
                TASKS.write().unwrap().retain(|task| task.id != self.id);
                self.task.cancel();
                task_done(&self.task);
                TaskOutcome::Cancelled
            }
        }
    }

    /// 取消任务
    ///
    /// 与 [`cancel_task`] 同一套收尾：移出队列并唤醒 [`wait`](Self::wait)。
    pub fn cancel(&self) {
        let _ = cancel_task(self.id);
    }
}

// ============================================================================
// 测试支持与单元测试
// ============================================================================

#[cfg(test)]
pub(crate) mod test_util {
    //! 测试公共辅助：日志、配置、基础目录、GUI 回调等全局状态在同一个测试
    //! 二进制中只能初始化一次，这里用 `OnceLock` + `Once` 保证幂等。

    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, Once, OnceLock};

    /// GUI 回调记录到的事件列表
    pub static GUI_EVENTS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

    /// 测试运行根目录，临时目录下的唯一子目录
    static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
    /// 保证初始化流程只执行一次
    static INIT: Once = Once::new();

    /// 测试专用的 GUI 回调，把所有通知记录为字符串
    struct TestGui;

    impl super::IDownloadGui for TestGui {
        fn update(&self, thread: u32, file: &std::sync::Arc<super::download_item::DownloadItem>) {
            let mut events = GUI_EVENTS.get().unwrap().lock().unwrap();
            events.push(format!(
                "file:{}:{}:{:.1}",
                thread,
                file.base.name,
                file.progress()
            ));
        }

        fn update_task(&self, state: super::DownloadTaskState) {
            let mut events = GUI_EVENTS.get().unwrap().lock().unwrap();
            match state {
                super::DownloadTaskState::AddTask(id) => events.push(format!("AddTask:{id}")),
                super::DownloadTaskState::RemoveTask(id) => events.push(format!("RemoveTask:{id}")),
                super::DownloadTaskState::UpdateTask(obj) => {
                    events.push(format!("UpdateTask:{}:{:.0}", obj.id, obj.progress))
                }
            }
        }
    }

    /// 在临时目录下创建唯一的子目录，供测试隔离使用
    ///
    /// - `name`: 目录名中的用例特征，用于区分同一测试的不同用例
    ///
    /// # 返回值
    ///
    /// 返回已创建的空目录路径，名字带用例特征、进程 id 与自增序号
    pub fn make_temp_dir(name: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = mml_testutil::temp_dir().join(format!(
            "mml-downloader-unit-{}-{}-{}",
            name,
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 初始化测试环境，进程内只执行一次
    ///
    /// # 返回值
    ///
    /// 返回测试运行根目录，重复调用返回同一目录
    pub fn ensure_env() -> PathBuf {
        INIT.call_once(|| {
            let dir = make_temp_dir("run");

            GUI_EVENTS
                .set(Mutex::new(Vec::new()))
                .ok()
                .unwrap_or_else(|| panic!("GUI_EVENTS 初始化失败"));

            // 日志系统，下载线程内部记录错误时依赖信号量已初始化
            mml_log::start(&dir).unwrap();
            // 基础目录
            mml_base::init(&dir);
            // 配置系统
            mml_config::init(&dir).unwrap();
            // GUI 回调，全局单例
            super::set_gui_handel(Box::new(TestGui));

            RUN_DIR.set(dir).unwrap();
        });

        RUN_DIR.get().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use mml_base::file_item::FileItemObj;

    use crate::download_task::{DownloadTask, TaskOutcome};

    use super::test_util::{GUI_EVENTS, ensure_env};

    /// 构造一个简单的下载文件项
    fn file_item(name: &str) -> FileItemObj {
        FileItemObj {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// 任务队列：FIFO 出队 + 计数
    #[test]
    fn task_queue_fifo_and_counts() {
        ensure_env();

        let task = DownloadTask::new(vec![file_item("a"), file_item("b")]);
        assert_eq!(task.total_size, 2);
        assert_eq!(task.completed_count.load(Ordering::SeqCst), 0);
        assert_eq!(task.failed_count.load(Ordering::SeqCst), 0);

        // SegQueue 是先进先出
        let first = task.get_item().unwrap();
        assert_eq!(first.base.name, "a");
        let second = task.get_item().unwrap();
        assert_eq!(second.base.name, "b");
        // 队列取空后返回 None
        assert!(task.get_item().is_none());
    }

    /// 成功完成的任务：wait_done 返回 Done，并从全局任务队列移除
    #[test]
    fn task_done_waits_and_completes() {
        ensure_env();

        let task = DownloadTask::new(vec![file_item("a")]);
        let id = task.id;

        // 队列非空时任务未完成，wait_done 不会被提前唤醒；
        // 这里先取走唯一文件再标记完成，保证不会阻塞
        let _item = task.get_item().unwrap();
        task.done();
        assert_eq!(task.completed_count.load(Ordering::SeqCst), 1);

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let res = rt.block_on(task.wait_done());
        assert_eq!(res, TaskOutcome::Done);

        // 任务原本就不在全局队列中，此处确认移除无副作用
        assert!(super::get_tasks().iter().all(|t| t.id != id));
    }

    /// 存在失败文件的任务：wait_done 返回 Failed
    #[test]
    fn task_failed_reports_false() {
        ensure_env();

        let task = DownloadTask::new(vec![file_item("a")]);
        let _item = task.get_item().unwrap();
        task.fail();
        assert_eq!(task.failed_count.load(Ordering::SeqCst), 1);

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let res = rt.block_on(task.wait_done());
        assert_eq!(res, TaskOutcome::Failed);
    }

    /// 任务被取消：结束方式是 Cancelled，且不计入失败数
    #[test]
    fn task_cancelled_is_not_a_failure() {
        ensure_env();

        let task = DownloadTask::new(vec![file_item("a")]);
        let _item = task.get_item().unwrap();
        task.mark_cancelled();
        assert_eq!(task.failed_count.load(Ordering::SeqCst), 0);

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(rt.block_on(task.wait_done()), TaskOutcome::Cancelled);
    }

    /// 取消令牌已触发：wait_done 立刻返回 Cancelled，不等在途文件自行收尾
    #[test]
    fn task_cancel_token_reports_cancelled() {
        ensure_env();

        let task = DownloadTask::new(vec![file_item("a")]);
        let _item = task.get_item().unwrap();
        task.cancel();

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(rt.block_on(task.wait_done()), TaskOutcome::Cancelled);
    }

    /// 任务进度通过 GUI 回调上报，1/2 完成即 50%
    #[test]
    fn task_progress_notifies_gui() {
        ensure_env();

        let task = DownloadTask::new(vec![file_item("a"), file_item("b")]);
        let id = task.id;

        let _item = task.get_item().unwrap();
        task.done();

        let events = GUI_EVENTS.get().unwrap().lock().unwrap();
        assert!(
            events.iter().any(|e| e == &format!("UpdateTask:{id}:50")),
            "应收到 50% 进度事件，实际事件: {:?}",
            *events
        );
    }
}
