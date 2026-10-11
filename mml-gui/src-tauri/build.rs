//! 构建脚本：Tauri 胶水 + 调用 gui-ipc-gen 生成 IPC 绑定
//!
//! 扫描与生成逻辑全在 `mml-gui/ipc-gen/`，包名 `gui-ipc-gen`，有单测，
//! 这里只负责算路径、传配置、写文件、打 cargo 指令。
//!
//! 注意：vendor 插件的还原**不能**放这里 —— cargo 在解析依赖图时就要读
//! `target/vendor/tauri-plugin-decoration/Cargo.toml`，那发生在任何 build script
//! 之前。改由各构建入口在 cargo 之前跑 `vendor/prepare.ps1`，见 build-*.bat。

use std::{collections::BTreeMap, path::PathBuf};

/// 构建入口：Tauri 构建 + 生成 IPC 绑定文件
fn main() {
    tauri_build::build();

    // src 下任何文件变动都触发重新扫描；单独重生成可跑 npm run gen
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ts_dir = manifest.join("../../mml-vue/src/lib");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let generated = gui_ipc_gen::generate(&gui_ipc_gen::GenConfig {
        src_dir: manifest.join("src"),
        // mml-names 的 Lang 是外部 crate 的 unit 枚举：线上是变体名，
        // 变体固定 zh_cn / en_us，前端一直这么写死，见 lib/guiConfig.ts 的 Locale
        external_types: BTreeMap::from([(
            "Lang".to_string(),
            "\"zh_cn\" | \"en_us\"".to_string(),
        )]),
        ..Default::default()
    })
    .expect("生成 IPC 绑定失败");

    // 输出文件也登记 rerun-if-changed：删掉或手改后会重新生成
    let outs = [
        (out_dir.join("invokes_gen.rs"), &generated.invokes_gen_rs),
        (ts_dir.join("listens.ts"), &generated.listens_ts),
        (ts_dir.join("bindings.ts"), &generated.bindings_ts),
    ];
    for (path, content) in outs {
        println!("cargo:rerun-if-changed={}", path.display());
        gui_ipc_gen::write_if_changed(&path, content)
            .unwrap_or_else(|e| panic!("写入 {} 失败: {e}", path.display()));
    }
}
