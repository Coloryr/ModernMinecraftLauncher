//! 资源添加窗口 DTO：CurseForge / Modrinth 的项目列表、详情、文件列表与下载任务
//!
//! 从单个 565 行的文件按用途拆成几个模块；类型经 `pub use` 再导出，
//! 所以 `crate::dtos::add_resource_dto::XxxDto` 这些既有路径都没变。

mod common;
mod dependency;
mod detail;
mod file;
mod project;
mod status;

pub use self::common::{DecPicDto, McmodDto, PicDto, SourceTypeDto, TagDto};
pub use self::dependency::{ResourceDependencyDto, ResourceDependencyFileDto};
pub use self::detail::ProjectDetailDto;
pub use self::file::{FileListDto, FileListItemDto};
pub use self::project::{ProjectDto, ProjectItemDto};
pub use self::status::{ResourceSaveDto, ResourceStatusDto, ResourceTaskDto};
