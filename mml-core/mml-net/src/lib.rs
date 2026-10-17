//! 网络请求模块
//!
//! 本模块是启动器的 HTTP 客户端层，封装了 `reqwest` 库，
//! 提供统一的网络请求接口，支持代理配置和多种请求方式。
//!
//! # 双客户端设计
//!
//! 本模块维护两个独立的 HTTP 客户端实例：
//!
//! - **WORK_CLIENT** — 用于一般网络请求，如下载资源、调用 API
//! - **LOGIN_CLIENT** — 用于登录相关请求，如 OAuth、Yggdrasil 认证
//!
//! 两者可独立配置不同的代理策略，确保登录流量和下载流量可以走不同的网络通道。
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`mojang_api`] | Mojang 官方 API |
//! | [`curseforge_api`] | CurseForge API |
//! | [`modrinth_api`] | Modrinth API |
//! | [`fabric_api`] / [`quilt_api`] | 模组加载器 API |
//! | [`optifine_api`] | OptiFine 下载 |
//! | [`authlib_api`] | Authlib-Injector 下载 |
//! | [`adoptium_api`] | Adoptium Java 下载 |
//! | [`nide8_api`] | 统一通行证 API |
//! | [`liteloader_api`] | LiteLoader API |
//! | [`openfrp_api`] / [`sakurafrp_api`] | 联机平台 OpenFrp / SakuraFrp 的节点列表 |
//! | [`chunkbase_api`] | Chunkbase 相关接口 |
//! | [`urls`] | URL 常量定义 |
//! | [`url_helper`] | URL 构建辅助函数 |
//! | [`maven_utils`] | Maven 坐标工具 |
//! | [`input_file`] | 输入文件抽象 |

use mml_base::serialize_tools;
use mml_config::config_obj::{HttpObj, ProxyState, ProxyType};
use mml_names::i18;
use mml_names::i18_items::error_type::{CoreResult, ErrorType, HttpErrorData};
use mml_names::i18_items::info_type::InfoType;
use reqwest::header::{ETAG, HeaderMap, HeaderValue, IF_NONE_MATCH, USER_AGENT};
use reqwest::{Proxy, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::ser::SerializeTuple;
use serde::{Serialize, Serializer};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub mod adoptium_api;
pub mod authlib_api;
pub mod chunkbase_api;
pub mod curseforge_api;
pub mod fabric_api;
pub mod foojay_api;
pub mod input_file;
pub mod liteloader_api;
pub mod maven_utils;
pub mod mclo_api;
pub mod modrinth_api;
pub mod mojang_api;
pub mod nide8_api;
pub mod openfrp_api;
pub mod openj9_api;
pub mod optifine_api;
pub mod quilt_api;
pub mod sakurafrp_api;
pub mod url_helper;
pub mod urls;
pub mod zulu_api;

/// 默认 HTTP 超时时间，单位秒：连接超时与单次读取超时共用
///
/// 注意是 `read_timeout` 单次读空闲超时，而非请求总超时：总超时会掐断
/// 慢速网络下的大文件下载，客户端 jar 有 40+MB
const DEFAULT_TIMEOUT: u64 = 10;

/// 默认 User-Agent 标识
const DEFAULT_USER_AGENT: &str = "mml/1.0.0";

/// 记录一次失败的请求
///
/// 请求失败一律在这里落一条错误日志，`logs.log` 里就能看到地址与原因。
///
/// - `err`: 失败原因
pub(crate) fn log_request_error(err: &ErrorType) {
    mml_log::error_type(err.clone());
}

/// 将 reqwest 错误映射为项目统一的 ErrorType，并记一条日志
fn map_err(error: reqwest::Error) -> ErrorType {
    let err = ErrorType::HttpError(HttpErrorData {
        error: error.to_string(),
        url: match error.url() {
            Some(url) => url.to_string(),
            None => Default::default(),
        },
        status: None,
    });
    log_request_error(&err);
    err
}

/// 请求速率限制器
///
/// 按固定时间窗口实现，限制每分钟的最大请求数。
/// 当达到上限后，后续请求将等待下一个时间窗口。
struct RateLimiter {
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

/// HTTP 客户端封装
///
/// 对 `reqwest::Client` 的二次包装，增加了超时配置、默认请求头、代理支持
/// 以及可选的请求速率限制。
pub struct Client {
    /// 底层 reqwest 客户端，超时与默认请求头都已配好
    inner: reqwest::Client,
    /// 可选的速率限制器，锁内为 `None` 表示不限制
    rate_limiter: Arc<Mutex<Option<RateLimiter>>>,
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

/// 表单请求的一个键值对
pub struct FormField<'a> {
    /// 表单字段名，必填，如 `client_id`
    pub key: &'a str,
    /// 表单字段值，按 `application/x-www-form-urlencoded` 编码后进请求体
    pub value: &'a str,
}

impl Serialize for FormField<'_> {
    /// 按 2 元组而不是结构体序列化
    ///
    /// `serde_urlencoded` 只把 2 元组当键值对，具名结构体走 `serialize_struct` 会被它判为
    /// unsupported pair，请求直接失败；按元组序列化后线格式与原先的 `(&str, &str)` 完全一致。
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(self.key)?;
        tuple.serialize_element(self.value)?;
        tuple.end()
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
    async fn acquire_quota(&self, url: &str, max_per_minute: u32) -> CoreResult<()> {
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
        let err = ErrorType::HttpError(HttpErrorData {
            error,
            url,
            status: Some(status.as_u16()),
        });
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

    /// 发送 POST 请求，JSON 请求体，返回原始响应
    ///
    /// - `url`: 请求地址
    /// - `body`: 请求体，序列化成 JSON
    ///
    /// # 返回值
    ///
    /// 返回原始响应，状态码由调用方自行判断
    pub async fn post_json_get_req<B: Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> CoreResult<reqwest::Response> {
        // `.json()` 在此就把 body 序列化成拥有的字节，builder 不借用 `body`
        let req = self.inner.post(url).json(body);
        abortable(async move { req.send().await.map_err(map_err) }).await
    }

    /// 发送 POST 请求，multipart 表单上传文件，带 Bearer 鉴权
    ///
    /// - `url`: 请求地址
    /// - `form`: multipart 表单，文件与字段都已装好
    /// - `token`: Bearer 令牌
    ///
    /// # 返回值
    ///
    /// 返回原始响应
    pub async fn post_multipart(
        &self,
        url: &str,
        form: reqwest::multipart::Form,
        token: &str,
    ) -> CoreResult<reqwest::Response> {
        // `bearer_auth` / `multipart` 都是即刻转换，token 拷进 header、form 被 move，
        // builder 构造完成后不借用任何入参
        let req = self.inner.post(url).bearer_auth(token).multipart(form);
        abortable(async move { req.send().await.map_err(map_err) }).await
    }

    /// 发送 POST 请求，JSON 请求体，返回响应文本
    ///
    /// - `url`: 请求地址
    /// - `body`: 请求体，序列化成 JSON
    ///
    /// # 返回值
    ///
    /// 返回响应体文本
    pub async fn post_json_get_text<B: Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> CoreResult<String> {
        let req = self.inner.post(url).json(body);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(async move { resp.text().await.map_err(map_err) }).await
    }

    /// 发送 POST 请求，JSON 请求体，返回响应字节
    ///
    /// - `url`: 请求地址
    /// - `body`: 请求体，序列化成 JSON
    ///
    /// # 返回值
    ///
    /// 返回响应体字节
    pub async fn post_json_get_bytes<B: Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> CoreResult<Vec<u8>> {
        let req = self.inner.post(url).json(body);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(async move { resp.bytes().await.map_err(map_err) })
            .await
            .map(|data| data.to_vec())
    }

    /// 发送 POST 请求，JSON 请求体，返回反序列化的 JSON
    ///
    /// - `url`: 请求地址
    /// - `json`: 请求体，序列化成 JSON
    ///
    /// # 返回值
    ///
    /// 响应体反序列化成 `T`
    pub async fn post_json_get_json<B: Serialize, T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        json: &B,
    ) -> CoreResult<T> {
        let req = self.inner.post(url).json(json);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(handle_response(resp)).await
    }

    /// 发送带速率限制的 POST 请求，JSON 请求体，返回反序列化的 JSON
    ///
    /// - `url`: 请求地址
    /// - `json`: 请求体，序列化成 JSON
    /// - `max_per_minute`: 每分钟最大请求数
    ///
    /// # 返回值
    ///
    /// 响应体反序列化成 `T`
    pub async fn post_json_get_json_limited<B: Serialize, T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        json: &B,
        max_per_minute: u32,
    ) -> CoreResult<T> {
        self.acquire_quota(url, max_per_minute).await?;
        let req = self.inner.post(url).json(json);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(handle_response(resp)).await
    }

    /// 发送 POST 请求，表单请求体，返回反序列化的 JSON
    ///
    /// - `url`: 请求地址
    /// - `params`: 表单字段列表，见 [`FormField`]
    ///
    /// # 返回值
    ///
    /// 响应体反序列化成 `T`
    pub async fn post_form_get_json<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        params: &[FormField<'_>],
    ) -> CoreResult<T> {
        // `.form()` 即刻把参数编码进 body，builder 不借用 `params`
        let req = self.inner.post(url).form(params);
        let resp = abortable(async move { req.send().await.map_err(map_err) }).await?;
        abortable(handle_response(resp)).await
    }

    /// 发送 POST 请求，表单请求体，返回原始响应
    ///
    /// 用于需要对非 2xx 响应自行解析 body 的场景，如 OAuth 设备码轮询中
    /// 400 加 `authorization_pending` 属正常中间态，不应视为请求失败。
    ///
    /// - `url`: 请求地址
    /// - `params`: 表单字段列表，见 [`FormField`]
    ///
    /// # 返回值
    ///
    /// 返回原始响应，状态码由调用方自行判断
    pub async fn post_form_get_req(
        &self,
        url: &str,
        params: &[FormField<'_>],
    ) -> CoreResult<reqwest::Response> {
        let req = self.inner.post(url).form(params);
        abortable(async move { req.send().await.map_err(map_err) }).await
    }
}

impl Default for Client {
    /// 默认客户端：跟随系统代理
    fn default() -> Self {
        Self::new(ProxyState::Auto)
    }
}

/// 带 ETag 缓存校验的下载结果，[`Client::get_bytes_with_check`] 返回
pub struct AssetCheckResponse {
    /// 服务器返回 304，表示本地缓存的资源仍然有效
    pub not_modified: bool,
    /// ETag 响应头；S3 / R2 类存储在非分片上传时就是内容的 MD5
    pub etag: Option<String>,
    /// 响应体字节，304 时为空
    pub data: Vec<u8>,
}

/// 全局通用 HTTP 客户端，用于资源下载与一般 API 调用
static WORK_CLIENT: OnceLock<RwLock<Arc<Client>>> = OnceLock::new();
/// 全局登录 HTTP 客户端，用于 OAuth、Yggdrasil 认证
static LOGIN_CLIENT: OnceLock<RwLock<Arc<Client>>> = OnceLock::new();

/// 当前"请求世代"：每次 [`rebuild`] 自增，用于一次性中断所有在途请求
///
/// 用 `watch` 广播而不是 `Notify::notify_waiters()`：后者只唤醒当时已在队列里的
/// waiter 且不保存许可，而 `Notified` 要到第一次被 poll 才登记，晚一步订阅的请求
/// 会永远错过中断；`watch` 存最新值，晚订阅者 `changed()` 立刻返回。
static ABORT_GEN: AtomicU64 = AtomicU64::new(0);
/// 世代广播通道的发送端，用于广播新的世代号
static ABORT_TX: OnceLock<tokio::sync::watch::Sender<u64>> = OnceLock::new();

/// 在途请求任务的取消句柄表
///
/// [`abortable`] 每提交一个请求就把 `AbortHandle` 登记进来，任务结束后清理；
/// [`abort_all`] 遍历整表 `abort()`，这是唯一不依赖"目标 future 被 poll"的取消手段。
static ABORT_TASKS: OnceLock<std::sync::Mutex<Vec<tokio::task::AbortHandle>>> = OnceLock::new();

/// 取取消句柄表，首次调用时建表
fn abort_tasks() -> &'static std::sync::Mutex<Vec<tokio::task::AbortHandle>> {
    ABORT_TASKS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// 登记一个在途任务
///
/// - `handle`: 该请求任务的取消句柄
fn register_task(handle: tokio::task::AbortHandle) {
    if let Ok(mut list) = abort_tasks().lock() {
        // 顺手清掉已结束的，避免表无限增长；AbortHandle 无法精确移除指定项，按 is_finished 过滤够用
        list.retain(|h| !h.is_finished());
        list.push(handle);
    }
}

/// 清理已结束的任务，请求正常完成时调用
fn prune_tasks() {
    if let Ok(mut list) = abort_tasks().lock() {
        list.retain(|h| !h.is_finished());
    }
}

/// 强制取消所有在途请求：遍历登记表逐个 `abort()`
///
/// # 返回值
///
/// 返回被取消的任务数，用于日志
fn abort_all_tasks() -> usize {
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
fn abort_tx() -> &'static tokio::sync::watch::Sender<u64> {
    ABORT_TX.get_or_init(|| {
        let (tx, _rx) = tokio::sync::watch::channel(0u64);
        tx
    })
}

/// 世代广播通道的接收端，每次调用订阅一个新的接收者
///
/// `changed()` 只在**调用方持有的这个接收者**还没见过的新值出现时才返回，
/// 所以不同请求之间互不影响。
fn abort_rx() -> tokio::sync::watch::Receiver<u64> {
    abort_tx().subscribe()
}

/// 被中断的请求返回的错误文案，[`is_aborted`] 按它判定中断
pub const ABORT_MSG: &str = "请求已中断（代理/网络设置已更新）";

/// 构造"请求已中断"错误，并记一条日志
///
/// - `url`: 请求地址；中断发生在请求任务内部时取不到，传空串
///
/// # 返回值
///
/// 返回 error 为 [`ABORT_MSG`] 的 HTTP 错误
fn abort_error(url: &str) -> ErrorType {
    let err = ErrorType::HttpError(HttpErrorData {
        url: url.to_string(),
        error: ABORT_MSG.to_string(),
        status: None,
    });
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
/// 错误文本等于 [`ABORT_MSG`] 时返回 `true`
pub fn is_aborted(err: &ErrorType) -> bool {
    match err {
        ErrorType::HttpError(data) => data.error == ABORT_MSG,
        _ => false,
    }
}

/// 把一次请求与"全局中断"绑在一起
///
/// 请求正常完成返回其结果；中途 [`rebuild`] 被调用，即用户改了代理，则立刻返回
/// [`ABORT_MSG`]。必须用 `spawn` + `AbortHandle`：写在 future 内部的检查都要求这个
/// future 还被 poll，而实测出现过请求从未被调度的情况，只有 runtime 层面的强制取消
/// 能覆盖它。卡死兜底仍交给 builder 上的 `read_timeout`，不加请求总超时。
///
/// - `fut`: 一次请求的 future，必须是 `'static`，因为它会被 spawn 成独立任务
///
/// # 返回值
///
/// 正常完成返回请求结果；收到中断或任务被强制取消时返回 [`ABORT_MSG`] 对应的错误
async fn abortable<F, T>(fut: F) -> CoreResult<T>
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
                    let mapped = ErrorType::HttpError(HttpErrorData {
                        url: String::new(),
                        error: err.to_string(),
                        status: None,
                    });
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
async fn wait_abort(entered_gen: u64) {
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

/// 按当前配置构建一个客户端
///
/// 抽出来是为了让 [`init`] 与 [`rebuild`] 共用同一套「读配置 → 选代理」逻辑：
/// `User` 用用户填的地址，其余策略交给 `Client::new`。
///
/// - `state`: 代理策略，决定走不走用户填的地址
/// - `ptype`: 用户代理的类型，仅在 `state` 为 `User` 时使用
/// - `http`: 网络配置，代理地址与认证信息都在里面
///
/// # 返回值
///
/// 返回建好的客户端
fn build_client(state: ProxyState, ptype: ProxyType, http: &HttpObj) -> Client {
    if state == ProxyState::User {
        Client::new_proxy(
            ptype,
            &http.proxy_ip,
            http.proxy_port,
            &http.proxy_user,
            &http.proxy_password,
        )
    } else {
        Client::new(state)
    }
}

/// 把代理配置描述成一行日志文本，不含密码
///
/// - `state`: 代理策略
/// - `ptype`: 代理类型，仅在 `state` 为 `User` 时体现
/// - `http`: 网络配置，只读其中的代理地址与用户名
///
/// # 返回值
///
/// 返回形如 `User/http 127.0.0.1:7890 认证=有` 的描述
fn describe_proxy(state: ProxyState, ptype: ProxyType, http: &HttpObj) -> String {
    match state {
        ProxyState::User => i18::get_info(InfoType::ProxyUser {
            kind: match ptype {
                ProxyType::Http => "http",
                ProxyType::Sock4 => "socks4",
                ProxyType::Sock5 => "socks5",
            }
            .to_string(),
            ip: http.proxy_ip.clone(),
            port: http.proxy_port,
            auth: !http.proxy_user.is_empty(),
        }),
        ProxyState::None => i18::get_info(InfoType::ProxyNone),
        ProxyState::Auto => i18::get_info(InfoType::ProxyAuto),
    }
}

/// 初始化 HTTP 客户端
///
/// 根据配置中的代理设置分别创建通用客户端和登录客户端。
/// 应在程序启动时调用一次；之后改代理设置请用 [`rebuild`]。
pub fn init() {
    let config = mml_config::read_config();
    let http = &config.http;

    WORK_CLIENT.get_or_init(|| {
        RwLock::new(Arc::new(build_client(
            http.work_proxy,
            http.work_proxy_type,
            http,
        )))
    });
    LOGIN_CLIENT.get_or_init(|| {
        RwLock::new(Arc::new(build_client(
            http.login_proxy,
            http.login_proxy_type,
            http,
        )))
    });

    mml_log::info_type(InfoType::HttpClientInit {
        work: describe_proxy(http.work_proxy, http.work_proxy_type, http),
        login: describe_proxy(http.login_proxy, http.login_proxy_type, http),
    });
}

/// 按当前配置**重建**两个 HTTP 客户端，改代理设置后调用
///
/// 客户端存在 `RwLock<Arc<Client>>` 里，保存网络设置后调本函数即可立刻换上新配置，
/// 不必重启启动器。已经取走的 `Arc` 不受影响，仍指向旧客户端，但在途请求会先被中断，
/// 之后新取的请求一律用新客户端。
///
/// [`init`] 尚未调用时什么都不做：配置还没读，此时也没有请求在跑。
pub fn rebuild() {
    // 先中断所有在途请求，含限速等待，再换客户端：顺序反了的话，
    // 旧请求可能刚醒过来又用旧客户端发出去
    let aborted_gen = abort_all();
    mml_log::info_type(InfoType::HttpClientRestart {
        generation: aborted_gen,
    });

    let config = mml_config::read_config();
    let http = &config.http;

    if let Some(lock) = WORK_CLIENT.get() {
        let client = Arc::new(build_client(http.work_proxy, http.work_proxy_type, http));
        if let Ok(mut guard) = lock.write() {
            *guard = client;
        }
    }
    if let Some(lock) = LOGIN_CLIENT.get() {
        let client = Arc::new(build_client(http.login_proxy, http.login_proxy_type, http));
        if let Ok(mut guard) = lock.write() {
            *guard = client;
        }
    }

    mml_log::info_type(InfoType::HttpClientRestartDone {
        work: describe_proxy(http.work_proxy, http.work_proxy_type, http),
        login: describe_proxy(http.login_proxy, http.login_proxy_type, http),
    });
}

/// 中断所有在途请求
///
/// 自增世代号并**广播**给所有正在等待的请求，被中断的请求返回 [`ABORT_MSG`]
/// 对应的错误，调用方按"请求失败"处理即可，用户改代理后重试就是想要的结果。
/// 用 `watch` 广播存值、不丢信号，理由见 [`ABORT_GEN`]。
///
/// 单独暴露出来是为了让 [`rebuild`] 之外的场景，如退出登录、切源，也能用。
///
/// # 返回值
///
/// 返回自增后的世代号，用于日志排查"到底有没有真的中断"
pub fn abort_all() -> u64 {
    let generation = ABORT_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    // 广播最新世代，watch 存值，晚订阅的请求也能读到；接收端已全部丢弃时返回 Err，忽略即可
    let _ = abort_tx().send(generation);
    // **强制取消全部在途任务**：这是不依赖"请求被 poll"的硬手段
    abort_all_tasks();

    generation
}

/// 获取全局通用 HTTP 客户端
///
/// # 返回值
///
/// 返回当前生效的通用客户端，用于资源下载和一般 API 请求
pub fn get_work_client() -> Arc<Client> {
    WORK_CLIENT
        .get()
        .and_then(|lock| lock.read().ok().map(|guard| guard.clone()))
        .unwrap()
}

/// 获取全局登录 HTTP 客户端
///
/// # 返回值
///
/// 返回当前生效的登录客户端，用于 OAuth / Yggdrasil 认证请求
pub fn get_login_client() -> Arc<Client> {
    LOGIN_CLIENT
        .get()
        .and_then(|lock| lock.read().ok().map(|guard| guard.clone()))
        .unwrap()
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
        let err = ErrorType::HttpError(HttpErrorData {
            error,
            url,
            status: Some(status.as_u16()),
        });
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
