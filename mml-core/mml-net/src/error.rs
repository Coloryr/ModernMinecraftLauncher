//! 错误转换
//!
//! 把 `reqwest` 的错误文本与状态码转成内核错误。

use mml_names::i18_items::error_type::{ErrorData, ErrorType, HttpErrorData};

use super::log_request_error;

/// 把 reqwest 错误连同底层原因串成一行
///
/// `Display` 常常只有一层（构建阶段失败就只有 `builder error`），真正的错因挂在 `source`
/// 链上：只记顶层等于没记，排查时分不出是地址、协议、代理还是 TLS。
///
/// - `error`: reqwest 抛出的错误
///
/// # 返回值
///
/// `顶层：原因：原因…` 形式的一行文本
pub(super) fn err_text(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(err) = source {
        text.push_str("：");
        text.push_str(&err.to_string());
        source = err.source();
    }
    text
}

/// 将 reqwest 错误映射为项目统一的 ErrorType，并记一条日志
pub(super) fn map_err(error: reqwest::Error) -> ErrorType {
    let err = ErrorType::HttpError(HttpErrorData {
        error: err_text(&error),
        url: match error.url() {
            Some(url) => url.to_string(),
            None => Default::default(),
        },
        status: None,
    });
    log_request_error(&err);
    err
}

/// 响应状态异常转 HttpError
pub(crate) fn http_error(error: String, url: String, status: Option<u16>) -> ErrorType {
    ErrorType::HttpError(HttpErrorData { error, url, status })
}

/// 出错原因转 SerializerError
pub(crate) fn serializer_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::SerializerError(ErrorData::from_error(err))
}
