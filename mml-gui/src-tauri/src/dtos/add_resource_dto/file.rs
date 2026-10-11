//! 文件 / 版本列表 DTO：某个项目下的文件条目
//!
//! 从 `add_resource_dto/mod.rs` 拆出来的。`new_curseforge` / `new_modrinth` 把两个源的
//! 文件数据统一成同一份前端结构，没有可下文件的版本返回 `None`。

use serde::{Deserialize, Serialize};

use mml_game::launcher::{FileType, ModPackType};
use mml_net::curseforge_api::file_obj::CurseForgeFileDataObj;
use mml_net::modrinth_api::version_obj::ModrinthVersionObj;

use super::common::SourceTypeDto;

/// 项目文件列表，即项目详情内的版本 / 文件分页
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListDto {
    /// 当前页条目
    pub list: Vec<FileListItemDto>,
    /// 该项目的文件总数，分页用，不受当前页影响
    pub count: u64,
    /// 项目名字，即源站的项目标题
    pub name: String,
    /// 最大页，按每页 50 条算
    pub max_page: u64,
}

/// 项目文件条目，即某个可下载的版本 / 文件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListItemDto {
    /// 文件或版本的显示名
    pub name: String,
    /// 下载次数
    pub download: u64,
    /// 文件大小，单位字节
    pub size: u64,
    /// 发布时间，源站返回的字符串，未做格式化
    pub time: String,
    /// 是否已经下载
    pub is_download: bool,
    /// 是否正在下载
    pub download_now: bool,
    /// 下载源信息
    pub source: SourceTypeDto,
}

impl FileListItemDto {
    /// 由 CurseForge 文件数据构造
    ///
    /// - `data`: CurseForge 文件数据
    /// - `file_type`: 资源类型
    /// - `is_download`: 是否已经下载
    /// - `download_now`: 是否正在下载
    ///
    /// # 返回值
    ///
    /// 返回列表条目
    pub fn new_curseforge(
        data: &CurseForgeFileDataObj,
        file_type: FileType,
        is_download: bool,
        download_now: bool,
    ) -> Self {
        Self {
            name: data.display_name.clone(),
            download: data.download_count,
            size: data.file_length,
            is_download,
            time: data.file_date.clone(),
            download_now,
            source: SourceTypeDto {
                file_type: file_type.to_string(),
                source: ModPackType::CurseForge.to_string(),
                pid: data.mod_id.to_string(),
                fid: data.id.to_string(),
            },
        }
    }

    /// 由 Modrinth 版本数据构造
    ///
    /// - `data`: Modrinth 版本数据
    /// - `file_type`: 资源类型
    /// - `is_download`: 是否已经下载
    /// - `download_now`: 是否正在下载
    ///
    /// # 返回值
    ///
    /// 返回列表条目，文件取 primary，没有 primary 取第一个
    pub fn new_modrinth(
        data: &ModrinthVersionObj,
        file_type: FileType,
        is_download: bool,
        download_now: bool,
    ) -> Option<Self> {
        // 取 primary 文件，没有 primary 退回第一个；一个文件都没有时返回 None
        // 原来是 first().unwrap()，Modrinth 返回空 files 会 panic，这种版本本来也没东西可下
        let file = data
            .files
            .iter()
            .find(|item| item.primary)
            .or_else(|| data.files.first())?;
        Some(Self {
            name: data.name.clone(),
            download: data.downloads,
            size: file.size,
            time: data.date_published.clone(),
            is_download,
            download_now,
            source: SourceTypeDto {
                file_type: file_type.to_string(),
                source: ModPackType::Modrinth.to_string(),
                // pid 是项目、fid 是文件即版本号，与 CurseForge 侧一致；
                // 反了会让 add_modpack_install 拿版本号当项目号去查
                pid: data.project_id.clone(),
                fid: data.id.clone(),
            },
        })
    }
}
