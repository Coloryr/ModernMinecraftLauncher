use std::sync::OnceLock;

use mml_names::i18_items::error_type::CoreResult;
use serde::{Deserialize, Serialize};

use crate::urls;

/// Zulu 包列表接口返回的单个 Java 包
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ZuluObj {
    /// 目标架构
    pub arch: String,
    /// 包类型，取值 jdk / jre
    pub bundle_type: String,
    /// 位宽，取值 32 / 64
    pub hw_bitness: String,
    /// Java 版本号分段，如 [21, 0, 5]
    pub java_version: Vec<i32>,
    /// 包显示名称
    pub name: String,
    /// 目标操作系统
    pub os: String,
    /// 安装包 SHA-256 校验值
    pub sha256_hash: String,
    /// 安装包下载直链
    pub url: String,
    /// Zulu 自己的版本号分段
    pub zulu_version: Vec<i32>,
}

/// 全部 Java 包缓存，首次查询后复用
static ZULU: OnceLock<Vec<ZuluObj>> = OnceLock::new();

/// 拉取 Zulu 包列表并写入 `ZULU` 缓存
async fn init() -> CoreResult<Vec<ZuluObj>> {
    let list: Vec<ZuluObj> = crate::get_work_client().get_json(urls::ZULU).await?;
    Ok(ZULU.get_or_init(|| list).clone())
}

/// 获取 Zulu 的 Java 包列表
///
/// # 返回值
///
/// 首次调用拉取并缓存，之后直接返回缓存
pub async fn get_java_list() -> CoreResult<Vec<ZuluObj>> {
    match ZULU.get() {
        Some(data) => Ok(data.clone()),
        None => init().await,
    }
}
