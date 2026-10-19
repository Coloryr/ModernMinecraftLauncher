//! 整合包安装流程手动测试：用本机真实压缩包跑完整安装链，打印进度与错误。
//!
//! 真实联网 + 依赖本机压缩包，默认跳过；手动运行：
//! `cargo test -p mml-game --test modpack -- --ignored --nocapture`
//! 压缩包路径改源码里的 `PACK_FILE`，CurseForge key 由 GUI 正常注入，测试内为空

use std::{path::{Path, PathBuf}, sync::Arc};

use mml_game::add_game::{self, PackType};
use mml_game::gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState, IAddInstanceGui, IAddModPackGui};
use mml_game::GameInstance;
use tokio_util::sync::CancellationToken;

/// 测试压缩包路径
const PACK_FILE: &str = r"H:\ftb-stoneblock-4-1.14.2.zip";

/// 实例创建回调桩：同意自动改名，拒绝覆盖已有实例
struct TestGui;

#[async_trait::async_trait]
impl IAddInstanceGui for TestGui {
    async fn name_replace(&self, _name: &str) -> bool {
        true
    }

    async fn overwrite(&self, _obj: GameInstance) -> bool {
        false
    }
}

/// 把安装状态映射成前端使用的 ID
fn state_id(state: &AddModPackState) -> &'static str {
    match state {
        AddModPackState::DownloadPack => "downloadPack",
        AddModPackState::ReadInfo => "readInfo",
        AddModPackState::GetInfo => "getInfo",
        AddModPackState::DownloadFile => "downloadFile",
        AddModPackState::Extract => "extract",
        AddModPackState::Done => "done",
    }
}

/// 整合包安装回调桩：把阶段与进度打到 stdout
struct PackGui;

impl IAddModPackGui for PackGui {
    fn set_state(&self, state: AddModPackState) {
        println!("[安装] 阶段: {}", state_id(&state));
    }

    fn set_now(&self, value: usize, all: Option<usize>) {
        println!("[安装] 进度 {}/{}", value, all.map(|v| v.to_string()).unwrap_or_else(|| "?".into()));
    }

    fn set_sub_text(&self, text: Option<String>) {
        println!("[安装] 子进度文字: {}", text.unwrap_or_default());
    }

    fn set_sub_now(&self, value: usize, all: Option<usize>) {
        println!("[安装] 子进度 {}/{}", value, all.map(|v| v.to_string()).unwrap_or_else(|| "?".into()));
    }
}

/// 按 mml_core::init 的链路初始化各内核系统
fn start(run_dir: &Path) {
    // 与 mml_core::init 相同的初始化链，不含 jvms / config_save
    mml_base::init(run_dir.to_path_buf());
    // 不设置 CurseForge key：仅测试 API 之前的阶段，即检测、读信息、建实例、解压
    mml_names::init(mml_base::get_base_dir()).unwrap();
    mml_log::start(mml_base::get_base_dir()).unwrap();
    mml_config::init(mml_base::get_base_dir()).unwrap();
    mml_config::config_save::start();
    mml_game::init(mml_base::get_base_dir()).unwrap();
    mml_net::init();

    mml_downloader::init(run_dir).unwrap();
    mml_downloader::set_gui_handel(Box::new(TestDownloader));
    mml_downloader::start();
}

/// 下载回调桩：每过 25% 打一行
struct TestDownloader;

impl mml_downloader::IDownloadGui for TestDownloader {
    fn update(&self, thread: u32, file: &Arc<mml_downloader::download_item::DownloadItem>) {
        let pro = file.progress() as u64;
        if pro > 0 && pro % 25 == 0 {
            println!(
                "[下载] 线程 {thread} {} {}%",
                file.base.name,
                pro
            );
        }
    }

    fn update_task(&self, state: mml_downloader::DownloadTaskState) {
        match state {
            mml_downloader::DownloadTaskState::AddTask(id) => println!("[下载] 新任务 {id}"),
            mml_downloader::DownloadTaskState::RemoveTask(id) => println!("[下载] 任务结束 {id}"),
            mml_downloader::DownloadTaskState::UpdateTask(obj) => {
                println!("[下载] 任务 {} 进度 {:.1}%", obj.id, obj.progress)
            }
        }
    }
}

/// 用本机真实压缩包跑一遍完整的 CurseForge 整合包安装
#[tokio::test]
#[ignore]
async fn install_curseforge_pack() {
    let run_dir = mml_testutil::temp_dir().join("mml-modpack-test");
    std::fs::create_dir_all(&run_dir).unwrap();
    start(&run_dir);

    let file = PathBuf::from(PACK_FILE);
    if !file.exists() {
        // 本机没有测试压缩包时跳过，保证全平台可跑
        eprintln!("跳过: 测试压缩包不存在: {}", file.display());
        return;
    }

    // 类型检测 + 名字识别
    let detected = add_game::detect_pack(&file).expect("类型检测失败");
    println!(
        "检测: type={} name={}",
        detected.pack_type.id(),
        detected.name
    );

    let token = CancellationToken::new();
    let gui: AddInstanceGui = Some(Arc::new(TestGui));
    let pack_gui: AddModPackGui = Some(Arc::new(PackGui));

    let res = add_game::install_archive_from_file(
        &file,
        Some(detected.name),
        None,
        None,
        gui,
        pack_gui,
        None,
        PackType::CurseForge,
        token,
    )
    .await;

    match res {
        Ok(uuid) => println!("安装成功: {uuid}"),
        Err(e) => panic!("安装失败: {} ({:?})", mml_names::i18::get_error(e.clone()), e),
    }
}
