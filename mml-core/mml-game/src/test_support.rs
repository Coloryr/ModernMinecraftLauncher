//! 测试共用的地基，仅 `#[cfg(test)]` 编译
//!
//! 内核这几个系统都是**进程级单例**：`mml_base` 的运行目录、`mml_names` 的路径、
//! `mml_log` 的日志流、`mml_config` 的保存线程。凡是需要"能落盘 / 能读盘"的用例，
//! 都得先把它们起来。
//!
//! **为什么必须只有一份**：`mml_log::STREAM` 是 `OnceLock`，`start()` 内部是
//! `STREAM.set(..).unwrap()` —— **第二次调用直接 panic**。所以各测试模块不能各自写
//! 一份 boot，必须统一走这里。
//!
//! 目录固定为 `mml-group-test-<pid>`，在 `mml-testutil::temp_dir()` 下：
//! 同一进程内所有用例共用这一个运行目录，与单例的语义一致；
//! 并行跑不同测试二进制时靠 pid 区分。

use std::path::PathBuf;
use std::sync::Once;

/// 启动全部全局单例，返回本次测试运行的目录
///
/// 幂等：重复调用只生效第一次，见 `Once`。**不清理目录** —— 日志流可能已经打开，
/// 删掉会让后续写入失败；需要干净目录的用例请自己用独立子目录。
pub fn boot() -> PathBuf {
    static INIT: Once = Once::new();
    let run = mml_testutil::temp_dir().join(format!("mml-group-test-{}", std::process::id()));
    INIT.call_once(|| {
        std::fs::create_dir_all(&run).unwrap();

        mml_base::init(&run);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_config::config_save::start();
    });
    run
}

/// 程序化造 jar / 语言表时的一个文本条目
///
/// 与 `tests/common` 里那份是同一形状，但 `tests/` 与 crate 内的 `#[cfg(test)]`
/// 是两个编译单元，类型无法互用，各留一份。
pub struct TextEntry<'a> {
    /// 条目名：jar 里是包内路径，语言表里是 key
    pub name: &'a str,
    /// 文本内容，写盘时按 UTF-8 取字节
    pub text: &'a str,
}

/// 造 [`TextEntry`]
pub fn text_entry<'a>(name: &'a str, text: &'a str) -> TextEntry<'a> {
    TextEntry { name, text }
}
