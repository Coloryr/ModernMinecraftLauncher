//! 游戏版本 DTO
use serde::{Deserialize, Serialize};

/// 游戏版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfoDto {
    /// 版本号
    pub id: String,
    /// 版本类型，如 release / snapshot / old_beta 等
    pub version_type: String,
}
