use chrono::{DateTime, FixedOffset, Local};
use mml_auth::oauth;

/// 打印本地时间与固定偏移时间的对照，供人工查看
#[test]
fn test_time() {
    let dt = Local::now();
    let time: DateTime<FixedOffset> = dt.fixed_offset();

    println!("Local: {}", dt);
    println!("FixedOffset结果: {}", time);
}

/// 走一次真实的 OAuth 设备码流程；默认不运行，需要网络与已设置的客户端密钥
#[tokio::test]
#[ignore = "skip"]
async fn oauth_test() {
    let code = oauth::get_code().await.unwrap();
}
