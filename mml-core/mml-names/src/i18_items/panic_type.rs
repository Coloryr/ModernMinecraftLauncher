/// 严重错误类型
///
/// 预定义的严重错误种类，经 [`i18`](crate::i18) 转换为对应语言的字符串。
#[derive(Clone, Debug)]
pub enum PanicType {
    /// 核心启动参数中的程序路径为空
    CoreArgLocalEmpty,
    /// 核心启动参数中的程序路径无效
    CoreArgLocalError,
    /// 日志文件打开失败，依次携带路径、错误信息
    LogOpenFail(String, String),
}
