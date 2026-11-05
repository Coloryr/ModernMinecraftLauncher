//! 构建脚本：macOS 下补上 Xcode 的 clang 运行时
//!
//! SDL3 的 Objective-C 源码用 `@available` 判断系统版本，部署目标低于被检查的版本时
//! clang 会把它编译成对 `__isPlatformVersionAtLeast` 的调用。这个符号由 Xcode 的
//! `libclang_rt.osx.a` 提供，clang 默认会代链；rustc 用 `-nodefaultlibs` 链接，
//! std 里的那份弱符号又会被 release 的 LTO 丢掉，静态链接 SDL3 的 Release 构建因此报
//! 未定义符号。这里显式链接它，只在 macOS 生效。

use std::{path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let Ok(output) = Command::new("xcrun")
        .args(["clang", "--print-runtime-dir"])
        .output()
    else {
        println!("cargo:warning=无法执行 xcrun，SDL3 的 @available 检查可能链接失败");
        return;
    };
    if !output.status.success() {
        println!(
            "cargo:warning=xcrun clang --print-runtime-dir 执行失败，SDL3 的 @available 检查可能链接失败"
        );
        return;
    }

    let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !Path::new(&dir).join("libclang_rt.osx.a").is_file() {
        println!(
            "cargo:warning={dir} 下没有 libclang_rt.osx.a，SDL3 的 @available 检查可能链接失败"
        );
        return;
    }

    println!("cargo:rustc-link-search=native={dir}");
    // 让链接器按需取用，不打进 rlib
    println!("cargo:rustc-link-lib=static:-bundle=clang_rt.osx");
}
