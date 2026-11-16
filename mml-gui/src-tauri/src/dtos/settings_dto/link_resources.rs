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

/// 已下载资源列表的查询条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkResourceQueryDto {
    /// 搜索词，匹配项目 ID、显示名、文件名与 SHA1；空串表示不筛
    pub keyword: String,
    /// 页码，从 1 开始；超出末页时回落到末页
    pub page: u32,
    /// 每页条数，单位条；0 按默认值处理
    pub page_size: u32,
}

/// 已下载资源列表的一页
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkResourcePageDto {
    /// 筛选后的总条数，单位条
    pub total: u64,
    /// 筛选后仍在磁盘上的总占用，单位字节
    pub total_size: u64,
    /// 当前页码，从 1 开始
    pub page: u32,
    /// 总页数，至少为 1
    pub pages: u32,
    /// 每页条数，单位条
    pub page_size: u32,
    /// 当前页的记录，按项目 ID 与类型分组后顺序排列
    pub items: Vec<LinkResourceDto>,
}
