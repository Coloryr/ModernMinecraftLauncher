use std::env;

use mml_sys::open_helper;

/// 手动验证：用系统默认程序打开仓库根下的 .gitignore
#[ignore = "reason"]
#[test]
pub fn open_file() {
    let exe_path = env::current_exe().expect("Failed to get exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");
    let dir = exe_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    println!("{}", dir.to_string_lossy());

    open_helper::open_file(dir.join(".gitignore"));
    // open_helper::open_file_with_explorer(dir.join("Cargo.toml"));
    // open_helper::open_file_with_explorer(dir);
}

/// 手动验证：用默认浏览器打开网址
#[ignore = "reason"]
#[test]
pub fn open_url() {
    open_helper::open_url("https://www.baidu.com");
}
