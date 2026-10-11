//! 美式英语文案表
//!
//! 实现 [`I18Lang`](crate::i18::I18Lang)，把各条目类型映射为英文文案。

use crate::{
    VERSION,
    i18::I18Lang,
    i18_items::{
        error_type::{ArgEmptyData, ArgErrorData, DataNotFoundData, ErrorType, SkinBlockErrorData},
        gui_type::GuiType,
        info_type::InfoType,
        panic_type::PanicType,
        thread_type::ThreadType,
    },
};
/// 美式英语语言包
pub struct EnUs;

impl I18Lang for EnUs {
    /// 取英文一般信息文案
    ///
    /// - `info`: 信息条目
    ///
    /// # 返回值
    ///
    /// 英文成品字符串
    fn get_info(&self, info: &InfoType) -> String {
        match info {
            InfoType::CoreStart => format!("M²L started, version: {}", *VERSION),
            InfoType::CoreStop => String::from("M²L stopped"),
            InfoType::TempFile => String::from("Temporary files"),
        }
    }

    /// 取英文错误文案
    ///
    /// - `error`: 错误条目，部分条目自带运行时数据
    ///
    /// # 返回值
    ///
    /// 英文成品字符串
    fn get_error(&self, error: &ErrorType) -> String {
        match error {
            ErrorType::Panic(panic) => self.get_panic(panic),

            ErrorType::ConfigError(data) => {
                format!(
                    "Failed to process config file {}: {}",
                    data.path.display().to_string(),
                    data.error
                )
            }
            ErrorType::HttpError(data) => match data.status {
                Some(status) => format!(
                    "HTTP request {} failed: {} (status {})",
                    data.url, data.error, status
                ),
                None => format!("HTTP request {} failed: {}", data.url, data.error),
            },

            ErrorType::SerializerError(data) => format!("JSON parse failed: {}", data.error),
            ErrorType::PathNotExists(data) => {
                format!("Path not found: {}", data.path.display().to_string())
            }

            ErrorType::AuthFail(data) => format!("Account operation failed: {}", data),
            ErrorType::AuthNoProfile => String::from("Account error, no profile found"),
            ErrorType::AuthTokenTimeout => {
                String::from("Account token expired, please sign in again")
            }
            ErrorType::AuthServerNull => {
                String::from("Account is missing its server address, please re-add it")
            }
            ErrorType::OAuthGetTokenError(data) => {
                format!("OAuth token request failed: {}", data.error)
            }
            ErrorType::OAuthGetTokenEmpty => String::from("OAuth did not return a token"),

            ErrorType::FileSystemError(data) => {
                format!(
                    "Failed to process file {}: {}",
                    data.path.display().to_string(),
                    data.error
                )
            }
            ErrorType::FileReadError(data) => format!("Failed to read file: {}", data.error),

            ErrorType::ArchiveOpenError(data) => {
                format!(
                    "Failed to open archive {}: {}",
                    data.path.display().to_string(),
                    data.error
                )
            }
            ErrorType::ArchiveReadError(data) => format!("Failed to read archive: {}", data.error),
            ErrorType::ArchiveError(data) => {
                format!(
                    "Archive processing failed: {} → {}: {}",
                    data.source, data.target, data.error
                )
            }
            ErrorType::ArchiveWriteError(data) => {
                format!("Failed to write archive: {}", data.error)
            }

            ErrorType::TaskCancel => String::from("Task cancelled"),
            ErrorType::TaskTimeout => String::from("Task timed out"),
            ErrorType::TaskError(data) => format!("Task error: {}", data.error),

            ErrorType::NbtTypeError => String::from("NBT type error"),
            ErrorType::NbtReadError => String::from("Failed to read NBT"),

            ErrorType::ArgEmpty(data) => match data {
                ArgEmptyData::Name => String::from("Name argument is empty"),
                ArgEmptyData::UUID => String::from("UUID argument is empty"),
                ArgEmptyData::Version => String::from("Version argument is empty"),
            },
            ErrorType::ArgError(data) => match data {
                ArgErrorData::ArchiveType => String::from("Invalid archive type argument"),
            },
            ErrorType::DataNotFound(data) => match data {
                DataNotFoundData::Info => String::from("Requested information not found"),
                DataNotFoundData::RegistryKey(key) => format!("Registry key not found: {key}"),
                DataNotFoundData::Url => String::from("Requested URL not found"),
                DataNotFoundData::GameInstance => String::from("Game instance not found"),
                DataNotFoundData::Version(ver) => format!("Game version not found: {ver}"),
            },
            ErrorType::JavaNotFound => String::from("No suitable Java found"),
            ErrorType::GpuNotAvailable => {
                String::from("No available GPU backend for icon rendering")
            }
            ErrorType::InstanceNameExists(name) => format!("Instance name {name} already exists"),

            ErrorType::DownloadFileOverFail(data) => {
                format!(
                    "Failed to overwrite file {}: {}",
                    data.file.display().to_string(),
                    data.error
                )
            }
            ErrorType::DownloadFileSizeError(data) => {
                format!(
                    "File {} size mismatch (expected {}, got {})",
                    data.file.display().to_string(),
                    data.size,
                    data.now
                )
            }
            ErrorType::DownloadFileHashError(data) => {
                format!(
                    "File {} hash mismatch (expected {}, got {})",
                    data.file.display().to_string(),
                    data.hash,
                    data.now
                )
            }
            ErrorType::DownloadFileFail => String::from("File download failed"),

            ErrorType::InvalidOperation => String::from("Invalid operation"),

            ErrorType::SocketError(data) => format!("Socket error: {}", data.error),
            ErrorType::ThreadError(data) => format!("Failed to start thread: {}", data.error),
            ErrorType::ProcessError(data) => format!("Failed to start process: {}", data.error),
            ErrorType::InstanceVersionError => String::from("Invalid version number"),
            ErrorType::Base64Error(data) => format!("Base64 processing failed: {}", data.error),
            ErrorType::StreamError(data) => format!("Stream processing error: {}", data.error),

            ErrorType::KeyIsNull => String::from("Key is not set"),

            ErrorType::SkinBlockError(data) => match data {
                SkinBlockErrorData::NameIllegal(name) => {
                    format!(
                        "Invalid skin block name: {name} (only ASCII letters, digits, '-' and '_', max 64 characters)"
                    )
                }
                SkinBlockErrorData::SkinSize { width, height } => {
                    format!("Skin must be 64x64 (or legacy 64x32), got {width}x{height}")
                }
                SkinBlockErrorData::PlayerNotFound => {
                    String::from("Player not found, or the player has no skin")
                }
                SkinBlockErrorData::DecodeFail => String::from("Failed to decode skin PNG"),
                SkinBlockErrorData::RenderFail => String::from("Failed to render skin block"),
                SkinBlockErrorData::NotFound(id) => format!("Skin block not found: {id}"),
            },
        }
    }

    /// 取英文严重错误文案
    ///
    /// - `panic`: 严重错误条目
    ///
    /// # 返回值
    ///
    /// 英文成品字符串
    fn get_panic(&self, panic: &PanicType) -> String {
        match panic {
            PanicType::CoreArgLocalEmpty => String::from("Run path is empty"),
            PanicType::CoreArgLocalError => String::from("Run path does not exist"),
            PanicType::LogOpenFail(data, data1) => {
                format!("Log system initialization failed: {} path: {}", data1, data)
            }
        }
    }

    /// 取英文线程名
    ///
    /// - `thread`: 线程条目
    ///
    /// # 返回值
    ///
    /// 英文线程名
    fn get_thread(&self, thread: &ThreadType) -> String {
        match thread {
            ThreadType::LogThread => String::from("Log thread"),
            ThreadType::ConfigSaveThread => String::from("Config save thread"),
            ThreadType::LanClientV4 => String::from("LAN game V4 listener thread"),
            ThreadType::LanClientV6 => String::from("LAN game V6 listener thread"),
            ThreadType::LanServer => String::from("LAN game broadcast thread"),
            ThreadType::GameCount => String::from("Game launch stats thread"),
        }
    }

    /// 取英文界面文案
    ///
    /// - `gui`: 界面文案条目
    ///
    /// # 返回值
    ///
    /// 界面文案尚未接入，恒为空串
    fn get_gui(&self, gui: &GuiType) -> String {
        match gui {
            _ => String::new(),
        }
    }
}
