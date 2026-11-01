use super::*;

// 注意：以下测试依赖 KEY 全局未被 `set_key()` 初始化，
// 本测试二进制内不要新增会调用 `set_key()` 的非 ignore 测试。

/// 未设置 KEY 时，设备码流程第一步应在发起网络请求前失败
#[tokio::test]
async fn test_get_code_without_key() {
    let result = get_code().await;
    assert!(matches!(result, Err(ErrorType::KeyIsNull)));
}

/// 未设置 KEY 时，refresh_token 刷新也应在发起网络请求前失败
#[tokio::test]
async fn test_refresh_token_without_key() {
    let result = refresh_oauth_token("fake-refresh-token").await;
    assert!(matches!(result, Err(ErrorType::KeyIsNull)));
}

