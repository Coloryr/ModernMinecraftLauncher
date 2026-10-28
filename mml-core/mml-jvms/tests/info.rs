//! 扫描本机 Java 并逐条打印，供人工查看。
//!
//! 扫描会把结果写回全局配置，先起内核单例，否则 `write_config` 会 panic。

use std::{path::PathBuf, sync::OnceLock};

/// 本次运行的内核根目录，进程内只初始化一次
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化配置与 Java 管理
fn setup() {
    RUN_DIR.get_or_init(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml-jvms-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        mml_base::init(&dir);
        mml_config::init(&dir).expect("配置系统初始化失败");
        // scan_java 末尾会 save()，需要后台保存线程
        mml_config::config_save::start();
        mml_jvms::init(&dir).expect("Java 管理初始化失败");
        dir
    });
}

#[test]
fn get_java_list() {
    setup();
    mml_jvms::scan_java();
    let list2 = mml_jvms::get_all_java();
    for item in list2 {
        println!(
            "find java {}: {} {} {} {}",
            item.major_version,
            item.name,
            item.version,
            item.arch,
            item.path.to_string_lossy()
        );
    }
}
