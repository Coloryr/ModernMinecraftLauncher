use std::env;

/// 初始化日志 / 配置 / 网络
fn init() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let run_dir = exe_dir.parent().unwrap().to_path_buf();

    mml_log::start(&run_dir);
    mml_config::init(&run_dir);
    mml_net::init();
}

/// 停止日志
fn stop() {
    mml_log::stop();
}

// #[ignore = "reason"]
// #[tokio::test]
// async fn get_fabric_meta() {
//     init();

//     let data = fabric_api::get_meta().await.unwrap();

//     // println!("{}", String::from_utf8_lossy(&data));

//     let obj: Value = serde_json::from_slice(&data).unwrap();

//     assert!(obj.is_object());

//     let obj1 = obj.as_object();
//     let item = &obj1.iter().next().unwrap();

//     assert!(!item.is_empty());
//     assert!(item.contains_key("game"));

//     stop();
// }

// #[ignore = "reason"]
// #[tokio::test]
// async fn get_fabric_loader() {
//     init();

//     let data = fabric_api::get_loader("26.2", "0.19.3").await.unwrap();

//     // println!("{}", String::from_utf8_lossy(&data));

//     let obj: Value = serde_json::from_slice(&data).unwrap();

//     assert!(obj.is_object());

//     let obj1 = obj.as_object().unwrap();

//     assert!(!obj1.is_empty());
//     assert!(obj1.contains_key("id"));

//     stop();
// }
