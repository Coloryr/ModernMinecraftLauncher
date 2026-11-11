use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};

use mml_config::config_obj::JvmConfigObj;
use mml_sys::ArchEnum;

use super::*;

/// 涉及全局 Java 列表 / 全局配置的用例串行化，避免相互干扰
static JVMS_LOCK: Mutex<()> = Mutex::new(());

/// 测试运行根目录与一次性初始化
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();
static INIT: Once = Once::new();

/// 初始化配置系统与后台保存线程等全局依赖，返回运行根目录
fn ensure_env() -> PathBuf {
    INIT.call_once(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml-jvms-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_config::init(&dir).unwrap();
        // remove/remove_all 会调用 save()，需要后台保存线程已启动
        mml_config::config_save::start();

        RUN_DIR.set(dir).unwrap();
    });

    RUN_DIR.get().unwrap().clone()
}

/// 构造测试用 Java 信息
fn java_info(name: &str, major: i32, arch: ArchEnum) -> JavaInfoObj {
    JavaInfoObj {
        name: name.to_string(),
        path: PathBuf::from(format!("/fake/{name}/bin/java")),
        version: format!("{major}.0.0"),
        major_version: major,
        java_type: "OpenJDK".to_string(),
        vendor: "OpenJDK".to_string(),
        arch,
    }
}

/// 向全局列表插入测试条目，返回清理函数，避免污染其他用例
fn insert(entries: Vec<JavaInfoObj>) -> impl FnOnce() {
    let mut list = JVMS.write().unwrap();
    for info in entries {
        list.insert(info.name.clone(), Arc::new(info));
    }

    drop(list);
    || {
        let mut list = JVMS.write().unwrap();
        // 移除所有 ut- 前缀的测试条目
        list.retain(|name, _| !name.starts_with("ut-"));
    }
}

/// 与当前系统架构不同的架构，用于架构过滤测试
fn other_arch() -> ArchEnum {
    if mml_sys::get_system_info().system_arch == ArchEnum::X86_64 {
        ArchEnum::AArch64
    } else {
        ArchEnum::X86_64
    }
}

/// get_java：精确匹配与更高版本匹配
#[test]
fn get_java_exact_and_over() {
    ensure_env();
    let _guard = JVMS_LOCK.lock().unwrap();

    let sys_arch = mml_sys::get_system_info().system_arch;
    let cleanup = insert(vec![
        java_info("ut-j17", 17, sys_arch),
        java_info("ut-j21", 21, sys_arch),
    ]);

    // 精确匹配
    let found = get_java(17, false).expect("应找到 major=17 的 Java");
    assert_eq!(found.major_version, 17);

    // over=true 时取更高版本，降序取最新
    let found = get_java(17, true).expect("应找到 major>=17 的 Java");
    assert_eq!(found.major_version, 21);

    // 精确匹配不存在的版本
    assert!(get_java(99, false).is_none());

    cleanup();
}

/// get_java：过滤与当前系统架构不一致的 Java
#[test]
fn get_java_filters_arch() {
    ensure_env();
    let _guard = JVMS_LOCK.lock().unwrap();

    let sys_arch = mml_sys::get_system_info().system_arch;
    let cleanup = insert(vec![
        java_info("ut-j21", 21, sys_arch),
        java_info("ut-j30-bad-arch", 30, other_arch()),
    ]);

    // 架构不一致的高版本应被过滤掉
    let found = get_java(17, true).expect("应找到架构匹配的 Java");
    assert_eq!(found.major_version, 21);
    assert!(get_java(30, true).is_none(), "架构不匹配的 Java 不应返回");

    cleanup();
}

/// get_java_info / get_all_java 的读取行为
#[test]
fn get_java_info_and_all() {
    let _guard = JVMS_LOCK.lock().unwrap();

    let cleanup = insert(vec![java_info("ut-info", 8, ArchEnum::X86_64)]);

    let info = get_java_info("ut-info").expect("应能按名称取到 Java 信息");
    assert_eq!(info.major_version, 8);
    assert_eq!(info.java_type, "OpenJDK");

    // 不存在的名称返回 None
    assert!(get_java_info("no-such-java").is_none());

    // 全量列表包含插入条目
    assert!(get_all_java().iter().any(|item| item.name == "ut-info"));

    cleanup();
    // 清理后取不到
    assert!(get_java_info("ut-info").is_none());
}

/// remove：同时从内存列表和配置文件移除
#[test]
fn remove_deletes_from_memory_and_config() {
    ensure_env();
    let _guard = JVMS_LOCK.lock().unwrap();

    let cleanup = insert(vec![java_info("ut-remove", 11, ArchEnum::X86_64)]);

    // 预置配置条目
    {
        let mut config = mml_config::write_config();
        config.java_list.push(JvmConfigObj {
            name: "ut-remove".to_string(),
            local: "java/fake".to_string(),
        });
    }

    remove("ut-remove");

    // 内存中已移除
    assert!(get_java_info("ut-remove").is_none());
    // 配置中已移除
    let config = mml_config::read_config();
    assert!(
        !config.java_list.iter().any(|item| item.name == "ut-remove"),
        "配置中的 Java 条目应被移除"
    );

    // 移除不存在的名称不应 panic
    drop(config);
    remove("no-such-java");

    cleanup();
}

/// remove_all：清空内存列表与配置
#[test]
fn remove_all_clears_everything() {
    ensure_env();
    let _guard = JVMS_LOCK.lock().unwrap();

    let cleanup = insert(vec![
        java_info("ut-all-1", 8, ArchEnum::X86_64),
        java_info("ut-all-2", 17, ArchEnum::X86_64),
    ]);

    {
        let mut config = mml_config::write_config();
        config.java_list.push(JvmConfigObj {
            name: "ut-all-1".to_string(),
            local: "java/fake".to_string(),
        });
    }

    remove_all();

    assert!(get_all_java().is_empty(), "remove_all 应清空内存列表");
    let config = mml_config::read_config();
    assert!(config.java_list.is_empty(), "remove_all 应清空配置列表");

    cleanup();
}

/// 变更事件：注册 / 触发 / 注销
#[test]
fn jvm_change_event_subscribe_and_unsubscribe() {
    let counter = Arc::new(AtomicU64::new(0));

    let counter_clone = counter.clone();
    let id = add_jvm_change(move || {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    invoke_jvm_change();
    assert_eq!(counter.load(Ordering::SeqCst), 1, "触发后应回调一次");

    invoke_jvm_change();
    assert_eq!(counter.load(Ordering::SeqCst), 2, "再次触发应再次回调");

    // 注销后不再回调
    remove_jvm_change(id);
    invoke_jvm_change();
    assert_eq!(counter.load(Ordering::SeqCst), 2, "注销后不应再回调");
}

/// scan_java：在未安装 Java 的环境下也应安全返回，有 Java 则正常入列
#[test]
fn scan_java_does_not_panic() {
    let _guard = JVMS_LOCK.lock().unwrap();

    // 记录扫描前已有条目，扫描后清掉新增条目，
    // 避免本机真实 JVM 残留干扰其他用例，如 get_java 的版本匹配断言
    let before: std::collections::HashSet<String> = JVMS.read().unwrap().keys().cloned().collect();

    scan_java();

    // 无论是否找到 Java，列表内容都是合法的
    for item in get_all_java() {
        assert!(!item.version.is_empty() || item.major_version > 0);
    }

    JVMS.write()
        .unwrap()
        .retain(|name, _| before.contains(name));
}
