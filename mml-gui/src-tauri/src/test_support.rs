//! 单元测试的共用启动，仅 `#[cfg(test)]` 编译
//!
//! 内核那几个系统 [`mml_base`] / [`mml_names`] / [`mml_log`] 是**进程级单例**：
//! `mml_log` 的 `STREAM` / `SEM` 都是 `OnceLock`，第二次 `start()` 会 panic；而没
//! `start()` 过就调 `info()` / `error()` 一样会 panic，内部是 `SEM.get().unwrap()`。
//! 所以同一 crate 里的用例**只在这里 boot 一次**，与 mml-game 的 `test_support` 同一口径，
//! 见 AGENTS.md §6。
//!
//! 临时目录一律走 [`mml_testutil`]，落在仓库的 `target/temp` 下，不要用系统 `%TEMP%`。

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Once,
};

/// 启动内核单例，每进程一次；重复调用是安全的空操作
///
/// 用到 `mml_log` 或 `mml_base` 的用例都要先调它。
pub fn ensure_boot() {
    static BOOT: Once = Once::new();
    BOOT.call_once(|| {
        let dir = mml_testutil::temp_path(format!("mml-gui-test-boot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(&dir).unwrap();
        mml_log::start(&dir).unwrap();
    });
}

/// 一个干净的测试临时目录，建在 `target/temp` 下，按 `tag` 加随机名分开
///
/// **每个用例各用各的子目录**：落盘是异步的，共用目录会互相踩，见 AGENTS.md §6。
///
/// - `tag`: 用例特征名，用来在临时目录里认出是谁的
///
/// # 返回值
///
/// 已建好的空目录路径
pub fn temp_dir(tag: &str) -> PathBuf {
    let dir = mml_testutil::temp_path(format!("mml-gui-{tag}-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 用完清掉临时目录，用例失败时留着现场方便查
///
/// - `dirs`: 要删的目录列表，删不掉就忽略
pub fn cleanup(dirs: &[&Path]) {
    for dir in dirs {
        let _ = fs::remove_dir_all(dir);
    }
}
