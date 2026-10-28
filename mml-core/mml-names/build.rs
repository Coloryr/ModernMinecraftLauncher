//! 构建脚本：把编译当天的日期写进配置版本号
//!
//! `mml_names::VERSION` 形如 `1.1.20261028`，日期只在这里取一次 ——
//! 运行时再取就成了"运行日期"，隔天跑旧包会显示错日子。

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-env=MML_NAMES_BUILD_DATE={}",
        chrono::Local::now().format("%Y%m%d")
    );
}
