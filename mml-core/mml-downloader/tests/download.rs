use std::{env, fs, sync::Arc};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_downloader::{DownloadTaskState, IDownloadGui, download_item::DownloadItem};

/// 版本清单地址，只用来探外网可达性
const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// 1.21.1 的版本 JSON：地址里带 sha1，Mojang 换文件就会换地址，内容不会变
const VERSION_JSON_URL: &str = "https://piston-meta.mojang.com/v1/packages/82ba6501da174a665c1787775f946487422f2432/1.21.1.json";
/// [`VERSION_JSON_URL`] 的 SHA-1，与地址里的那一段一致
const VERSION_JSON_SHA1: &str = "82ba6501da174a665c1787775f946487422f2432";

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

/// 真实网络下载测试：从 Mojang 取一个地址固定的版本 JSON，默认随 `cargo test` 执行，
/// 连不上外网时跳过。
#[tokio::test]
async fn test_download() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    let _ = mml_log::start(&run_dir);
    let _ = mml_config::init(&run_dir);
    mml_net::init();

    // 外网不通就跳过：离线环境不该报成下载失败
    match mml_net::get_work_client()
        .get_bytes(VERSION_MANIFEST_URL)
        .await
    {
        Ok(data) if !data.is_empty() => {}
        _ => {
            eprintln!("跳过: 网络不可用（无法获取 Mojang 版本清单）");
            return;
        }
    }

    let gui = GuiRun {};

    mml_downloader::set_gui_handel(Box::new(gui));

    mml_downloader::init(&run_dir).unwrap();
    mml_downloader::start();

    let out_file = run_dir.join("1.21.1.json");

    let obj = FileItemObj {
        name: String::from("1.21.1.json"),
        file: out_file.clone(),
        url: String::from(VERSION_JSON_URL),
        hash: FileHash::Sha1(String::from(VERSION_JSON_SHA1)),
        later: LaterRun::None,
    };

    let res = mml_downloader::start_download_task(vec![obj]).await;
    assert!(res);

    mml_downloader::stop();

    fs::remove_file(out_file).unwrap();
}
