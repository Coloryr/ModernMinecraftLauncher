//! 测试专用工具：把临时目录统一放到仓库的 `target/temp` 下
//!
//! **为什么不用 `std::env::temp_dir()`**，即系统 `%TEMP%` / `/tmp`：
//! - 项目约定就是"临时文件放 `target/temp`"，见 AGENTS.md §6 / §11，不放系统盘 ——
//!   系统临时目录会被清理，测试留下的样本下次就找不到了；
//! - 测试产物散在 `%TEMP%` 里，一个用例一个随机名，排查时根本找不到；
//! - `target/` 已在 `.gitignore` 里，放这儿天然不会被误提交。
//!
//! **为什么单开一个 crate**：本仓没有"所有 crate 都依赖"的那个内核 crate
//! `mml-log` / `mml-names` / `mml-nbt` / `mml-skin*` 都不依赖 `mml-base`，
//! 而"仓库根在哪"这件事必须只有一份实现 —— 各处自己拼 `CARGO_MANIFEST_DIR/../..`
//! 迟早写歪，且改目录结构时要满仓找。所以用一个零依赖的小 crate，各 crate 按
//! `[dev-dependencies]` 引入，只进测试，不进正式产物。

use std::path::{Path, PathBuf};

/// 仓库根目录
///
/// 编译期算好：本 crate 固定在 `<仓库根>/mml-core/mml-testutil`，
/// 所以往上两级就是仓库根。用 [`Path::parent`] 而不是拼 `"../.."` ——
/// 这样得到的路径**不含 `..`**，写进日志、做前缀比较、喂给 `mml_base::init` 都干净。
fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest.to_path_buf())
}

/// 测试临时目录的根：`<仓库根>/target/temp`
///
/// 只返回路径，**不创建目录** —— 各用例本来就会 `create_dir_all`，
/// 有些还要先 `remove_dir_all` 清掉上一次的残留，这里替它们建反而碍事。
pub fn temp_dir() -> PathBuf {
    repo_root().join("target").join("temp")
}

/// 测试临时目录下的一个路径，目录名 / 文件名由调用方给
///
/// 一般用法：`mml_testutil::temp_path(format!("mml-xxx-{}", Uuid::new_v4()))`
pub fn temp_path(name: impl AsRef<Path>) -> PathBuf {
    temp_dir().join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时目录必须落在仓库的 `target/temp` 下，这是本 crate 存在的唯一理由
    #[test]
    fn temp_dir_is_under_target() {
        let dir = temp_dir();
        assert!(
            dir.ends_with(Path::new("target").join("temp")),
            "实际是 {dir:?}"
        );
        // 不含 `..`：否则写进日志 / 做前缀比较都会很难看
        assert!(
            !dir.to_string_lossy().contains(".."),
            "路径不该含 `..`：{dir:?}"
        );
        // 仓库根必须真的存在，parent() 走错层级时这条会挂
        assert!(
            dir.parent().is_some_and(|p| p.is_dir()),
            "target 目录应存在"
        );
    }

    /// temp_path 拼接在 temp_dir 之下
    #[test]
    fn temp_path_joins_under_temp_dir() {
        let p = temp_path("mml-some-case");
        assert_eq!(p.parent(), Some(temp_dir().as_path()));
    }
}
