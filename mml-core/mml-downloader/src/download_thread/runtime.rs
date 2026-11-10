//! 下载线程的异步运行时桥接
//!
//! 全部下载线程共用一份 `current_thread` 运行时，用 `block_on` 把异步 HTTP 请求转成同步调用；
//! 可取消版本让请求与取消令牌竞速，避免停止下载时被在途请求拖住。

use super::*;

/// 用于在下载线程中执行异步任务的 Tokio 运行时
///
/// 全部下载线程共用 `OnceLock` 里这一份 `current_thread` 运行时，
/// 各线程通过 `block_on` 把异步 HTTP 请求转成同步调用。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 在当前线程中阻塞执行异步任务
///
/// - `f`: 待执行的异步任务
///
/// # 返回值
///
/// 返回异步任务的输出
pub(crate) fn block_on<F: Future>(f: F) -> F::Output {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
        })
        .block_on(f)
}

/// 在当前线程中阻塞执行一个 HTTP 请求，任务被取消时立即放弃它
///
/// 请求挂住时（服务端不回包、连接半开）自身要等读超时才结束，而 `stop()` 会 join
/// 下载线程，取消只靠请求结束后的标志检查就来不及了 —— 退出会跟着一起卡住。
/// 这里让请求与取消令牌竞速，取消时直接丢弃请求 Future，连接随之断开。
///
/// - `task`: 请求所属的下载任务，用它的取消令牌
/// - `fut`: 待执行的请求 Future
///
/// # 返回值
///
/// `None` 表示任务已被取消，请求结果作废；其余情况是请求自身的输出
pub(crate) fn block_on_cancellable<F, T>(task: &Arc<DownloadTask>, fut: F) -> Option<T>
where
    F: Future<Output = T>,
{
    let cancel = task.cancel_token();
    block_on(async move {
        tokio::select! {
            res = fut => Some(res),
            _ = cancel.cancelled() => None,
        }
    })
}
