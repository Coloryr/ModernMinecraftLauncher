//! 限流
//!
//! 按 URL 的每分钟配额节流，超限时排队等待。

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

use mml_names::i18_items::error_type::CoreResult;

use super::abort::{ABORT_GEN, abort_error, abort_rx, abortable};
use super::client::Client;
use super::error::map_err;
use super::response::handle_response;

/// 请求速率限制器
///
/// 按固定时间窗口实现，限制每分钟的最大请求数。
/// 当达到上限后，后续请求将等待下一个时间窗口。
pub(super) struct RateLimiter {
    /// 每分钟允许的最大请求数
    max_requests: u32,
    /// 当前时间窗口的起始时刻
    window_start: Instant,
    /// 当前窗口内已发出的请求数
    request_count: u32,
}

impl RateLimiter {
    /// 创建新的速率限制器
    ///
    /// - `max_requests`: 每分钟允许的最大请求数
    fn new(max_requests: u32) -> Self {
        Self {
            max_requests,
            window_start: Instant::now(),
            request_count: 0,
        }
    }

    /// 更新最大请求数限制
    ///
    /// - `max_requests`: 每分钟允许的最大请求数
    fn update_limit(&mut self, max_requests: u32) {
        self.max_requests = max_requests;
    }

    /// 尝试获取一个请求槽位；当前窗口已满则等到下一个窗口
    async fn acquire(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);

        // 已经过去一分钟就重置窗口
        if elapsed >= Duration::from_secs(60) {
            self.window_start = now;
            self.request_count = 0;
        }

        if self.request_count >= self.max_requests {
            // 等满当前窗口，醒来后开一个新窗口
            let wait_time = Duration::from_secs(60) - elapsed;
            tokio::time::sleep(wait_time).await;
            self.window_start = Instant::now();
            self.request_count = 0;
        }

        self.request_count += 1;
    }
}

impl Client {
    /// 限速等待，可被全局中断打断
    ///
    /// 抽出来给三个 `*_limited` 共用：改代理时如果卡在配额等待上，也应该立刻放弃。
    ///
    /// - `url`: 中断时写进错误里的请求地址
    /// - `max_per_minute`: 每分钟最大请求数，与上次不同则就地更新限制器
    ///
    /// # 返回值
    ///
    /// 等到配额返回 `Ok(())`；等待期间发生中断则返回中断错误
    pub(super) async fn acquire_quota(&self, url: &str, max_per_minute: u32) -> CoreResult<()> {
        let mut guard = self.rate_limiter.lock().await;
        match *guard {
            Some(ref mut limiter) => limiter.update_limit(max_per_minute),
            None => *guard = Some(RateLimiter::new(max_per_minute)),
        }
        let Some(limiter) = guard.as_mut() else {
            return Ok(());
        };

        let generation = ABORT_GEN.load(Ordering::SeqCst);
        let mut rx = abort_rx();
        // 订阅前就已中断：立刻放弃，不用再等配额
        if *rx.borrow() != generation {
            return Err(abort_error(url));
        }
        tokio::select! {
            _ = limiter.acquire() => Ok(()),
            changed = rx.changed() => {
                if changed.is_err() || *rx.borrow() != generation {
                    Err(abort_error(url))
                } else {
                    // 无关通知：把配额等完，acquire 已消费一次，补一次等待即可
                    limiter.acquire().await;
                    Ok(())
                }
            }
        }
    }

    /// 发送带速率限制的 GET 请求，返回反序列化的 JSON
    ///
    /// - `url`: 请求地址
    /// - `max_per_minute`: 每分钟最大请求数
    ///
    /// # 返回值
    ///
    /// 响应体反序列化成 `T`
    pub async fn get_json_limited<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        max_per_minute: u32,
    ) -> CoreResult<T> {
        self.acquire_quota(url, max_per_minute).await?;
        // 先在本方法里把请求构造好：RequestBuilder 已经拥有 URL / 序列化后的 body，
        // 之后 spawn 的 future 不再借用任何局部变量，`abortable` 要求 'static
        let req = self.inner.get(url);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(handle_response(resp)).await
    }

    /// 发送带速率限制的 GET 请求，返回响应体文本
    ///
    /// - `url`: 请求地址
    /// - `max_per_minute`: 每分钟最大请求数
    ///
    /// # 返回值
    ///
    /// 返回响应体文本
    pub async fn get_text_limited(&self, url: &str, max_per_minute: u32) -> CoreResult<String> {
        self.acquire_quota(url, max_per_minute).await?;
        let req = self.inner.get(url);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(async move { resp.text().await.map_err(map_err) }).await
    }

    /// 发送带速率限制的 GET 请求，返回响应体字节
    ///
    /// - `url`: 请求地址
    /// - `max_per_minute`: 每分钟最大请求数
    ///
    /// # 返回值
    ///
    /// 返回响应体字节
    pub async fn get_bytes_limited(&self, url: &str, max_per_minute: u32) -> CoreResult<Vec<u8>> {
        self.acquire_quota(url, max_per_minute).await?;
        let req = self.inner.get(url);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(async move { resp.bytes().await.map_err(map_err) })
            .await
            .map(|data| data.to_vec())
    }
}
