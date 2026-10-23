use std::{path::PathBuf, result};

use crate::i18_items::panic_type::PanicType;

/// 通用错误数据，只有一条错误描述
#[derive(Clone, Debug)]
pub struct ErrorData {
    /// 错误描述文本，由出错处生成
    pub error: String,
}

impl ErrorData {
    /// 由任意可显示的出错值构造错误数据
    ///
    /// - `error`: 出错原因，取它的 `Display` 输出
    ///
    /// # 返回值
    ///
    /// 见 [`ErrorData`]
    pub fn from_error(error: impl std::fmt::Display) -> Self {
        Self {
            error: error.to_string(),
        }
    }
}

/// HTTP 请求错误信息
#[derive(Clone, Debug)]
pub struct HttpErrorData {
    /// 请求地址
    pub url: String,
    /// 错误描述，请求阶段失败时是底层网络错误
    pub error: String,
    /// 响应状态码；请求阶段失败还没有响应，为 `None`
    pub status: Option<u16>,
}

/// 路径找不到
#[derive(Clone, Debug)]
pub struct PathNotExistsData {
    /// 找不到的路径
    pub path: PathBuf,
}

/// 文件系统错误
#[derive(Clone, Debug)]
pub struct FileSystemErrorData {
    /// 出错的路径
    pub path: PathBuf,
    /// 错误描述
    pub error: String,
}

/// 移动到回收站失败的数据
#[derive(Clone, Debug)]
pub struct MoveToTrashErrorData {
    /// 出错的路径
    pub path: PathBuf,
    /// Windows 下的 SHFileOperationW 返回码，其它平台为 0
    pub code: i32,
}

/// 组播发送失败的数据
#[derive(Clone, Debug)]
pub struct MulticastSendErrorData {
    /// 地址族，`IPv4` 或 `IPv6`
    pub family: String,
    /// 失败原因
    pub error: String,
}

/// 压缩包处理错误
#[derive(Clone, Debug)]
pub struct ArchiveErrorData {
    /// 源路径
    pub source: String,
    /// 目标路径
    pub target: String,
    /// 错误描述
    pub error: String,
}

/// 下载文件大小不符的数据
#[derive(Clone, Debug)]
pub struct DownloadFileSizeErrorData {
    /// 本地文件路径
    pub file: PathBuf,
    /// 下载地址
    pub url: String,
    /// 实际大小，单位字节
    pub now: u64,
    /// 预期大小，单位字节
    pub size: u64,
}

/// 单个文件下载失败的数据
#[derive(Clone, Debug)]
pub struct DownloadItemFailData {
    /// 文件名，取下载项的显示名
    pub name: String,
    /// 第几次失败，从 1 开始
    pub times: i32,
    /// 失败原因，已按当前语言渲染
    pub error: String,
    /// 下载地址；下载项没给地址时为空串
    pub url: String,
}

/// 下载文件覆盖失败的数据
#[derive(Clone, Debug)]
pub struct DownloadFileOverFailData {
    /// 覆盖失败的本地文件路径
    pub file: PathBuf,
    /// 覆盖失败的原因
    pub error: Box<ErrorType>,
}

/// 下载文件校验失败的数据
#[derive(Clone, Debug)]
pub struct DownloadFileHashErrorData {
    /// 本地文件路径
    pub file: PathBuf,
    /// 实际校验值
    pub now: String,
    /// 预期校验值
    pub hash: String,
}

/// 名字不合法的原因
#[derive(Clone, Debug)]
pub enum NameInvalidReason {
    /// 去掉首尾空白后为空
    Empty,
    /// 含不允许的字符，值 = 第一个非法字符
    IllegalChar(char),
    /// 字符数超过上限
    TooLong {
        /// 实际字符数
        len: usize,
        /// 允许的最大字符数
        max: usize,
    },
    /// 以点结尾，Windows 会把结尾的点吃掉
    TrailingDot,
    /// Windows 保留设备名，如 CON / NUL / COM1
    Reserved,
}

/// 名字不合法：实例名 / 分组名等在创建前校验
#[derive(Clone, Debug)]
pub struct NameInvalidData {
    /// 用户输入的名字，原样
    pub name: String,
    /// 不合法的原因
    pub reason: NameInvalidReason,
}

/// 输入参数为空的种类
#[derive(Clone, Debug)]
pub enum ArgEmptyData {
    /// 名字参数为空
    Name,
    /// UUID 参数为空
    UUID,
    /// 版本参数为空
    Version,
}

/// 输入参数错误的种类
#[derive(Clone, Debug)]
pub enum ArgErrorData {
    /// 压缩包类型参数错误
    ArchiveType,
}

/// 所需数据找不到的种类
#[derive(Clone, Debug)]
pub enum DataNotFoundData {
    /// 请求的信息
    Info,
    /// 注册表键，值 = 键名
    RegistryKey(String),
    /// 请求的网址
    Url,
    /// 游戏实例
    GameInstance,
    /// 游戏版本，值 = 版本号
    Version(String),
}

/// 皮肤方块错误
#[derive(Clone, Debug)]
pub enum SkinBlockErrorData {
    /// 方块名非法，值 = 传入的名字；只允许英文字母、数字、`-`、`_`，不超过 64 字符
    NameIllegal(String),
    /// 皮肤尺寸不符，须 64×64 或旧版 64×32；`width` / `height` 是实际尺寸，单位像素
    SkinSize { width: u32, height: u32 },
    /// 找不到玩家，或该玩家没有皮肤
    PlayerNotFound,
    /// 皮肤 PNG 解码失败
    DecodeFail,
    /// 图标渲染失败
    RenderFail,
    /// 皮肤方块不存在，值 = 方块 ID
    NotFound(String),
}

/// mml 执行结果，错误侧统一是 [`ErrorType`]
pub type CoreResult<T> = result::Result<T, ErrorType>;

/// mml 错误类型
#[derive(Clone, Debug)]
pub enum ErrorType {
    /// 严重错误，直接结束程序
    Panic(PanicType),

    /// 配置文件处理时出错
    ConfigError(FileSystemErrorData),

    /// Http请求出错
    HttpError(HttpErrorData),

    /// 网络请求被中断：用户改了代理或网络设置，在途请求全部作废
    Aborted {
        /// 请求地址；中断发生在请求任务内部时取不到，为空串
        url: String,
    },

    /// 序列化处理错误
    SerializerError(ErrorData),

    /// 账户操作错误，值 = 失败原因
    AuthFail(String),
    /// 账户操作没有返回档案
    AuthNoProfile,
    /// 登录密钥过期
    AuthTokenTimeout,
    /// 账户缺少服务器地址，旧版数据未保存，需删除后重新添加
    AuthServerNull,

    /// OAuth获取登录码错误
    OAuthGetTokenError(ErrorData),
    /// OAuth获取不到登录码
    OAuthGetTokenEmpty,

    /// 文件系统处理错误
    FileSystemError(FileSystemErrorData),
    /// 路径越界或经过不允许跟随的链接
    UnsafePath {
        /// 允许操作的根目录
        root: PathBuf,
        /// 被拒绝的路径
        path: PathBuf,
    },
    /// 移动到回收站失败
    MoveToTrashError(MoveToTrashErrorData),
    /// 文件获取错误
    FileReadError(ErrorData),
    /// 路径不存在
    PathNotExists(PathNotExistsData),

    /// 压缩包打开错误
    ArchiveOpenError(FileSystemErrorData),
    /// 压缩包读取错误
    ArchiveReadError(ErrorData),
    /// 压缩文件处理错误
    ArchiveError(ArchiveErrorData),
    /// 压缩文件写错误
    ArchiveWriteError(ErrorData),

    /// 任务取消
    TaskCancel,
    /// 任务执行超时
    TaskTimeout,
    /// 任务出错
    TaskError(ErrorData),

    /// NBT类型错误
    NbtTypeError,
    /// NBT读取失败
    NbtReadError,

    /// 输入参数为空
    ArgEmpty(ArgEmptyData),
    /// 输入参数错误
    ArgError(ArgErrorData),
    /// 所需文件未能找到
    DataNotFound(DataNotFoundData),
    /// 没有可用的 GPU 后端
    GpuNotAvailable,
    /// 实例名字已存在，值 = 名字
    InstanceNameExists(String),
    /// 名字不合法，见 [`NameInvalidData`]
    NameInvalid(NameInvalidData),
    /// 皮肤方块错误
    SkinBlockError(SkinBlockErrorData),
    /// 找不到合适的Java
    JavaNotFound,

    /// 下载文件覆盖错误
    DownloadFileOverFail(DownloadFileOverFailData),
    /// 下载文件的预期大小不符合
    DownloadFileSizeError(DownloadFileSizeErrorData),
    /// 下载文件校验失败
    DownloadFileHashError(DownloadFileHashErrorData),
    /// 单个文件下载失败一次
    DownloadItemFail(DownloadItemFailData),
    /// 文件下载失败
    DownloadFileFail,

    /// 错误的操作
    InvalidOperation,

    /// Socket处理出错
    SocketError(ErrorData),
    /// 组播发送地址未设置
    MulticastAddrMissing,
    /// 组播发送失败
    MulticastSendFailed(MulticastSendErrorData),
    /// 线程启动错误
    ThreadError(ErrorData),
    /// 进程启动错误
    ProcessError(ErrorData),
    /// 当前平台不支持提权
    ProcessElevationUnsupported,
    /// 创建命名管道失败
    NamedPipeCreateFailed(ErrorData),
    /// 创建命名管道事件失败
    NamedPipeEventCreateFailed(ErrorData),
    /// 命名管道连接超时
    NamedPipeConnectTimeout,
    /// 等待命名管道连接失败
    NamedPipeWaitFailed(ErrorData),
    /// 连接命名管道失败
    NamedPipeConnectFailed(ErrorData),
    /// 版本号错误
    InstanceVersionError,
    /// BASE64错误
    Base64Error(ErrorData),
    /// 流处理异常
    StreamError(ErrorData),
    /// MOTD 里的 varint 超过长度上限
    MotdVarintTooBig,

    /// 密钥未设置
    KeyIsNull,

    /// 下载线程 panic，值 = panic 文本
    DownloadThreadPanic(ErrorData),
    /// panic 载荷既不是字符串也不是静态串，拿不到原文
    PanicPayloadUnknown,
    /// 查 Modrinth 作者头像失败
    ModrinthAvatarFailed(ErrorData),
    /// mclo.gs 拒绝上传，值 = 对方给的原因
    McloUploadRejected(String),

    /// MOTD 查询：连接超时
    MotdConnectTimeout {
        /// 服务器地址
        ip: String,
        /// 端口
        port: u16,
    },
    /// MOTD 查询：连接失败
    MotdConnectFailed {
        /// 服务器地址
        ip: String,
        /// 端口
        port: u16,
        /// 底层网络错误
        error: String,
    },
    /// MOTD 查询：响应长度非法
    MotdBadResponseLen,
    /// MOTD 查询：响应包类型不对
    MotdBadPacketId {
        /// 实际收到的包 id
        packet_id: i32,
    },
    /// MOTD 查询：响应数据不完整
    MotdResponseIncomplete,

    /// 生成方块动画图标时 APNG 编码失败
    ApngEncodeFail {
        /// 失败的 png 编码步骤名，如 `set_animated`
        stage: String,
        /// 底层编码错误
        error: String,
    },
    /// IPC：创建异步运行时失败
    IpcRuntimeCreateError(ErrorData),
    /// IPC：绑定端口失败
    IpcBindError(ErrorData),
    /// IPC：接受连接失败
    IpcAcceptError(ErrorData),
    /// IPC：发送数据失败
    IpcWriteError(ErrorData),
    /// IPC：数据帧超过长度上限
    IpcFrameTooLarge {
        /// 实际长度，单位字节
        len: usize,
    },
    /// IPC：接收缓冲区溢出，断开连接
    IpcBufferOverrun,
}
