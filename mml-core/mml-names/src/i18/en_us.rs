//! 美式英语文案表
//!
//! 实现 [`I18Lang`](crate::i18::I18Lang)，把各条目类型映射为英文文案。

use crate::{
    VERSION,
    i18::I18Lang,
    i18_items::{
        error_type::{
            ArgEmptyData, ArgErrorData, DataNotFoundData, ErrorType, NameInvalidData,
            NameInvalidReason, SkinBlockErrorData,
        },
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
            InfoType::AdoptMmcInstance { name } => format!("adopted MMC instance: {name}"),
            InfoType::ImportLegacyGroups { groups, instances } => format!(
                "imported legacy group file instgroups.json: {groups} new groups, {instances} instances"
            ),
            InfoType::DownloadTaskDone {
                total,
                completed,
                failed,
            } => {
                format!("download task finished: {total} total, {completed} done, {failed} failed")
            }
            InfoType::ProxyUser {
                kind,
                ip,
                port,
                auth,
            } => format!(
                "User/{kind} {ip}:{port} (auth={})",
                if *auth { "yes" } else { "no" }
            ),
            InfoType::ProxyNone => String::from("None (proxy disabled)"),
            InfoType::ProxyAuto => String::from("Auto (follow system)"),
            InfoType::HttpClientInit { work, login } => {
                format!("HTTP client initialized: work[{work}] login[{login}]")
            }
            InfoType::HttpClientRestart { generation } => format!(
                "HTTP client restarting, in-flight requests aborted (generation -> {generation})"
            ),
            InfoType::HttpClientRestartDone { work, login } => {
                format!("HTTP client restarted: work[{work}] login[{login}]")
            }
            InfoType::IpcNotifyExisting { port } => {
                format!("IPC: notified the running instance, port {port}")
            }
            InfoType::IpcNotifyExistingFailed { port } => {
                format!("IPC: notify failed on port {port}, starting a new server")
            }
            InfoType::CloudServerStart { address } => {
                format!("Save cloud sync server listening on {address}")
            }
            InfoType::IpcServerStart { port } => {
                format!("IPC: server started, listening on 127.0.0.1:{port}")
            }
            InfoType::IpcClientConnected { addr } => format!("IPC: client connected {addr}"),
            InfoType::IpcUnknownMessageType { msg_type } => {
                format!("IPC: unknown message type {msg_type}")
            }
            InfoType::IpcLauncherShowWindow => {
                String::from("IPC: the running instance asked to show the main window")
            }
            InfoType::IpcGameChannelBound { guid } => format!("IPC: game channel bound to {guid}"),
            InfoType::GamepadConnected { name } => format!("[gamepad] gamepad connected: {name}"),
            InfoType::GamepadDisconnected { name } => {
                format!("[gamepad] gamepad disconnected: {name}")
            }
            InfoType::GamepadInitFailed { error } => {
                format!("[gamepad] gamepad reader initialization failed: {error}")
            }
            InfoType::GamepadMapUnavailable { path } => {
                format!("[gamepad] no usable gamepad map in {path}")
            }
            InfoType::GamepadMapLoadFailed { path, error } => {
                format!("[gamepad] gamepad map unavailable: {path}: {error}")
            }
            InfoType::GamepadMapDefaultWritten { path } => {
                format!("[gamepad] wrote the default gamepad map: {path}")
            }
            InfoType::GamepadMapLoaded { path } => {
                format!("[gamepad] gamepad map loaded: {path}")
            }
            InfoType::GamepadForwardStart { uuid } => {
                format!("[gamepad] forwarding gamepad input to instance {uuid}")
            }
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
            ErrorType::HttpError(data) => {
                // 构建阶段就失败时没有 URL，给个明确说法（与中文表同一处口径）
                let url = if data.url.is_empty() {
                    "(no URL)"
                } else {
                    data.url.as_str()
                };
                match data.status {
                    Some(status) => format!(
                        "HTTP request {} failed: {} (status {})",
                        url, data.error, status
                    ),
                    None => format!("HTTP request {} failed: {}", url, data.error),
                }
            }

            ErrorType::Aborted { url } => {
                // 与中文表同一口径：拿不到地址时给个明确说法
                let url = if url.is_empty() {
                    "(no URL)"
                } else {
                    url.as_str()
                };
                format!("HTTP request {url} aborted: proxy or network settings changed")
            }

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
            ErrorType::UnsafePath { root, path } => format!(
                "Rejected unsafe path {} within {}",
                path.display(),
                root.display()
            ),
            ErrorType::MoveToTrashError(data) => format!(
                "Failed to move {} to trash, code 0x{:X}",
                data.path.display().to_string(),
                data.code
            ),
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
            ErrorType::DownloadRangeError(url) => format!("Invalid download byte range: {url}"),
            ErrorType::NbtStringTooLong(size) => {
                format!("NBT string is {size} bytes; maximum is 65535")
            }

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
            ErrorType::NameInvalid(NameInvalidData { name, reason }) => match reason {
                NameInvalidReason::Empty => String::from("Name cannot be empty"),
                NameInvalidReason::IllegalChar(c) => {
                    format!(
                        "Name \"{name}\" cannot contain the character {}",
                        c.escape_default()
                    )
                }
                NameInvalidReason::TooLong { len, max } => {
                    format!("Name \"{name}\" is too long: {len} characters, at most {max}")
                }
                NameInvalidReason::TrailingDot => {
                    format!("Name \"{name}\" cannot end with a dot")
                }
                NameInvalidReason::Reserved => {
                    format!("Name \"{name}\" is reserved by the system; pick another one")
                }
            },

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
            ErrorType::DownloadItemFail(data) => format!(
                "file {} failed {} time(s): {} (url {})",
                data.name,
                data.times,
                data.error,
                if data.url.is_empty() {
                    "missing"
                } else {
                    data.url.as_str()
                }
            ),
            ErrorType::DownloadFileFail => String::from("File download failed"),

            ErrorType::InvalidOperation => String::from("Invalid operation"),

            ErrorType::SocketError(data) => format!("Socket error: {}", data.error),
            ErrorType::MulticastAddrMissing => {
                String::from("The multicast send address is not set")
            }
            ErrorType::MulticastSendFailed(data) => {
                format!("Failed to send {} multicast: {}", data.family, data.error)
            }
            ErrorType::ThreadError(data) => format!("Failed to start thread: {}", data.error),
            ErrorType::ProcessError(data) => format!("Failed to start process: {}", data.error),
            ErrorType::ProcessElevationUnsupported => {
                String::from("Admin elevation is not supported on this platform")
            }
            ErrorType::NamedPipeCreateFailed(data) => {
                format!("Failed to create the named pipe: {}", data.error)
            }
            ErrorType::NamedPipeEventCreateFailed(data) => {
                format!("Failed to create the named pipe event: {}", data.error)
            }
            ErrorType::NamedPipeConnectTimeout => {
                String::from("Named pipe connection timed out; UAC may have been declined")
            }
            ErrorType::NamedPipeWaitFailed(data) => {
                format!(
                    "Failed while waiting for the named pipe connection: {}",
                    data.error
                )
            }
            ErrorType::NamedPipeConnectFailed(data) => {
                format!("Failed to connect the named pipe: {}", data.error)
            }
            ErrorType::InstanceVersionError => String::from("Invalid version number"),
            ErrorType::Base64Error(data) => format!("Base64 processing failed: {}", data.error),
            ErrorType::StreamError(data) => format!("Stream processing error: {}", data.error),
            ErrorType::MotdVarintTooBig => String::from("MOTD varint exceeds the length limit"),

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
            ErrorType::DownloadThreadPanic(data) => {
                format!("Download thread panicked: {}", data.error)
            }
            ErrorType::PanicPayloadUnknown => String::from("unknown panic"),
            ErrorType::ModrinthAvatarFailed(data) => {
                format!("Failed to query Modrinth author avatars: {}", data.error)
            }
            ErrorType::McloUploadRejected(reason) => {
                format!("mclo.gs rejected the upload: {reason}")
            }
            ErrorType::MotdConnectTimeout { ip, port } => {
                format!("Connection timed out: {ip}:{port}")
            }
            ErrorType::MotdConnectFailed { ip, port, error } => {
                format!("Connection failed: {ip}:{port} {error}")
            }
            ErrorType::MotdBadResponseLen => String::from("Unexpected response length"),
            ErrorType::MotdBadPacketId { packet_id } => {
                format!("Unexpected response packet type: {packet_id}")
            }
            ErrorType::MotdResponseIncomplete => String::from("Incomplete response data"),
            ErrorType::ApngEncodeFail { stage, error } => {
                format!("Failed to encode the block animation icon as APNG: {stage} {error}")
            }
            ErrorType::IpcRuntimeCreateError(data) => {
                format!("IPC: failed to create the async runtime: {}", data.error)
            }
            ErrorType::CloudServerFailed(data) => {
                format!("Save cloud sync server failed: {}", data.error)
            }
            ErrorType::IpcBindError(data) => {
                format!("IPC: failed to bind the port: {}", data.error)
            }
            ErrorType::IpcAcceptError(data) => {
                format!("IPC: failed to accept a connection: {}", data.error)
            }
            ErrorType::IpcWriteError(data) => {
                format!("IPC: failed to send data: {}", data.error)
            }
            ErrorType::IpcInvalidFrame => "IPC: invalid frame length".to_string(),
            ErrorType::IpcFrameTooLarge { len } => {
                format!("IPC: frame too large, {len} bytes")
            }
            ErrorType::IpcBufferOverrun => {
                String::from("IPC: receive buffer overrun, dropping the connection")
            }
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
}
