/// 信息类型
///
/// 预定义的一般信息种类，经 [`i18`](crate::i18) 转换为对应语言的字符串。
#[derive(Clone, Debug)]
pub enum InfoType {
    /// 启动器核心启动，文案里带版本号
    CoreStart,
    /// 启动器核心停止
    CoreStop,

    /// 临时文件
    TempFile,

    /// 接管了一个 MMC 实例，带实例名
    AdoptMmcInstance {
        /// 实例名，取 MMC 配置或目录名
        name: String,
    },

    /// 下载任务结束：总数 / 成功数 / 失败数
    DownloadTaskDone {
        /// 文件总数
        total: usize,
        /// 成功数
        completed: usize,
        /// 失败数
        failed: usize,
    },

    /// 代理描述：手动代理，带类型 / 地址 / 端口与是否认证
    ProxyUser {
        /// 代理类型名，如 `http` / `socks5`
        kind: String,
        /// 代理地址
        ip: String,
        /// 代理端口
        port: u16,
        /// 代理是否带用户名密码
        auth: bool,
    },
    /// 代理描述：禁用代理
    ProxyNone,
    /// 代理描述：跟随系统
    ProxyAuto,

    /// HTTP 客户端已初始化，带通用 / 登录两个客户端的代理描述
    HttpClientInit {
        /// 通用客户端的代理描述
        work: String,
        /// 登录客户端的代理描述
        login: String,
    },
    /// HTTP 客户端重启，带被中断的在途请求世代号
    HttpClientRestart {
        /// 世代号，每次重启递增
        generation: u64,
    },
    /// HTTP 客户端重启完成，字段同 [`InfoType::HttpClientInit`]
    HttpClientRestartDone {
        /// 通用客户端的代理描述
        work: String,
        /// 登录客户端的代理描述
        login: String,
    },
    /// 已通知运行中的启动器实例，带 IPC 端口
    IpcNotifyExisting {
        /// IPC 服务端口
        port: u16,
    },
    /// 通知运行中的实例失败，改为启动新的 IPC 服务，带端口
    IpcNotifyExistingFailed {
        /// IPC 服务端口
        port: u16,
    },
    /// IPC 服务已启动，带监听端口
    IpcServerStart {
        /// 监听端口
        port: u16,
    },
    /// 有一个 IPC 客户端连上，带对端地址
    IpcClientConnected {
        /// 对端地址，形如 `127.0.0.1:12345`
        addr: String,
    },
    /// 收到未知类型的 IPC 消息，带消息类型号
    IpcUnknownMessageType {
        /// 消息类型号
        msg_type: i32,
    },
    /// 已运行实例请求显示主窗口
    IpcLauncherShowWindow,
    /// 游戏通道已绑定，带实例 ID
    IpcGameChannelBound {
        /// 实例 ID
        guid: String,
    },
}
