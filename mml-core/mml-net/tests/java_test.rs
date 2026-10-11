use std::env;

use mml_net::adoptium_api;

/// 初始化日志、配置与 HTTP 客户端
fn init() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    mml_log::start(&run_dir);
    mml_config::init(&run_dir);
    mml_net::init();
}

/// 停掉日志
fn stop() {
    mml_log::stop();
}

/// 拉取 Adoptium 的 Java 版本列表，断言其中含 8
async fn get_adoptium() {
    let list = adoptium_api::get_java_version().await;
    assert!(list.is_ok());

    let list = list.unwrap();
    assert!(list.contains(&String::from("8")));
}

/// 联网验证 Adoptium 的 Java 版本列表可拉取
#[tokio::test]
async fn java_test() {
    init();

    get_adoptium().await;

    stop();
}
