use std::env;

use mml_net::openj9_api;

/// 按可执行文件所在目录初始化基础、日志、配置与网络，供本文件的用例共用
fn init() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    mml_base::init(&run_dir);
    mml_log::start(&run_dir).unwrap();
    mml_config::init(&run_dir).unwrap();
    mml_net::init();
}

/// 真实拉取一次 Java 列表，验证 openj9 接口在联网环境下能正常返回
#[tokio::test]
async fn test() {
    init();

    let _data = openj9_api::get_java_list().await.unwrap();
}
