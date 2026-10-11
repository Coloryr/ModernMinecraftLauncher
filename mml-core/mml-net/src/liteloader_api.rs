//! LiteLoader 模组加载器 API
//!
//! LiteLoader 是一个轻量级的 Minecraft 模组加载器，与 Forge 共存。

use mml_names::i18_items::error_type::CoreResult;

use crate::urls;

/// 获取 LiteLoader 版本元数据，即可用版本列表
///
/// # 返回值
///
/// 返回元数据 JSON 原文字节
pub async fn get_meta() -> CoreResult<Vec<u8>> {
    let url = format!("{}versions/versions.json", urls::LITELOADER);

    crate::get_work_client().get_bytes(&url).await
}
