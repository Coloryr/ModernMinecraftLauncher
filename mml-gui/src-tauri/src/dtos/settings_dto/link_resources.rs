//! 设置窗口「实验性功能」里已下载资源列表的 DTO

use serde::{Deserialize, Serialize};

/// 公共目录里的一份已下载资源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkResourceDto {
    /// 资源类型：`mod` / `resourcepack`
    pub kind: String,
    /// 项目 ID
    pub pid: String,
    /// 显示名
    pub name: String,
    /// 公共目录下的文件名
    pub file: String,
    /// 文件 SHA1，小写
    pub hash: String,
    /// 文件大小，单位字节；文件不在时为 0
    pub size: u64,
    /// 文件是否还在公共目录里
    pub exists: bool,
}
