//! Java 下载链路集成测试
//!
//! 走完整链路：foojay /packages 拉取候选列表 → 从列表缓存取出下载项 →
//! 下载压缩包 → 解包识别并注册进 Java 列表。
//!
//! 真实联网 + 真实解包；运行目录在仓库 target/temp，git 忽略，可反复复用。
//! 注意：本测试会下载几十 MB 的压缩包，比一般单测慢。

use std::path::PathBuf;
use std::sync::Once;

use mml_gui_lib::dtos::JavaTypes;
use mml_gui_lib::windows::java_download::{
    JavaInstallResult, install_cached, java_download_get_list,
};
use tokio_util::sync::CancellationToken;

static INIT: Once = Once::new();

/// 运行根目录，核心初始化 / 下载 / Java 存放都在这下面
///
/// # 返回值
///
/// 仓库 target/temp 下的固定目录，git 忽略，可跨进程复用
fn run_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/java-download-test")
}

/// 初始化全局依赖链，进程内只执行一次，与应用 mml_core::init 同构
fn setup() {
    INIT.call_once(|| {
        std::fs::create_dir_all(run_dir()).unwrap();

        mml_core::init(mml_core::CoreInitObj {
            path: run_dir(),
            oauth_key: String::new(),
            curseforge_key: String::new(),
        })
        .expect("核心初始化失败");
        mml_core::load().expect("核心加载失败");
        // 应用在 tauri setup 里做的下载线程池启动
        mml_downloader::start();
    });
}

/// foojay：拉列表 → 下载压缩包 → 解包注册进 Java 列表
#[tokio::test]
async fn foojay_download_and_register() {
    setup();

    // 按当前系统取包：写死 windows 时在 Linux / macOS 上会去下一个几十 MB 的 Windows JRE，
    // 解包也认不出来，等于白跑
    let system = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };

    // jre 包比 jdk 小很多，下载快
    let list = java_download_get_list(
        JavaTypes::Foojay,
        "jre".to_string(),
        21,
        system.to_string(),
        arch.to_string(),
    )
    .await
    .expect("foojay 拉取列表失败");
    assert!(
        !list.is_empty(),
        "foojay 21/jre/{system}/{arch} 应有候选包"
    );

    // 列表只留能解包的格式：deb / rpm / apk 这类包管理器格式不能出现
    for item in &list {
        let lower = item.filename.to_ascii_lowercase();
        assert!(
            [".zip", ".tar", ".tar.gz", ".tgz", ".tar.xz", ".txz", ".7z"]
                .iter()
                .any(|ext| lower.ends_with(ext)),
            "不可解包的格式混进列表: {}",
            item.filename
        );
    }

    // 优先 Temurin：包更小、下载地址最稳定；没有 zip 再退到 tar.gz
    let Some(picked) = list
        .iter()
        .find(|item| item.name.starts_with("Temurin") && item.filename.ends_with(".zip"))
        .or_else(|| list.iter().find(|item| item.filename.ends_with(".zip")))
        .or_else(|| list.iter().find(|item| item.filename.ends_with(".tar.gz")))
    else {
        panic!("候选包里应存在可解包的压缩包");
    };
    println!(
        "候选包 {} 个，选择下载: {} ({})",
        list.len(),
        picked.name,
        picked.filename
    );

    // 下载 → 解包 → 注册；与命令走同一条安装流程
    let result = install_cached(&picked.uuid, CancellationToken::new(), |_| {}).await;
    assert!(
        matches!(result, JavaInstallResult::Done),
        "下载 + 解包注册失败: {result:?}"
    );

    // 全新运行目录下，注册进来的 Java 必然来自本次下载
    let all = mml_jvms::get_all_java();
    assert!(!all.is_empty(), "下载后 Java 列表不应为空");
    for item in &all {
        println!(
            "已注册: {} · {} · {} · {} · {}",
            item.name,
            item.version,
            item.java_type,
            item.arch,
            item.path.display()
        );
    }
}
