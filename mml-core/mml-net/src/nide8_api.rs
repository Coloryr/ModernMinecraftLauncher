//! 统一通行证 Nide8 API
//!
//! 提供从 Nide8 服务器获取认证 JAR 版本信息和哈希值的功能。
//! 用于在游戏启动时注入 Nide8 认证模块。

use mml_names::i18_items::error_type::CoreResult;
use serde::{Deserialize, Serialize};

use crate::urls;

/// Nide8 认证 JAR 信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Nide8Obj {
    /// JAR 版本号，对应响应键 `jarVersion`
    #[serde(rename = "jarVersion")]
    pub jar_version: String,
    /// JAR 文件哈希，对应响应键 `jarHash`
    #[serde(rename = "jarHash")]
    pub jar_hash: String,
}

impl Default for Nide8Obj {
    /// 全字段取默认值，配合 `#[serde(default)]` 兜底缺失字段
    fn default() -> Self {
        Self {
            jar_version: Default::default(),
            jar_hash: Default::default(),
        }
    }
}

/// 获取 Nide8 认证 JAR 的最新版本信息和哈希
///
/// 用于下载并在游戏启动时注入 Nide8 认证模块。
///
/// # 返回值
///
/// 返回 JAR 版本号与哈希
pub async fn get_obj() -> CoreResult<Nide8Obj> {
    crate::get_work_client()
        .get_json::<Nide8Obj>(&format!(
            "{}00000000000000000000000000000000/",
            urls::NIDE8_URL
        ))
        .await
}
