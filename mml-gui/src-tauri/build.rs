//! 构建脚本：Tauri 胶水 + 调用 gui-ipc-gen 生成 IPC 绑定
//!
//! 扫描与生成逻辑全在仓库根 ipc-gen/，包名 gui-ipc-gen，有单测，
//! 这里只负责算路径、传配置、写文件、打 cargo 指令。
//!
//! 注意：vendor 插件的还原**不能**放这里 —— cargo 在解析依赖图时就要读
//! target/vendor/tauri-plugin-decoration/Cargo.toml，那发生在任何 build script
//! 之前。改由各构建入口在 cargo 之前跑 vendor/prepare.ps1，见 build-*.bat。

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// 构建入口：Tauri 构建 + 生成 IPC 绑定文件
fn main() {
    tauri_build::build();

    // 单元测试需要链接器生成清单，才能从 v6 控件库加载 TaskDialogIndirect。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
        // 应用已由 Tauri 嵌入完整清单，此参数必须在 EMBED 后，避免资源重复。
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }

    // src 下任何文件变动都触发重新扫描；单独重生成可跑 npm run gen
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");

    // 测试版 / 预发布版的版本号要带构建日期（如 1.0.0.20261028），正式版不带。
    // 日期在这里定死：运行时再取就成了"运行日期"，隔天跑旧包会显示错日子。
    println!(
        "cargo:rustc-env=MML_BUILD_DATE={}",
        chrono::Local::now().format("%Y%m%d")
    );

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // 前端契约落在独立的 mml-ipc npm 包，mml-vue 与 custom 下的示例都从那里取
    let ts_dir = manifest.join("../../mml-ipc/src");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let generated = gui_ipc_gen::generate(&gui_ipc_gen::GenConfig {
        src_dir: manifest.join("src"),
        // mml-names 的 Lang 是外部 crate 的 unit 枚举：线上是变体名，
        // 变体固定 zh_cn / en_us，前端一直这么写死，见 lib/guiConfig.ts 的 Locale
        external_types: BTreeMap::from([("Lang".to_string(), "\"zh_cn\" | \"en_us\"".to_string())]),
        ..Default::default()
    })
    .expect("生成 IPC 绑定失败");

    // 输出文件也登记 rerun-if-changed：删掉或手改后会重新生成
    let mut outs: Vec<(PathBuf, &str)> = vec![
        (out_dir.join("invokes_gen.rs"), &generated.invokes_gen_rs),
        (ts_dir.join("listens.ts"), &generated.listens_ts),
        (ts_dir.join("bindings.ts"), &generated.bindings_ts),
    ];
    for file in &generated.type_files {
        outs.push((ts_dir.join(&file.rel_path), &file.content));
    }
    for (path, content) in outs {
        println!("cargo:rerun-if-changed={}", path.display());
        gui_ipc_gen::write_if_changed(&path, content)
            .unwrap_or_else(|e| panic!("写入 {} 失败: {e}", path.display()));
    }

    // DTO 源文件删掉后，旧的 types/*.ts 要跟着清掉，否则包里留着没人引用的死文件
    prune_types(&ts_dir.join("types"), &generated.type_files);
}

/// 删掉 types/ 下不在本次生成结果里的 .ts
///
/// - types_dir: types/ 目录，可能还不存在
/// - files: 本次生成的文件，rel_path 相对 mml-ipc/src
fn prune_types(types_dir: &Path, files: &[gui_ipc_gen::GeneratedFile]) {
    let expected: BTreeSet<String> = files
        .iter()
        .filter_map(|f| f.rel_path.strip_prefix("types/"))
        .map(|s| s.to_string())
        .collect();
    prune_dir(types_dir, types_dir, &expected);
}

/// 递归清理；base 保持不变，用来算相对路径
fn prune_dir(base: &Path, dir: &Path, expected: &BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            prune_dir(base, &path, expected);
            continue;
        }
        let rel = path
            .strip_prefix(base)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if path.extension().is_some_and(|e| e == "ts") && !expected.contains(&rel) {
            let _ = std::fs::remove_file(&path);
        }
    }
}
