//! 下载线程池按配置伸缩：设置里改「下载并发数」后调 start() 即刻生效
//!
//! 单独一个测试二进制：线程池是进程级单例，与其他用例共用会互相改配置。

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use mml_config::config_obj::ProxyState;

/// 初始化内核各系统，返回运行目录；重复调用返回同一目录
fn setup() -> PathBuf {
    static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
    if let Some(dir) = RUN_DIR.get() {
        return dir.clone();
    }

    let dir = mml_testutil::temp_dir().join(format!("mml-thread-pool-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    RUN_DIR.set(dir.clone()).unwrap();

    mml_base::init(&dir);
    mml_log::start(&dir).expect("日志系统启动失败");
    mml_config::init(&dir).expect("配置系统初始化失败");
    {
        let mut config = mml_config::write_config();
        config.http.work_proxy = ProxyState::None;
        config.http.login_proxy = ProxyState::None;
    }
    dir
}

/// 线程池跟着配置走，重复调用不会重复建线程
#[test]
fn pool_follows_config() {
    setup();

    mml_downloader::start();
    assert_eq!(mml_downloader::thread_count(), 10, "默认配置应为 10 个线程");

    mml_downloader::start();
    assert_eq!(
        mml_downloader::thread_count(),
        10,
        "重复调用 start 不应重复创建线程"
    );

    {
        let mut config = mml_config::write_config();
        config.http.download_thread = 3;
    }
    mml_downloader::start();
    assert_eq!(mml_downloader::thread_count(), 3, "缩容后应剩 3 个线程");

    {
        let mut config = mml_config::write_config();
        config.http.download_thread = 16;
    }
    mml_downloader::start();
    assert_eq!(mml_downloader::thread_count(), 16, "扩容后应有 16 个线程");

    mml_downloader::stop();
}