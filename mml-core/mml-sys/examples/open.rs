//! 用系统默认程序打开文件或网址，手动验证 `open_helper` 的两个入口
//!
//! 运行：
//! - `cargo run -p mml-sys --example open` 打开仓库根的 `.gitignore`
//! - `cargo run -p mml-sys --example open -- file <路径>` 打开指定文件
//! - `cargo run -p mml-sys --example open -- url <地址>` 打开网址

use std::path::PathBuf;

use mml_sys::open_helper;

/// 仓库根目录，用来给缺省参数挑一个文件
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("url"), Some(url)) => open_helper::open_url(&url),
        (Some("file"), Some(path)) => open_helper::open_file(path),
        _ => open_helper::open_file(repo_root().join(".gitignore")),
    }
}
