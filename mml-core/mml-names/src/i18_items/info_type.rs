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
}
