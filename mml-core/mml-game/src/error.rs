//! 错误包装的短名
//!
//! `ErrorData::from_error(..)` 直接写在闭包里是两三行噪声，同一个变体还会在多个
//! 文件里各抄一份；收在这里，一个变体一个函数。

use mml_names::i18_items::error_type::{ErrorData, ErrorType};

/// 出错原因转 SerializerError
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn serializer_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::SerializerError(ErrorData::from_error(err))
}

/// 出错原因转 ArchiveReadError
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn archive_read_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::ArchiveReadError(ErrorData::from_error(err))
}

/// 出错原因转 StreamError
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn stream_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::StreamError(ErrorData::from_error(err))
}

/// 出错原因转 SocketError
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn socket_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::SocketError(ErrorData::from_error(err))
}

/// 出错原因转 ThreadError
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn thread_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::ThreadError(ErrorData::from_error(err))
}

/// 出错原因转 TaskError
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn task_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::TaskError(ErrorData::from_error(err))
}

/// 出错原因转 ModrinthAvatarFailed
///
/// - `err`: 出错原因，取它的 `Display` 输出
///
/// # 返回值
///
/// 见 [`ErrorType`]
pub(crate) fn modrinth_avatar_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::ModrinthAvatarFailed(ErrorData::from_error(err))
}
