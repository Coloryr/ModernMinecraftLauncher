/// 线程类型
///
/// 预定义的线程名称种类，经 [`i18`](crate::i18) 转换为对应语言的线程名。
#[derive(Clone, Debug)]
pub enum ThreadType {
    /// 日志线程
    LogThread,
    /// 配置后台保存线程
    ConfigSaveThread,
    /// IPv4 局域网游戏监听线程
    LanClientV4,
    /// IPv6 局域网游戏监听线程
    LanClientV6,
    /// 局域网游戏广播线程
    LanServer,
    /// 游戏启动次数统计线程
    GameCount,
}
