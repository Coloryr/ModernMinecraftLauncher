//! mclo.gs 日志上传
//!
//! 把日志文本 POST 到 mclo.gs，换回一个可分享的网页地址，日志窗口的"上传日志"用它。
//! 上传是**用户主动触发**的，内容会离开本机，界面上要先问一句再传。
//!
//! 接口：`POST https://api.mclo.gs/1/log`，表单字段 `content`；成功返回
//! `{"success":true,"url":"https://mclo.gs/xxxx"}`，失败返回 `{"success":false,"error":"..."}`，
//! 其中"日志太大"这类业务失败也是 **200**，所以成败要看响应体里的 `success`，不能只看状态码。

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use serde::Deserialize;

use crate::urls;

/// 上传响应体
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct McloPushObj {
    /// 是否上传成功
    pub success: bool,
    /// 成功时的分享地址
    pub url: String,
    /// 失败原因，成功时为空
    pub error: String,
}

/// 上传日志文本
///
/// - `content`: 日志全文；空串会被对方拒绝，调用方先判
///
/// # 返回值
///
/// 成功返回可分享的网址；网络失败或对方判为失败时返回相应错误
pub async fn push(content: String) -> CoreResult<String> {
    let client = crate::get_work_client();
    let obj: McloPushObj = client
        .post_form_get_json(
            urls::MCLO_PUSH,
            &[crate::FormField {
                key: "content",
                value: &content,
            }],
        )
        .await?;

    if !obj.success || obj.url.is_empty() {
        // 业务失败：对方给的原因只进日志，界面按通用的"上传失败"报
        mml_log::warn(ErrorType::McloUploadRejected(obj.error.clone()).to_string());
        return Err(ErrorType::InvalidOperation);
    }

    Ok(obj.url)
}
