//! Tauri 构建脚本：配置 / 图标 / capabilities 由 tauri-build 处理。

fn main() {
    tauri_build::build();
    // 图标要嵌进 exe 资源；不盯着它的话换了图标也不会重新打包
    println!("cargo:rerun-if-changed=icons");
    // 预览脚本作为资源打包，改动后要重新拷贝
    println!("cargo:rerun-if-changed=../preview");
    println!("cargo:rerun-if-changed=../lib/messages.mjs");
}
