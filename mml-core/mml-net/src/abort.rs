//! 请求中断
//!
//! 全局世代号加任务表，一次中断取消所有在途请求。

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use super::error::http_error;
use super::log_request_error;

/// 当前"请求世代"：每次 [`rebuild`] 自增，用于一次性中断所有在途请求
///
/// 用 `watch` 广播而不是 `Notify::notify_waiters()`：后者只唤醒当时已在队列里的
/// waiter 且不保存许可，而 `Notified` 要到第一次被 poll 才登记，晚一步订阅的请求
/// 会永远错过中断；`watch` 存最新值，晚订阅者 `changed()` 立刻返回。
pub(super) static ABORT_GEN: AtomicU64 = AtomicU64::new(0);
/// 世代广播通道的发送端，用于广播新的世代号
static ABORT_TX: OnceLock<tokio::sync::watch::Sender<u64>> = OnceLock::new();

/// 在途请求任务的取消句柄表
///
/// [`abortable`] 每提交一个请求就把 `AbortHandle` 登记进来，任务结束后清理；
/// [`abort_all`] 遍历整表 `abort()`，这是唯一不依赖"目标 future 被 poll"的取消手段。
static ABORT_TASKS: OnceLock<std::sync::Mutex<Vec<tokio::task::AbortHandle>>> = OnceLock::new();

/// 取取消句柄表，首次调用时建表
pub(super) fn abort_tasks() -> &'static std::sync::Mutex<Vec<tokio::task::AbortHandle>> {
    ABORT_TASKS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// 登记一个在途任务
///
/// - `handle`: 该请求任务的取消句柄
pub(super) fn register_task(handle: tokio::task::AbortHandle) {
    if let Ok(mut list) = abort_tasks().lock() {
        // 顺手清掉已结束的，避免表无限增长；AbortHandle 无法精确移除指定项，按 is_finished 过滤够用
        list.retain(|h| !h.is_finished());
        list.push(handle);
    }
}

/// 清理已结束的任务，请求正常完成时调用
pub(super) fn prune_tasks() {
    if let Ok(mut list) = abort_tasks().lock() {
        list.retain(|h| !h.is_finished());
    }
}

/// 强制取消所有在途请求：遍历登记表逐个 `abort()`
///
/// # 返回值
///
/// 返回被取消的任务数，用于日志
pub(super) fn abort_all_tasks() -> usize {
    let tasks: Vec<tokio::task::AbortHandle> = match abort_tasks().lock() {
        Ok(mut list) => std::mem::take(&mut *list),
        Err(_) => return 0,
    };
    let count = tasks.len();
    for handle in tasks {
        handle.abort();
    }
    count
}

/// 取世代广播通道的发送端
pub(super) fn abort_tx() -> &'static tokio::sync::watch::Sender<u64> {
    ABORT_TX.get_or_init(|| {
        let (tx, _rx) = tokio::sync::watch::channel(0u64);
        tx
    })
}

/// 世代广播通道的接收端，每次调用订阅一个新的接收者
///
/// `changed()` 只在**调用方持有的这个接收者**还没见过的新值出现时才返回，
/// 所以不同请求之间互不影响。
pub(super) fn abort_rx() -> tokio::sync::watch::Receiver<u64> {
    abort_tx().subscribe()
}

/// 构造"请求已中断"错误，并记一条日志
///
/// - `url`: 请求地址；中断发生在请求任务内部时取不到，传空串
///
/// # 返回值
///
/// 返回 [`ErrorType::Aborted`]
pub(super) fn abort_error(url: &str) -> ErrorType {
    let err = ErrorType::Aborted {
        url: url.to_string(),
    };
    log_request_error(&err);
    err
}

/// 自轮询间隔：请求每隔这么久自查一次世代号
///
/// 取 100ms，用户点保存后 0.1 秒内所有在途请求都会停；定时器由各请求自己的
/// runtime 驱动，不依赖跨线程唤醒，因此一定能生效。
const ABORT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// 这个错误是不是"被全局中断"造成的
///
/// 供上层区分两种失败：中断意味着用户改了代理，应当整体放弃、让用户重试；
/// 普通网络错误只是某一项查不到，按原有逻辑跳过即可。像加载器支持列表那种
/// "六路并发、单路失败不算失败"的地方必须用这个判断，否则中断错误会被
/// `unwrap_or(false)` 吞掉，外面既看不到错误、进度条也继续爬。
///
/// - `err`: 待判定的错误
///
/// # 返回值
///
/// 错误是 [`ErrorType::Aborted`] 时返回 `true`
pub fn is_aborted(err: &ErrorType) -> bool {
    // 按变体判定而不是比文案：重建文案也不会漏判
    matches!(err, ErrorType::Aborted { .. })
}

/// 把一次请求与"全局中断"绑在一起
///
/// 请求正常完成返回其结果；中途 [`rebuild`] 被调用，即用户改了代理，则立刻返回
/// [`ErrorType::Aborted`]。必须用 `spawn` + `AbortHandle`：写在 future 内部的检查都要求这个
/// future 还被 poll，而实测出现过请求从未被调度的情况，只有 runtime 层面的强制取消
/// 能覆盖它。卡死兜底仍交给 builder 上的 `read_timeout`，不加请求总超时。
///
/// - `fut`: 一次请求的 future，必须是 `'static`，因为它会被 spawn 成独立任务
///
/// # 返回值
///
/// 正常完成返回请求结果；收到中断或任务被强制取消时返回 [`ErrorType::Aborted`] 对应的错误
pub(super) async fn abortable<F, T>(fut: F) -> CoreResult<T>
where
    F: std::future::Future<Output = CoreResult<T>> + Send + 'static,
    T: Send + 'static,
{
    let entered_gen = ABORT_GEN.load(Ordering::SeqCst);

    // 进入时就已经过期，中断发生在提交之前：直接失败，不用起任务
    if ABORT_GEN.load(Ordering::SeqCst) != entered_gen {
        return Err(abort_error(""));
    }

    // 把请求放进独立任务：这样即使它卡住不让出，也能被 abort() 强制取消
    let handle = tokio::spawn(fut);
    let abort_handle = handle.abort_handle();
    register_task(abort_handle.clone());

    let result = tokio::select! {
        joined = handle => {
            // 任务结束：清理登记表里已完成项
            prune_tasks();
            match joined {
                Ok(res) => {
                    res
                }
                // 被 abort()：JoinError::is_cancelled
                Err(err) if err.is_cancelled() => Err(abort_error("")),
                Err(err) => {
                    // panic 等异常：当成请求失败，别把启动器带崩
                    let mapped = http_error(err.to_string(), String::new(), None);
                    log_request_error(&mapped);
                    Err(mapped)
                }
            }
        }
        // 世代变化，即保存了代理：强制取消任务并立即返回
        _ = wait_abort(entered_gen) => {
            abort_handle.abort();
            prune_tasks();
            Err(abort_error(""))
        }
    };

    result
}

/// 等到世代号变得与 `entered_gen` 不同，可被 `abort_all` 唤醒
///
/// 定时自查加通道通知双路：通道是快路径，定时器保证通知没送达时也一定能发现。
///
/// - `entered_gen`: 进入请求时的世代号
pub(super) async fn wait_abort(entered_gen: u64) {
    let mut rx = abort_rx();
    loop {
        if ABORT_GEN.load(Ordering::SeqCst) != entered_gen {
            return;
        }
        tokio::select! {
            _ = tokio::time::sleep(ABORT_POLL_INTERVAL) => {}
            _ = rx.changed() => {}
        }
    }
}
