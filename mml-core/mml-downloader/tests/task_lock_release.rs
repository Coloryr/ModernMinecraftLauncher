//! 回归：任务收尾时不能持着任务表锁回调 GUI
//!
//! 真实链路里 GUI 回调会进 Tauri 事件循环（判断前台窗口要问主线程），是**阻塞**的；
//! 主线程上同时可能正跑着 `download_get_tasks`，它要读任务表。一旦 `task_done`
//! 持写锁去回调，两边就互等 —— 表现是"最后一个文件下完，整个程序死掉"。
//!
//! 这里用一个"进入回调后先卡住、由测试放行"的假回调复现：回调卡住的那段时间里，
//! 另一条线程必须还能用 `get_tasks` 读到任务表。

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once, OnceLock, mpsc};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

/// 测试运行根目录
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 初始化闸门：整个测试二进制只初始化一次
static INIT: Once = Once::new();

/// 初始化运行环境，进程内只执行一次
///
/// # 返回值
///
/// 返回测试运行根目录
fn setup() -> PathBuf {
    INIT.call_once(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml-dl-lock-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_log::start(&dir).expect("日志系统启动失败");
        mml_config::init(&dir).expect("配置系统初始化失败");
        mml_downloader::init(&dir).expect("下载器初始化失败");

        RUN_DIR.set(dir).unwrap();
    });

    RUN_DIR.get().unwrap().clone()
}

/// 假 GUI 回调：RemoveTask 命中目标任务时先通知测试，再卡住等放行
///
/// 卡住的效果等价于真实回调在等主线程处理窗口查询。
struct BlockingGui {
    /// 只对这条任务卡住，别的任务照常忽略
    waited_id: u64,
    /// 回调已进入，通知测试
    entered: Mutex<Option<mpsc::Sender<()>>>,
    /// 等测试放行
    gate: Mutex<Option<mpsc::Receiver<()>>>,
}

impl mml_downloader::IDownloadGui for BlockingGui {
    /// 本用例不关心单文件进度
    fn update(&self, _thread: u32, _file: &Arc<mml_downloader::download_item::DownloadItem>) {}

    /// 任务状态：只拦目标任务被移除的那一次
    ///
    /// - `state`: 任务状态变化
    fn update_task(&self, state: mml_downloader::DownloadTaskState) {
        let mml_downloader::DownloadTaskState::RemoveTask(id) = state else {
            return;
        };
        if id != self.waited_id {
            return;
        }

        if let Some(tx) = self.entered.lock().unwrap().take() {
            let _ = tx.send(());
        }
        // 模拟"等主线程"：最多等 5 秒，超时也继续，避免测试自身挂死
        if let Some(rx) = self.gate.lock().unwrap().take() {
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }
    }
}

/// 收尾回调卡住期间，任务表必须仍然可读
#[test]
fn task_done_releases_task_lock_before_gui_callback() {
    setup();

    let (entered_tx, entered_rx) = mpsc::channel();
    let (gate_tx, gate_rx) = mpsc::channel();

    // 先建任务拿到编号，再装回调：回调要按编号判断，装早了会把 AddTask 也拦进来
    let handle = mml_downloader::start_download_task_handle(Vec::new(), CancellationToken::new())
        .expect("创建下载任务失败");
    let id = handle.id();

    mml_downloader::set_gui_handel(Box::new(BlockingGui {
        waited_id: id,
        entered: Mutex::new(Some(entered_tx)),
        gate: Mutex::new(Some(gate_rx)),
    }));

    // 取消任务：收尾会走 task_done，回调在后台线程里卡住
    let canceller = std::thread::spawn(move || mml_downloader::cancel_task(id));

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("取消任务没有走到 GUI 回调");

    // 回调卡住的这段时间，主线程上的 get_tasks 必须能拿到读锁
    let (read_tx, read_rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let _ = mml_downloader::get_tasks();
        let _ = read_tx.send(());
    });

    let read_ok = read_rx.recv_timeout(Duration::from_secs(2)).is_ok();

    // 无论成败都放行：不把线程和锁留在阻塞里，失败时也只报这一条
    let _ = gate_tx.send(());
    assert!(canceller.join().is_ok(), "取消线程异常退出");
    assert!(reader.join().is_ok(), "读取线程异常退出");

    assert!(read_ok, "task_done 持着任务表写锁回调 GUI：回调卡住期间 get_tasks 读不到任务表");
}
