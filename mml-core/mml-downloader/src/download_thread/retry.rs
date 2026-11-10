//! 下载失败的重试判定
//!
//! 失败时把文件名、地址与错误一起写进日志，并按累计错误次数决定是否继续重试。

use super::*;

/// 记一条失败日志并累加重试次数
///
/// 光记错误类型看不出是哪个文件、哪个地址；下载失败多半出在地址或网络上，
/// 这两项必须跟着错误与重试次数一起落盘。
///
/// - `item`: 失败的文件项
/// - `err`: 失败原因
/// - `times`: 当前错误次数，可变引用，会自增
///
/// # 返回值
///
/// `true` — 应停止重试，`false` — 可继续重试
pub(crate) fn is_need_err(
    item: &mml_base::file_item::FileItemObj,
    err: ErrorType,
    times: &mut i32,
) -> bool {
    *times += 1;
    mml_log::error_type(ErrorType::DownloadItemFail(DownloadItemFailData {
        name: item.name.clone(),
        times: *times,
        error: mml_names::i18::get_error(err),
        url: item.url.clone(),
    }));
    *times > 5
}
