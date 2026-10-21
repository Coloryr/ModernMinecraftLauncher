use std::{env, path::Path, sync::Arc};

use mml_downloader::{DownloadTaskState, IDownloadGui, download_item::DownloadItem};

/// 下载回调的测试实现，只把状态打到标准输出
struct GuiRun {}

impl IDownloadGui for GuiRun {
    /// 单文件进度回调，测试里不输出
    fn update(&self, _thread: u32, _file: &Arc<DownloadItem>) {
        // 需要看进度时打开：按线程号打印每个下载项的百分比
    }

    /// 任务级状态回调，增删改都打到标准输出
    fn update_task(&self, state: DownloadTaskState) {
        match state {
            DownloadTaskState::AddTask(task) => {
                println!("下载状态 新建下载任务 {}", task)
            }
            DownloadTaskState::RemoveTask(task) => {
                println!("下载状态 删除下载任务 {}", task)
            }
            DownloadTaskState::UpdateTask(task_state_obj) => {
                println!(
                    "下载状态 更新下载任务 {} 进度 {}",
                    task_state_obj.id, task_state_obj.progress
                )
            }
        }
    }
}

/// 依次启动日志 / 配置 / 网络 / 下载器
///
/// - `run_dir`: 运行目录，日志与配置都落在它下面
fn start(run_dir: &Path) {
    let _ = mml_log::start(&run_dir);
    let _ = mml_config::init(&run_dir);
    mml_net::init();

    let gui = GuiRun {};

    mml_downloader::set_gui_handel(Box::new(gui));

    mml_downloader::init(&run_dir).unwrap();
    mml_downloader::start();
}

/// 冒烟测试：把各内核系统初始化一遍，不做断言
#[tokio::test]
async fn start_game() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    start(&run_dir);
}
