use std::{env, fs, sync::Arc};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_downloader::{DownloadTaskState, IDownloadGui, download_item::DownloadItem};

/// 测试用 GUI 回调，把下载状态打印到标准输出
struct GuiRun {}

impl IDownloadGui for GuiRun {
    fn update(&self, _thread: u32, _file: &Arc<DownloadItem>) {
        // let pro = file.progress() as u64;
        // if pro > 0 && pro % 10 == 0 {
        //     println!(
        //         "线程 {thread} 下载项目 {} {}/{} {}%",
        //         file.base.name,
        //         file.get_now_size(),
        //         file.get_all_size(),
        //         file.progress()
        //     );
        // }
    }

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

/// 真实网络下载测试：需要访问外网，Apache 镜像，仅在显式运行时执行：
/// `cargo test -- --ignored`，默认 `cargo test` 下跳过。
#[ignore = "需要访问外网"]
#[tokio::test]
async fn test_download() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    let _ = mml_log::start(&run_dir);
    let _ = mml_config::init(&run_dir);
    mml_net::init();

    let gui = GuiRun {};

    mml_downloader::set_gui_handel(Box::new(gui));

    mml_downloader::init(&run_dir).unwrap();
    mml_downloader::start();

    let obj = FileItemObj {
        name: String::from("apache-tomcat-11.0.22.zip"),
        file: run_dir.join("apache-tomcat-11.0.22.zip"),
        url: String::from(
            "https://dlcdn.apache.org/tomcat/tomcat-11/v11.0.22/bin/apache-tomcat-11.0.22.zip",
        ),
        hash: FileHash::Sha512(String::from(
            "b08163a8d51455d3a7ba8e588b824d06718450439be9d461913afa4a978f249d82b07d6837ad07ada0991408c2d0f1ccfc7c85617fe874e387e1ad89b4f7c12d",
        )),
        later: LaterRun::None,
    };

    let res = mml_downloader::start_download_task(vec![obj]).await;
    assert!(res);

    mml_downloader::stop();

    fs::remove_file(run_dir.join("apache-tomcat-11.0.22.zip")).unwrap();
}
