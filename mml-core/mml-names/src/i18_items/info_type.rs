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
}
