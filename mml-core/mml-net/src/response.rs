//! 响应处理
//!
//! 状态码判定、JSON 反序列化与错误上下文包装。

use mml_base::serialize_tools;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use serde::de::DeserializeOwned;

use super::error::{http_error, map_err};
use super::log_request_error;

/// 带 ETag 缓存校验的下载结果，[`Client::get_bytes_with_check`] 返回
pub struct AssetCheckResponse {
    /// 服务器返回 304，表示本地缓存的资源仍然有效
    pub not_modified: bool,
    /// ETag 响应头；S3 / R2 类存储在非分片上传时就是内容的 MD5
    pub etag: Option<String>,
    /// 响应体字节，304 时为空
    pub data: Vec<u8>,
}

/// 处理 HTTP 响应：检查状态码并解析 JSON
///
/// 状态码表示失败，即非 2xx 时返回 `HttpError`；成功时反序列化 JSON 为指定类型，
/// 解析失败时把请求地址一并写进错误信息。
///
/// **这里不加 `Send + 'static`**：`curseforge_api::send<T: DeserializeOwned>` 等
/// 别的模块会直接调用本函数，给泛型加约束会把它们一起卡死；需要经过 [`abortable`]
/// 即 `tokio::spawn` 的调用点，改为在各自方法上要求 `T: Send + 'static`。
///
/// - `resp`: 已收到的响应
///
/// # 返回值
///
/// 返回响应体反序列化后的 `T`
pub async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> CoreResult<T> {
    let status = resp.status();
    let url = resp.url().to_string();
    if !status.is_success() {
        let error = resp.text().await.unwrap_or_default();
        let err = http_error(error, url, Some(status.as_u16()));
        log_request_error(&err);
        return Err(err);
    }
    let bytes = resp.bytes().await.map_err(map_err)?;
    serialize_tools::json_from_bytes(&bytes).map_err(|err| {
        let err = serialize_err_context::<T>(err, &url);
        log_request_error(&err);
        err
    })
}

/// 给 JSON 解析错误补上请求地址与目标解析类型
///
/// serde 的报错只有字段级描述，如 `invalid type: null at line 1 column 42`，
/// 定位不到是哪个接口、哪个结构体出的问题，网络侧解析失败时统一补在后面。
///
/// - `err`: 原始错误，只有 `SerializerError` 会被改写
/// - `url`: 出错请求的地址
///
/// # 返回值
///
/// 返回补充了 `url` 与目标类型名的错误
pub fn serialize_err_context<T>(err: ErrorType, url: &str) -> ErrorType {
    match err {
        ErrorType::SerializerError(mut data) => {
            data.error = format!(
                "{} (url: {url}, target: {})",
                data.error,
                std::any::type_name::<T>()
            );
            ErrorType::SerializerError(data)
        }
        other => other,
    }
}
