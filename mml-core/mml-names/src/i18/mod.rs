use std::sync::{OnceLock, RwLock};

use crate::i18_items::{
    error_type::ErrorType, info_type::InfoType, panic_type::PanicType, thread_type::ThreadType,
};

/// 美式英语语言包
pub mod en_us;
/// 简体中文语言包，也是全局状态首次初始化时的默认实现
pub mod zh_cn;

/// 语言包实现
///
/// 一种语言一套文案查表，条目类型定义在 [`crate::i18_items`]；
/// 各方法都返回当前语言的成品字符串。
pub trait I18Lang {
    /// 取一般信息文案
    ///
    /// - `info`: 信息条目
    ///
    /// # 返回值
    ///
    /// 当前语言的成品字符串
    fn get_info(&self, info: &InfoType) -> String;
    /// 取错误文案
    ///
    /// - `error`: 错误条目，部分条目自带运行时数据
    ///
    /// # 返回值
    ///
    /// 当前语言的成品字符串
    fn get_error(&self, error: &ErrorType) -> String;
    /// 取严重错误文案
    ///
    /// - `panic`: 严重错误条目
    ///
    /// # 返回值
    ///
    /// 当前语言的成品字符串
    fn get_panic(&self, panic: &PanicType) -> String;
    /// 取线程名
    ///
    /// - `thread`: 线程条目
    ///
    /// # 返回值
    ///
    /// 当前语言的线程名
    fn get_thread(&self, thread: &ThreadType) -> String;
}

/// 当前语言包；首次访问时惰性初始化为简体中文
static I18: OnceLock<RwLock<Box<dyn I18Lang + Send + Sync>>> = OnceLock::new();

/// 替换当前语言包
///
/// - `lang_impl`: 新的语言实现，通常是 [`en_us::EnUs`] 或 [`zh_cn::ZhCn`]
pub fn set(lang_impl: Box<dyn I18Lang + Send + Sync>) {
    // 初始值只是为了拿到锁，随即被传入的实现覆盖
    let data = I18.get_or_init(|| RwLock::new(Box::new(zh_cn::ZhCn)));

    *data.write().unwrap() = lang_impl;
}

/// 取一般信息文案
///
/// - `info`: 信息条目
///
/// # 返回值
///
/// 当前语言的成品字符串
pub fn get_info(info: InfoType) -> String {
    with_info(&info)
}

/// 取错误文案
///
/// - `error`: 错误条目，部分条目自带运行时数据
///
/// # 返回值
///
/// 当前语言的成品字符串
pub fn get_error(error: ErrorType) -> String {
    with_error(&error)
}

/// 取严重错误文案
///
/// - `panic`: 严重错误条目
///
/// # 返回值
///
/// 当前语言的成品字符串
pub fn get_panic(panic: PanicType) -> String {
    with_panic(&panic)
}

/// 取线程名
///
/// - `thread`: 线程条目
///
/// # 返回值
///
/// 当前语言的线程名
pub fn get_thread(thread: ThreadType) -> String {
    with_thread(&thread)
}

/// 取一般信息文案，条目按引用传入
///
/// - `info`: 信息条目
///
/// # 返回值
///
/// 当前语言的成品字符串
pub fn with_info(info: &InfoType) -> String {
    // 尚未设置过语言包时用简体中文兜底
    let i18 = I18.get_or_init(|| RwLock::new(Box::new(zh_cn::ZhCn)));
    let lang = i18.read().unwrap();
    lang.get_info(info)
}

/// 取错误文案，条目按引用传入
///
/// - `error`: 错误条目，部分条目自带运行时数据
///
/// # 返回值
///
/// 当前语言的成品字符串
pub fn with_error(error: &ErrorType) -> String {
    let i18 = I18.get_or_init(|| RwLock::new(Box::new(zh_cn::ZhCn)));
    let lang = i18.read().unwrap();
    lang.get_error(error)
}

/// 取严重错误文案，条目按引用传入
///
/// - `panic`: 严重错误条目
///
/// # 返回值
///
/// 当前语言的成品字符串
pub fn with_panic(panic: &PanicType) -> String {
    let i18 = I18.get_or_init(|| RwLock::new(Box::new(zh_cn::ZhCn)));
    let lang = i18.read().unwrap();
    lang.get_panic(panic)
}

/// 取线程名，条目按引用传入
///
/// - `thread`: 线程条目
///
/// # 返回值
///
/// 当前语言的线程名
pub fn with_thread(thread: &ThreadType) -> String {
    let i18 = I18.get_or_init(|| RwLock::new(Box::new(zh_cn::ZhCn)));
    let lang = i18.read().unwrap();
    lang.get_thread(thread)
}
