//! 实例来源项目信息，保存于实例目录

use serde::{Deserialize, Serialize};

use crate::launcher::ModPackType;

/// 实例来源项目信息，记录整合包来源，用于更新等场景
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MmlProjectSaveObj {
    /// 整合包来源类型
    #[serde(rename = "type")]
    pub source_type: ModPackType,
    /// 项目 ID
    pub pid: String,
    /// 文件 ID
    pub fid: String,
}

impl Default for MmlProjectSaveObj {
    fn default() -> Self {
        Self {
            source_type: Default::default(),
            pid: Default::default(),
            fid: Default::default(),
        }
    }
}
