//! 简体中文文案表
//!
//! 实现 [`I18Lang`](crate::i18::I18Lang)，把各条目类型映射为中文文案；
//! 也是语言包未设置时的默认实现。

use crate::VERSION;

use crate::{
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

/// 简体中文语言包
pub struct ZhCn;

impl I18Lang for ZhCn {
    /// 取中文一般信息文案
    ///
    /// - `info`: 信息条目
    ///
    /// # 返回值
    ///
    /// 中文成品字符串
    fn get_info(&self, info: &InfoType) -> String {
        match info {
            InfoType::CoreStart => format!("M²L启动，版本：{}", *VERSION),
            InfoType::CoreStop => format!("M²L停止"),
            InfoType::TempFile => format!("临时文件"),
            InfoType::AdoptMmcInstance { name } => format!("检测到 MMC 实例并已接管：{name}"),
            InfoType::ImportLegacyGroups { groups, instances } => format!(
                "已把旧版分组文件 instgroups.json 并入分组表：新建 {groups} 个分组，归入 {instances} 个实例"
            ),
            InfoType::DownloadTaskDone {
                total,
                completed,
                failed,
            } => format!("下载任务结束：共 {total} 个，成功 {completed}，失败 {failed}"),
            InfoType::ProxyUser {
                kind,
                ip,
                port,
                auth,
            } => format!(
                "User/{kind} {ip}:{port}（认证={}）",
                if *auth { "有" } else { "无" }
            ),
            InfoType::ProxyNone => String::from("None（禁用代理）"),
            InfoType::ProxyAuto => String::from("Auto（跟随系统）"),
            InfoType::HttpClientInit { work, login } => {
                format!("HTTP 客户端已初始化：通用[{work}] 登录[{login}]")
            }
            InfoType::HttpClientRestart { generation } => {
                format!("HTTP 客户端重启：已中断在途请求（世代 -> {generation}）")
            }
            InfoType::HttpClientRestartDone { work, login } => {
                format!("HTTP 客户端重启完成：通用[{work}] 登录[{login}]")
            }
            InfoType::IpcNotifyExisting { port } => {
                format!("IPC：已通知运行中的实例，端口 {port}")
            }
            InfoType::IpcNotifyExistingFailed { port } => {
                format!("IPC：通知运行中的实例失败，端口 {port}，改为启动新的服务")
            }
            InfoType::IpcServerStart { port } => {
                format!("IPC：服务已启动，监听 127.0.0.1:{port}")
            }
            InfoType::IpcClientConnected { addr } => format!("IPC：客户端已连接 {addr}"),
            InfoType::IpcUnknownMessageType { msg_type } => {
                format!("IPC：未知的消息类型 {msg_type}")
            }
            InfoType::IpcLauncherShowWindow => String::from("IPC：已运行实例请求显示主窗口"),
            InfoType::IpcGameChannelBound { guid } => format!("IPC：游戏通道已绑定 {guid}"),
        }
    }

    /// 取中文错误文案
    ///
    /// - `error`: 错误条目，部分条目自带运行时数据
    ///
    /// # 返回值
    ///
    /// 中文成品字符串
    fn get_error(&self, error: &ErrorType) -> String {
        match error {
            ErrorType::Panic(panic) => self.get_panic(panic),

            ErrorType::ConfigError(data) => {
                format!(
                    "配置文件 {} 处理失败：{}",
                    data.path.display().to_string(),
                    data.error
                )
            }
            ErrorType::HttpError(data) => {
                // 请求在构建阶段就失败时没有 URL（reqwest 的 error.url() 为空）：
                // 给个明确说法，日志里不要留一段空白
                let url = if data.url.is_empty() {
                    "（无地址）"
                } else {
                    data.url.as_str()
                };
                match data.status {
                    Some(status) => {
                        format!("网络请求 {} 失败：{}（状态码 {}）", url, data.error, status)
                    }
                    None => format!("网络请求 {} 失败：{}", url, data.error),
                }
            }

            ErrorType::Aborted { url } => {
                // 与 HttpError 同一口径：拿不到地址时给个明确说法，别留空白
                let url = if url.is_empty() {
                    "（无地址）"
                } else {
                    url.as_str()
                };
                format!("网络请求 {url} 被中断：代理或网络设置已更新")
            }

            ErrorType::SerializerError(data) => format!("Json解析失败：{}", data.error),
            ErrorType::PathNotExists(data) => {
                format!("路径不存在：{}", data.path.display().to_string())
            }

            ErrorType::AuthFail(data) => format!("账户操作失败：{}", data),
            ErrorType::AuthNoProfile => String::from("账户操作错误，没有找到账户"),
            ErrorType::AuthTokenTimeout => String::from("账户令牌已过期，请重新登录"),
            ErrorType::AuthServerNull => String::from("账户缺少服务器地址，请删除后重新添加"),
            ErrorType::OAuthGetTokenError(data) => format!("OAuth获取登录令牌失败：{}", data.error),
            ErrorType::OAuthGetTokenEmpty => String::from("OAuth没有获取到登录令牌"),

            ErrorType::FileSystemError(data) => {
                format!(
                    "文件 {} 处理失败：{}",
                    data.path.display().to_string(),
                    data.error
                )
            }
            ErrorType::UnsafePath { root, path } => format!(
                "拒绝不安全路径 {}，操作范围为 {}",
                path.display(),
                root.display()
            ),
            ErrorType::MoveToTrashError(data) => format!(
                "把 {} 移到回收站失败，返回码 0x{:X}",
                data.path.display().to_string(),
                data.code
            ),
            ErrorType::FileReadError(data) => format!("文件读取失败：{}", data.error),

            ErrorType::ArchiveOpenError(data) => {
                format!(
                    "压缩包 {} 打开失败：{}",
                    data.path.display().to_string(),
                    data.error
                )
            }
            ErrorType::ArchiveReadError(data) => format!("压缩包读取失败：{}", data.error),
            ErrorType::ArchiveError(data) => {
                format!(
                    "压缩包处理失败：{} → {}：{}",
                    data.source, data.target, data.error
                )
            }
            ErrorType::ArchiveWriteError(data) => format!("压缩包写入失败：{}", data.error),

            ErrorType::TaskCancel => String::from("任务已取消"),
            ErrorType::TaskTimeout => String::from("任务执行超时"),
            ErrorType::TaskError(data) => format!("任务出错：{}", data.error),

            ErrorType::NbtTypeError => String::from("NBT类型错误"),
            ErrorType::NbtReadError => String::from("NBT读取失败"),
            ErrorType::DownloadRangeError(url) => format!("下载响应的字节范围不匹配：{url}"),
            ErrorType::NbtStringTooLong(size) => {
                format!("NBT 字符串为 {size} 字节，最多允许 65535 字节")
            }

            ErrorType::ArgEmpty(data) => match data {
                ArgEmptyData::Name => String::from("名字参数为空"),
                ArgEmptyData::UUID => String::from("UUID参数为空"),
                ArgEmptyData::Version => String::from("版本参数为空"),
            },
            ErrorType::ArgError(data) => match data {
                ArgErrorData::ArchiveType => String::from("压缩包类型参数错误"),
            },
            ErrorType::DataNotFound(data) => match data {
                DataNotFoundData::Info => String::from("找不到请求的信息"),
                DataNotFoundData::RegistryKey(key) => format!("注册表键不存在：{key}"),
                DataNotFoundData::Url => String::from("找不到请求的网址"),
                DataNotFoundData::GameInstance => String::from("找不到游戏实例"),
                DataNotFoundData::Version(ver) => format!("找不到游戏版本：{ver}"),
            },
            ErrorType::JavaNotFound => String::from("找不到合适的Java"),
            ErrorType::GpuNotAvailable => String::from("没有可用的 GPU 后端，无法渲染图标"),
            ErrorType::InstanceNameExists(name) => format!("实例名字 {name} 已存在"),
            ErrorType::NameInvalid(NameInvalidData { name, reason }) => match reason {
                NameInvalidReason::Empty => String::from("名字不能为空"),
                NameInvalidReason::IllegalChar(c) => {
                    format!("名字「{name}」不能包含字符 {}", c.escape_default())
                }
                NameInvalidReason::TooLong { len, max } => {
                    format!("名字「{name}」太长：{len} 个字符，最多 {max} 个")
                }
                NameInvalidReason::TrailingDot => format!("名字「{name}」不能以点结尾"),
                NameInvalidReason::Reserved => format!("名字「{name}」是系统保留名，换一个吧"),
            },

            ErrorType::DownloadFileOverFail(data) => {
                format!(
                    "文件 {} 覆盖失败：{}",
                    data.file.display().to_string(),
                    data.error
                )
            }
            ErrorType::DownloadFileSizeError(data) => {
                format!(
                    "文件 {} 下载大小不符（预期 {}，实际 {}）",
                    data.file.display().to_string(),
                    data.size,
                    data.now
                )
            }
            ErrorType::DownloadFileHashError(data) => {
                format!(
                    "文件 {} 校验失败（预期 {}，实际 {}）",
                    data.file.display().to_string(),
                    data.hash,
                    data.now
                )
            }
            ErrorType::DownloadItemFail(data) => format!(
                "文件 {} 第 {} 次下载失败：{}（地址 {}）",
                data.name,
                data.times,
                data.error,
                if data.url.is_empty() {
                    "缺失"
                } else {
                    data.url.as_str()
                }
            ),
            ErrorType::DownloadFileFail => String::from("文件下载失败"),

            ErrorType::InvalidOperation => String::from("错误的操作"),

            ErrorType::SocketError(data) => format!("Socket处理出错：{}", data.error),
            ErrorType::MulticastAddrMissing => String::from("没有设置组播发送地址"),
            ErrorType::MulticastSendFailed(data) => {
                format!("向 {} 组播发送失败：{}", data.family, data.error)
            }
            ErrorType::ThreadError(data) => format!("线程启动失败：{}", data.error),
            ErrorType::ProcessError(data) => format!("进程启动失败：{}", data.error),
            ErrorType::ProcessElevationUnsupported => String::from("当前平台不支持管理员提权"),
            ErrorType::NamedPipeCreateFailed(data) => {
                format!("创建命名管道失败：{}", data.error)
            }
            ErrorType::NamedPipeEventCreateFailed(data) => {
                format!("创建命名管道事件失败：{}", data.error)
            }
            ErrorType::NamedPipeConnectTimeout => {
                String::from("命名管道连接超时，可能是用户拒绝了 UAC 提权")
            }
            ErrorType::NamedPipeWaitFailed(data) => {
                format!("等待命名管道连接失败：{}", data.error)
            }
            ErrorType::NamedPipeConnectFailed(data) => {
                format!("连接命名管道失败：{}", data.error)
            }
            ErrorType::InstanceVersionError => String::from("版本号错误"),
            ErrorType::Base64Error(data) => format!("BASE64处理失败：{}", data.error),
            ErrorType::StreamError(data) => format!("流处理异常：{}", data.error),
            ErrorType::MotdVarintTooBig => String::from("MOTD 里的 varint 超过长度上限"),

            ErrorType::KeyIsNull => String::from("密钥未设置"),

            ErrorType::SkinBlockError(data) => match data {
                SkinBlockErrorData::NameIllegal(name) => {
                    format!("皮肤方块名非法：{name}（只允许英文字母数字-_，不超过64字符）")
                }
                SkinBlockErrorData::SkinSize { width, height } => {
                    format!("皮肤尺寸须为64×64（或旧版64×32），实际{width}×{height}")
                }
                SkinBlockErrorData::PlayerNotFound => {
                    String::from("找不到该玩家，或该玩家没有皮肤")
                }
                SkinBlockErrorData::DecodeFail => String::from("皮肤PNG解码失败"),
                SkinBlockErrorData::RenderFail => String::from("皮肤方块渲染失败"),
                SkinBlockErrorData::NotFound(id) => format!("皮肤方块不存在：{id}"),
            },
            ErrorType::DownloadThreadPanic(data) => format!("下载线程异常：{}", data.error),
            ErrorType::PanicPayloadUnknown => String::from("未知panic"),
            ErrorType::ModrinthAvatarFailed(data) => {
                format!("查Modrinth作者头像失败：{}", data.error)
            }
            ErrorType::McloUploadRejected(reason) => format!("mclo.gs拒绝上传：{reason}"),
            ErrorType::MotdConnectTimeout { ip, port } => format!("连接超时：{ip}:{port}"),
            ErrorType::MotdConnectFailed { ip, port, error } => {
                format!("连接失败：{ip}:{port} {error}")
            }
            ErrorType::MotdBadResponseLen => String::from("响应长度异常"),
            ErrorType::MotdBadPacketId { packet_id } => format!("响应包类型异常：{packet_id}"),
            ErrorType::MotdResponseIncomplete => String::from("响应数据不完整"),
            ErrorType::ApngEncodeFail { stage, error } => {
                format!("方块动画图标APNG编码失败：{stage} {error}")
            }
            ErrorType::IpcRuntimeCreateError(data) => {
                format!("IPC：创建异步运行时报错：{}", data.error)
            }
            ErrorType::IpcBindError(data) => format!("IPC：绑定端口失败：{}", data.error),
            ErrorType::IpcAcceptError(data) => format!("IPC：接受连接失败：{}", data.error),
            ErrorType::IpcWriteError(data) => format!("IPC：发送数据失败：{}", data.error),
            ErrorType::IpcInvalidFrame => "IPC：数据帧长度无效".to_string(),
            ErrorType::IpcFrameTooLarge { len } => format!("IPC：数据帧过长，{len} 字节"),
            ErrorType::IpcBufferOverrun => String::from("IPC：接收缓冲区溢出，断开连接"),
        }
    }

    /// 取中文严重错误文案
    ///
    /// - `panic`: 严重错误条目
    ///
    /// # 返回值
    ///
    /// 中文成品字符串
    fn get_panic(&self, panic: &PanicType) -> String {
        match panic {
            PanicType::CoreArgLocalEmpty => String::from("运行路径为空"),
            PanicType::CoreArgLocalError => String::from("运行路径不存在"),
            PanicType::LogOpenFail(data, data1) => {
                format!("日志系统初始化失败：{} 路径：{}", data1, data)
            }
        }
    }

    /// 取中文线程名
    ///
    /// - `thread`: 线程条目
    ///
    /// # 返回值
    ///
    /// 中文线程名
    fn get_thread(&self, thread: &ThreadType) -> String {
        match thread {
            ThreadType::LogThread => String::from("日志线程"),
            ThreadType::ConfigSaveThread => String::from("配置保存线程"),
            ThreadType::LanClientV4 => String::from("局域网游戏V4监听线程"),
            ThreadType::LanClientV6 => String::from("局域网游戏V6监听线程"),
            ThreadType::LanServer => String::from("局域网游戏广播线程"),
            ThreadType::GameCount => String::from("游戏启动统计线程"),
        }
    }
}
