//! 传输对象 DTO —— 前端 IPC wire 形态，使用 TS 命名 camelCase。
//!
//! 配置 / 持久化文件如 gui_config.json、main_data.json 等用 Rust 命名，
//! 仅当跨 Tauri IPC 传输到前端时才转成 DTO，或 DTO 本身即 wire 形态，
//! 如事件负载 / 命令入参。纯传输、无业务方法的窗口类型也集中在此。

pub mod account_dto;
pub mod add_dto;
pub mod add_modpack_dto;
pub mod add_resource_dto;
pub mod args_dto;
pub mod collect_dto;
pub mod colormc_dto;
pub mod config_dto;
pub mod custom_home_dto;
pub mod download_dto;
pub mod export_dto;
pub mod gui_config_dto;
pub mod instance_dto;
pub mod java_download_dto;
pub mod java_dto;
pub mod log_dto;
pub mod main_dto;
pub mod motd_dto;
pub mod resource_dto;
pub mod settings_dto;
pub mod skin_dto;
pub mod stats_dto;
pub mod version_dto;
pub mod window_dto;

pub use account_dto::{AccountStoreDto, AccountStoreViewDto};
pub use add_dto::{
    DetectedPackDto, DirEntry, FolderInstanceDto, LoaderProgressDto, NameConflictDto,
    PackProgressDto,
};
pub use add_modpack_dto::{ModPackStatusDto, ModPackTaskDto};
pub use add_resource_dto::{
    DecPicDto, FileListDto, FileListItemDto, McmodDto, PicDto, ProjectDetailDto, ProjectDto,
    ProjectItemDto, ResourceSaveDto, ResourceStatusDto, ResourceTaskDto, SourceTypeDto, TagDto,
};
pub use args_dto::{EnvVarLineDto, InstanceArgsDto};
pub use collect_dto::{CollectDataDto, CollectItemDto};
pub use colormc_dto::{ColorMcCompatDto, ColorMcInfoDto, ColorMcProgressDto, ColorMcReportDto};
pub use config_dto::{
    BytesSliceDto, ConfigHunkDto, ConfigNbtDto, ConfigTextDto, FileItemDto, FileItemType,
    McaChunkDto, NbtEditDto, NbtKindDto, NbtNodeDto, NbtOpDto,
};
pub use custom_home_dto::CustomHomeInfoDto;
pub use download_dto::{
    DownloadItemEvent, DownloadStatusDto, DownloadTaskDto, DownloadTaskEvent, DownloadThreadDto,
};
pub use export_dto::{ExportConfigDto, ExportEntryDto, ExportInfoDto, ExportProgressDto};
pub use gui_config_dto::{ClientConfigDto, GuiConfigDto, LoginLockItemDto, MainWindowConfigDto};
pub use instance_dto::{GroupDto, InstanceInfoDto};
pub use java_download_dto::{JavaDownloadItemDto, JavaDownloadOptionsDto, JavaTypes};
pub use java_dto::{JavaImportProgressDto, JavaInfoDto};
pub use log_dto::{LogFocusDto, LogSettingDto};
pub use main_dto::{
    BlockItemDto, BlockStatusDto, ErrorEvent, ExitEvent, IconSourceDto, InstanceChangeEvent,
    InstanceLangDto, InstancePatch, LogEvent, NewsItem, StateEvent, SystemMemoryDto,
};
pub use motd_dto::{MotdDto, MotdSegmentDto};
pub use resource_dto::{
    DataPackItemDto, DepProblemDto, DepProblemKindDto, ModGroupDto, ModItemDto, ModRenameDto,
    ModScanProgressDto, PackItemDto, ResourceDropDto, ResourceViewDto, SaveBackupDto, SaveEditDto,
    SaveItemDto, SchematicBlockDto, SchematicDetailDto, SchematicItemDto, ScreenshotItemDto,
    ServerItemDto, ShaderItemDto,
};
pub use settings_dto::{
    BgInfoDto, DnsSettingDto, GameCheckSettingDto, LaunchSettingDto, NetworkSettingDto,
    RunArgSettingDto, SettingsDefaultsDto, WindowSettingDto,
};
pub use stats_dto::{StatsDataDto, StatsInstanceDto};
pub use version_dto::VersionInfoDto;
pub use window_dto::{DropPathDto, FileDropDto, WindowSizeDto};
