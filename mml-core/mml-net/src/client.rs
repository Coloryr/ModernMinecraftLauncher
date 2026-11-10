//! HTTP 客户端
//!
//! 统一的请求入口：GET、重试、代理与客户端构建。

use std::fmt;
use std::sync::Arc;

use std::time::Duration;
use tokio::sync::Mutex;

use mml_config::config_obj::{ProxyState, ProxyType};
use mml_names::i18_items::error_type::CoreResult;
use reqwest::header::{ETAG, HeaderMap, HeaderValue, IF_NONE_MATCH, USER_AGENT};
use reqwest::{Proxy, Request, Response, StatusCode};
use serde::de::DeserializeOwned;

use super::abort::{abortable, is_aborted};
use super::error::http_error;
use super::error::map_err;
use super::log_request_error;
use super::response::AssetCheckResponse;
use super::response::handle_response;

use super::ratelimit::RateLimiter;
use super::{DEFAULT_TIMEOUT, DEFAULT_USER_AGENT};

/// HTTP 客户端封装
///
/// 对 `reqwest::Client` 的二次包装，增加了超时配置、默认请求头、代理支持
/// 以及可选的请求速率限制。
pub struct Client {
    /// 底层 reqwest 客户端，超时与默认请求头都已配好
    pub(super) inner: reqwest::Client,
    /// 可选的速率限制器，锁内为 `None` 表示不限制
    pub(super) rate_limiter: Arc<Mutex<Option<RateLimiter>>>,
}

impl fmt::Debug for Client {
    /// 手写 Debug：限速器只打印类型名，不展开内部状态
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("inner", &self.inner)
            .field("rate_limiter", &"Arc<Mutex<Option<RateLimiter>>>")
            .finish()
    }
}

impl Client {
    /// 创建 HTTP 客户端
    ///
    /// - `proxy`: 代理策略；`None` 显式禁用代理，其余策略跟随系统代理
    ///
    /// # 返回值
    ///
    /// 返回配好超时与默认请求头的客户端
    pub fn new(proxy: ProxyState) -> Self {
        let mut headers = HeaderMap::new();

        headers.insert(
            USER_AGENT,
            HeaderValue::try_from(DEFAULT_USER_AGENT).unwrap(),
        );

        let builder = reqwest::Client::builder()
            .read_timeout(Duration::from_secs(DEFAULT_TIMEOUT))
            .connect_timeout(Duration::from_secs(DEFAULT_TIMEOUT))
            .default_headers(headers);

        let builder = if proxy == ProxyState::None {
            builder.no_proxy()
        } else {
            builder
        };

        Client {
            inner: builder.build().unwrap(),
            rate_limiter: Arc::new(Mutex::new(None)),
        }
    }

    /// 创建一个使用自定义代理的客户端
    ///
    /// - `ptype`: 代理类型：HTTP / SOCKS4 / SOCKS5
    /// - `ip`: 代理服务器 IP
    /// - `port`: 代理服务器端口
    /// - `user`: 代理认证用户名，空字符串表示无认证
    /// - `pass`: 代理认证密码，`user` 为空时不生效
    ///
    /// # 返回值
    ///
    /// 返回走该代理的客户端
    pub fn new_proxy(
        ptype: ProxyType,
        ip: &String,
        port: u16,
        user: &String,
        pass: &String,
    ) -> Self {
        let mut headers = HeaderMap::new();

        headers.insert(
            USER_AGENT,
            HeaderValue::try_from(DEFAULT_USER_AGENT).unwrap(),
        );

        let proxy = match ptype {
            ProxyType::Http => Proxy::all(format!("http://{}:{}", ip, port)).unwrap(),
            ProxyType::Sock4 => Proxy::all(format!("socks4://{}:{}", ip, port)).unwrap(),
            ProxyType::Sock5 => Proxy::all(format!("socks5://{}:{}", ip, port)).unwrap(),
        };

        let proxy = if !user.is_empty() {
            proxy.basic_auth(user, pass)
        } else {
            proxy
        };

        let builder = reqwest::Client::builder()
            .read_timeout(Duration::from_secs(DEFAULT_TIMEOUT))
            .connect_timeout(Duration::from_secs(DEFAULT_TIMEOUT))
            .default_headers(headers)
            .proxy(proxy);

        Client {
            inner: builder.build().unwrap(),
            rate_limiter: Arc::new(Mutex::new(None)),
        }
    }

    /// 发送自定义 HTTP 请求
    ///
    /// - `build`: 已构造好的请求，方法、URL、请求体都在里面
    ///
    /// # 返回值
    ///
    /// 返回原始响应
    pub async fn send(&self, build: Request) -> CoreResult<Response> {
        let inner = self.inner.clone();
        abortable(async move { inner.execute(build).await.map_err(map_err) }).await
    }

    /// 发送 GET 请求，失败时自动重试一次
    ///
    /// 连接池中的空闲连接可能已被服务端或代理关闭，复用时会立即报
    /// "error sending request"，此时换新连接重试一次即可。
    ///
    /// **被全局中断时不重试**：中断意味着用户改了代理，这次请求已经作废，
    /// 重试会发出一条带着新世代号、不受本次中断影响的请求，导致调用方卡住。
    async fn get_with_retry(&self, url: &str) -> CoreResult<Response> {
        // 先取出需要的所有权：`abortable` 内部会 spawn，闭包必须 'static
        let client = self.inner.clone();
        let url = url.to_string();
        abortable(async move {
            match client.get(&url).send().await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    let mapped = map_err(err);
                    if is_aborted(&mapped) {
                        return Err(mapped);
                    }
                    client.get(&url).send().await.map_err(map_err)
                }
            }
        })
        .await
    }

    /// 发送 GET 请求，返回原始响应
    ///
    /// - `url`: 请求地址
    ///
    /// # 返回值
    ///
    /// 返回原始响应，失败时自动重试一次
    pub async fn get(&self, url: &str) -> CoreResult<Response> {
        self.get_with_retry(url).await
    }

    /// 带 If-None-Match 缓存校验的 GET 下载：发完整请求并取回响应头与响应体
    ///
    /// 本地存有资源的 ETag 时带上做校验；服务器返回 304 表示资源没变、
    /// 响应体为空，返回 200 时响应头里会带新的 ETag。失败时自动重试一次。
    ///
    /// - `url`: 请求地址
    /// - `etag`: 本地已存资源的 ETag，没有就传 `None`
    ///
    /// # 返回值
    ///
    /// 返回 [`AssetCheckResponse`]：304 标记、新 ETag、响应体
    pub async fn get_bytes_with_check(
        &self,
        url: &str,
        etag: Option<&str>,
    ) -> CoreResult<AssetCheckResponse> {
        // 两个 RequestBuilder 都在 spawn 之前构造好，带 ETag 的那个用于首次尝试，
        // 重试时重新发一条同样的请求；构造后它们已不借用 url / etag
        let first = {
            let mut req = self.inner.get(url);
            if let Some(etag) = etag {
                req = req.header(IF_NONE_MATCH, etag);
            }
            req
        };
        let retry = {
            let mut req = self.inner.get(url);
            if let Some(etag) = etag {
                req = req.header(IF_NONE_MATCH, etag);
            }
            req
        };

        let resp = abortable(async move {
            match first.send().await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    // 中断不重试，同 get_with_retry：重试会带新世代号，本次中断对它无效
                    let mapped = map_err(err);
                    if is_aborted(&mapped) {
                        return Err(mapped);
                    }
                    retry.send().await.map_err(map_err)
                }
            }
        })
        .await?;

        let not_modified = resp.status() == StatusCode::NOT_MODIFIED;
        let etag = resp
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        let data = if not_modified {
            Vec::new()
        } else {
            abortable(async move { resp.bytes().await.map_err(map_err) })
                .await?
                .to_vec()
        };

        Ok(AssetCheckResponse {
            not_modified,
            etag,
            data,
        })
    }

    /// 发送 GET 请求，返回响应体文本
    ///
    /// - `url`: 请求地址
    ///
    /// # 返回值
    ///
    /// 返回响应体文本
    pub async fn get_text(&self, url: &str) -> CoreResult<String> {
        let resp = self.get_with_retry(url).await?;
        let out = abortable(async move { resp.text().await.map_err(map_err) }).await;
        out
    }

    /// 发送 GET 请求，只在响应成功时返回响应体文本
    ///
    /// 请求失败时的响应同样带正文，404 多数是一页 HTML 错误页，[`Self::get_text`] 会把它
    /// 当成正常正文返回；只在"成功才有正文"的场合用本方法。
    ///
    /// - `url`: 请求地址
    ///
    /// # 返回值
    ///
    /// 返回响应体文本；HTTP 状态不是 2xx 时返回 [`ErrorType::HttpError`]
    pub async fn get_text_success(&self, url: &str) -> CoreResult<String> {
        let resp = self.get_with_retry(url).await?;
        let status = resp.status();
        if status.is_success() {
            return abortable(async move { resp.text().await.map_err(map_err) }).await;
        }

        // 错误正文照旧取回来写进错误里，便于看清是哪个仓库回的什么
        let url = resp.url().to_string();
        let error = abortable(async move { resp.text().await.map_err(map_err) })
            .await
            .unwrap_or_default();
        let err = http_error(error, url, Some(status.as_u16()));
        log_request_error(&err);
        Err(err)
    }

    /// 发送 GET 请求，返回响应体字节
    ///
    /// - `url`: 请求地址
    ///
    /// # 返回值
    ///
    /// 返回响应体字节
    pub async fn get_bytes(&self, url: &str) -> CoreResult<Vec<u8>> {
        let resp = self.get_with_retry(url).await?;
        let out = abortable(async move { resp.bytes().await.map_err(map_err) })
            .await
            .map(|data| data.to_vec());
        out
    }

    /// 发送 GET 请求，返回反序列化的 JSON
    ///
    /// - `url`: 请求地址
    ///
    /// # 返回值
    ///
    /// 响应体反序列化成 `T`
    pub async fn get_json<T: DeserializeOwned + Send + 'static>(&self, url: &str) -> CoreResult<T> {
        let resp = self.get_with_retry(url).await?;
        abortable(handle_response(resp)).await
    }

    /// 发送带有 Range 头的 GET 请求，用于断点续传
    ///
    /// - `url`: 请求地址
    /// - `pos`: 已下载的字节数，从该位置继续下载
    ///
    /// # 返回值
    ///
    /// 返回原始响应，调用方按 206 / 200 自行决定是续传还是重下
    pub async fn get_ranges(&self, url: &str, pos: u64) -> CoreResult<Response> {
        let req = self
            .inner
            .get(url)
            .header("Range", format!("bytes={}-", pos));
        abortable(async move { req.send().await.map_err(map_err) }).await
    }
}

impl Default for Client {
    /// 默认客户端：跟随系统代理
    fn default() -> Self {
        Self::new(ProxyState::Auto)
    }
}
