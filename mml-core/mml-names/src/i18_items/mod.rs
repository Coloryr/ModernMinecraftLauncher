//! 国际化条目类型定义
//!
//! 各枚举只定义"有哪些条目"，具体文本由 [`i18`](crate::i18) 按当前语言返回；
//! 每个类型都实现了 `Display`，可直接格式化为当前语言的字符串。

use core::fmt;

use crate::{
    i18,
    i18_items::{
        error_type::ErrorType, info_type::InfoType, panic_type::PanicType, thread_type::ThreadType,
    },
};

/// 错误条目类型
pub mod error_type;
/// 一般信息条目类型
pub mod info_type;
/// 严重错误条目类型
pub mod panic_type;
/// 线程名条目类型
pub mod thread_type;

impl fmt::Display for InfoType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", i18::with_info(self))
    }
}

impl fmt::Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", i18::with_error(self))
    }
}

impl fmt::Display for PanicType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", i18::with_panic(self))
    }
}

impl fmt::Display for ThreadType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", i18::with_thread(self))
    }
}
