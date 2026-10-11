use std::env;

use mml_net::foojay_api;

/// 初始化基础目录、日志、配置与 HTTP 客户端
fn init() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    mml_base::init(&run_dir);
    mml_log::start(&run_dir).unwrap();
    mml_config::init(&run_dir).unwrap();
    mml_net::init();
}

/// 联网拉取 Foojay 的筛选选项与某一档的 Java 列表
#[tokio::test]
async fn test() {
    init();

    let data = foojay_api::get_options().await.unwrap();
    assert!(!data.types.is_empty());
    assert!(!data.majors.is_empty());
    assert!(!data.systems.is_empty());
    assert!(!data.archs.is_empty());
    println!(
        "types: {:?}\nmajors: {:?}\nsystems: {:?}\narchs: {:?}",
        data.types, data.majors, data.systems, data.archs
    );

    let data = foojay_api::get_java_list(21, "windows", "x64", "jdk")
        .await
        .unwrap();
    assert!(!data.is_empty());
    println!("count: {}", data.len());
    println!("{:#?}", data.first().unwrap());
}
