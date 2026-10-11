//! Fabric 模组加载器 API
//!
//! 提供从 Fabric Meta API 获取加载器版本信息和下载配置的功能。

use mml_names::i18_items::error_type::CoreResult;

use crate::url_helper;

/// 获取 Fabric 加载器安装配置，即 profile JSON
///
/// - `mc`: Minecraft 游戏版本
/// - `version`: Fabric Loader 版本
///
/// # 返回值
///
/// 返回 profile JSON 原文字节，可直接作为版本 JSON 的内容合并
pub async fn get_loader(mc: &str, version: &str) -> CoreResult<Vec<u8>> {
    let url = format!(
        "{}/loader/{mc}/{version}/profile/json",
        url_helper::get_fabric_meta()
    );

    crate::get_work_client().get_bytes(&url).await
}

/// 获取 Fabric 元数据，即可用版本列表
///
/// # 返回值
///
/// 返回元数据 JSON 原文字节
pub async fn get_meta() -> CoreResult<Vec<u8>> {
    let url = url_helper::get_fabric_meta();

    crate::get_work_client().get_bytes(&url).await
}
