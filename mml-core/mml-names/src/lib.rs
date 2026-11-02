//! 启动器基础命名与国际化模块
//!
//! # 核心功能
//!
//! - **国际化 i18n** — 加载语言包并按当前语言返回文案、线程名、错误描述
//! - **路径/文件名常量** — 启动器使用的目录、文件、扩展名、参数常量
//! - **保留 UUID** — 各配置文件保存任务的固定 UUID
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`i18`] | 语言包加载与文案获取 |
//! | [`i18_items`] | 文案条目类型定义：信息 / 错误 / 严重错误 / 线程 / GUI |
//! | [`names`] | 路径、文件名与参数常量 |
//! | [`uuids`] | 配置文件保留 UUID |

pub mod i18;
pub mod i18_items;
pub mod names;
pub mod uuids;

use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    i18::{I18Lang, en_us::EnUs, zh_cn::ZhCn},
    i18_items::error_type::{CoreResult, ErrorType::FileSystemError, FileSystemErrorData},
    names::{LANG_EN_US, LANG_ZH_CN},
};

/// 配置结构主版本号，结构变了才加一
pub const VERSION_NUM: i32 = 0;
/// 小版本号
pub const VERSION_SEC_NUM: i32 = 1;
/// 修订号
pub const VERSION_PATCH_NUM: i32 = 7;
/// 构建日期，格式 yyyymmdd，由 `build.rs` 在编译期注入；只有测试版用得上
#[cfg(debug_assertions)]
const BUILD_DATE: &str = env!("MML_NAMES_BUILD_DATE");
/// 配置版本号
///
/// 测试版追加构建日期，如 `1.0.0.20261028`；正式版只有三段，如 `1.0.0`。
/// 日期跟着构建走，不用手改：换一天出的包，配置版本号自然就变了。
#[cfg(debug_assertions)]
pub const VERSION: LazyLock<String> =
    LazyLock::new(|| format!("{VERSION_NUM}.{VERSION_SEC_NUM}.{VERSION_PATCH_NUM}.{BUILD_DATE}"));

/// 配置版本号
///
/// 测试版追加构建日期，如 `1.0.0.20261028`；正式版只有三段，如 `1.0.0`
#[cfg(not(debug_assertions))]
pub const VERSION: LazyLock<String> =
    LazyLock::new(|| format!("{VERSION_NUM}.{VERSION_SEC_NUM}.{VERSION_PATCH_NUM}"));

/// 启动器界面语言
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// 简体中文
    zh_cn,
    /// 美式英语
    en_us,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::zh_cn
    }
}

/// 当前语言，进程级全局状态
static LANG: LazyLock<RwLock<Lang>> = LazyLock::new(|| {
    let local = get_current_locale();
    let lang = check_lang(&local);
    load_lang(lang);
    RwLock::new(lang)
});
/// 语言配置文件路径，由 [`init`] 首次调用时确定
static FILE: OnceLock<PathBuf> = OnceLock::new();

/// 当前平台的换行符
static LINE_ENDING: LazyLock<String> =
    LazyLock::new(|| String::from(if cfg!(windows) { "\r\n" } else { "\n" }));

/// 获取当前平台的换行符
///
/// # 返回值
///
/// Windows 为 `\r\n`，其它平台为 `\n`
pub fn get_line_ending() -> String {
    LINE_ENDING.clone()
}

/// 获取本地语言，依次读环境变量 `LANG`、`LC_ALL`、`LANGUAGE`
///
/// # 返回值
///
/// 环境变量里的语言字符串，都没有时回退为 [`LANG_ZH_CN`]
pub fn get_current_locale() -> String {
    if let Ok(lang) = env::var("LANG") {
        lang
    } else if let Ok(lang) = env::var("LC_ALL") {
        lang
    } else if let Ok(lang) = env::var("LANGUAGE") {
        lang
    } else {
        String::from(LANG_ZH_CN)
    }
}

/// 取语言对应的语言标识字符串
///
/// - `lang`: 语言枚举
///
/// # 返回值
///
/// [`LANG_ZH_CN`] 或 [`LANG_EN_US`]，也是写回语言文件的内容
pub fn get_lang(lang: Lang) -> &'static str {
    match lang {
        Lang::zh_cn => LANG_ZH_CN,
        Lang::en_us => LANG_EN_US,
    }
}

/// 从字符串判断语言类型
///
/// - `data`: 语言字符串，如 `zh_CN`、`en_US`
///
/// # 返回值
///
/// 识别出的语言，无法识别时回退为 [`Lang::zh_cn`]
fn check_lang(data: &String) -> Lang {
    if data.eq(LANG_ZH_CN) {
        return Lang::zh_cn;
    } else if data.eq(LANG_EN_US) {
        return Lang::en_us;
    }

    return Lang::zh_cn;
}

/// 把语言实现装入全局 i18 状态
fn load_lang(lang: Lang) {
    let i18: Box<dyn I18Lang + Send + Sync> = match lang {
        Lang::zh_cn => Box::new(ZhCn),
        Lang::en_us => Box::new(EnUs),
    };

    i18::set(i18);
}

/// 从语言文件读取语言类型并切换
fn load<P: AsRef<Path>>(file: P) -> CoreResult<()> {
    let mut stream = File::open(file.as_ref()).map_err(|err| {
        FileSystemError(FileSystemErrorData {
            path: file.as_ref().to_path_buf(),
            error: err.to_string(),
        })
    })?;
    let mut str = String::new();
    stream.read_to_string(&mut str).map_err(|err| {
        FileSystemError(FileSystemErrorData {
            path: file.as_ref().to_path_buf(),
            error: err.to_string(),
        })
    })?;
    let lang = check_lang(&str);
    *LANG.write().unwrap() = lang;
    load_lang(lang);

    Ok(())
}

/// 把当前语言写回语言文件
fn save() -> CoreResult<()> {
    let data = LANG.read().unwrap();
    let str = get_lang(*data);

    let file = FILE.get().unwrap();
    let mut stream = File::create(file).map_err(|err| {
        FileSystemError(FileSystemErrorData {
            path: file.to_path_buf(),
            error: err.to_string(),
        })
    })?;
    stream.write_all(str.as_bytes()).map_err(|err| {
        FileSystemError(FileSystemErrorData {
            path: file.to_path_buf(),
            error: err.to_string(),
        })
    })?;

    Ok(())
}

/// 获取当前语言类型
///
/// # 返回值
///
/// 全局状态里记录的语言
pub fn get_lang_type() -> Lang {
    LANG.read().unwrap().clone()
}

/// 切换语言并写回语言文件
///
/// - `lang`: 目标语言
///
/// # 返回值
///
/// `Ok(())` 表示已写回文件并切换生效
pub fn set_lang(lang: Lang) -> CoreResult<()> {
    *LANG.write().unwrap() = lang;
    save()?;

    load_lang(lang);

    Ok(())
}

/// 初始化语言：语言文件取 `path` 同目录下的 [`names::LANG_FILE`]，文件存在才读取
///
/// - `path`: 启动器运行目录下的任一文件路径，只用来定位目录
///
/// # 返回值
///
/// `Ok(())` 表示已读入语言文件，或文件不存在而沿用默认语言
pub fn init<P: AsRef<Path>>(path: P) -> CoreResult<()> {
    let file = path.as_ref().with_file_name(names::LANG_FILE);
    let file = FILE.get_or_init(|| file);

    if file.exists() {
        load(file)?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
