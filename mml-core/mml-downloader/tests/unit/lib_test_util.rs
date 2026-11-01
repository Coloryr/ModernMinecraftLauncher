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

