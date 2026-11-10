//! POST 请求
//!
//! JSON / 表单提交，以及按需解析成文本、字节或结构体。

use mml_names::i18_items::error_type::CoreResult;
use reqwest::header::AUTHORIZATION;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::abort::abortable;
use super::client::Client;
use super::error::map_err;
use super::form::FormField;
use super::response::handle_response;

impl Client {
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

    /// 发送带 Authorization 头的 POST 请求，返回反序列化的 JSON
    ///
    /// OpenFrp 等平台的新接口用登录后拿到的 Authorization 鉴权，请求体为空对象。
    ///
    /// - `url`: 请求地址
    /// - `authorization`: 鉴权头的值，原样放进 `Authorization`
    ///
    /// # 返回值
    ///
    /// 响应体反序列化成 `T`
    pub async fn post_json_auth<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        authorization: &str,
    ) -> CoreResult<T> {
        let req = self
            .inner
            .post(url)
            .header(AUTHORIZATION, authorization)
            .json(&serde_json::json!({}));
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
