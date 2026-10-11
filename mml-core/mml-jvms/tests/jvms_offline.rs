//! Java 运行时管理离线集成测试
//!
//! 全部离线执行：模拟的 Java 可执行文件只输出 `java -version` 风格的文本，
//! 不依赖系统真实 Java、不访问网络。覆盖：
//!
//! - `init` 创建 Java 存放目录
//! - `find_java_from_path` 在目录树中查找平台对应的 Java 可执行文件
//! - `add_item` 添加 / 校验 / 移除 Java，含配置持久化
//! - `load` 从配置加载列表，无效 Java 生成占位条目
//!
//! 平台差异：Windows 用 `.bat` 模拟 Java 输出，Unix 用 shell 脚本模拟。

use std::fs;
use std::path::PathBuf;
use std::sync::{
    Once, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use mml_config::config_obj::JvmConfigObj;
use mml_sys::ArchEnum;

/// 测试运行根目录，进程内只初始化一次
static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 保证初始化流程只执行一次，用例可能并行调用 setup
static INIT: Once = Once::new();

/// 临时目录自增计数，用于用例间隔离
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 初始化全局依赖链，进程内只执行一次，返回运行根目录
fn setup() -> PathBuf {
    INIT.call_once(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml-jvms-it-{}", uuid_like()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        RUN_DIR.set(dir.clone()).unwrap();

        mml_base::init(&dir);
        mml_config::init(&dir).expect("配置系统初始化失败");
        // add_item/remove 会调用 save()，需要后台保存线程
        mml_config::config_save::start();
    });

    RUN_DIR.get().unwrap().clone()
}

/// 生成唯一后缀，由进程号与计数拼成
fn uuid_like() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

/// 创建唯一的临时子目录
fn make_temp_dir(name: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!("mml-jvms-it-{}-{}", name, uuid_like()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 当前平台的 Java 可执行文件名
fn java_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        mml_names::names::JAVAW_FILE
    } else {
        mml_names::names::JAVA_FILE
    }
}

/// 创建一个模拟的 Java 可执行文件，输出 `java -version` 风格文本
///
/// # 返回值
///
/// 可执行文件路径，位于 `{dir}/jdk/bin/` 下
fn create_fake_java(dir: &PathBuf) -> PathBuf {
    let bin = dir.join("jdk").join("bin");
    fs::create_dir_all(&bin).unwrap();

    #[cfg(windows)]
    {
        let fake = bin.join("fake-java.bat");
        fs::write(
            &fake,
            "@echo off\r\necho fake java launcher 1>&2\r\necho openjdk version \"17.0.2\" 2024-01-16 1>&2\r\necho OpenJDK 64-Bit Server VM mixed mode 1>&2\r\nexit /b 0\r\n",
        )
        .unwrap();
        fake
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let fake = bin.join("fake-java");
        fs::write(
            &fake,
            "#!/bin/sh\necho 'fake java launcher' >&2\necho 'openjdk version \"17.0.2\" 2024-01-16' >&2\necho 'OpenJDK 64-Bit Server VM mixed mode' >&2\n",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        fake
    }
}

/// init：在运行目录下创建 Java 存放目录
#[test]
fn init_creates_java_dir() {
    let dir = setup();

    mml_jvms::init(&dir).expect("jvms 初始化失败");
    let java_dir = dir.join(mml_names::names::JAVA_DIR);
    assert!(java_dir.exists(), "应创建 {} 目录", java_dir.display());

    // 重复初始化应安全，幂等
    mml_jvms::init(&dir).expect("重复初始化应安全");
}

/// find_java_from_path：在目录树中找到平台对应的 Java 可执行文件
#[test]
fn find_java_from_path_finds_executable() {
    let dir = make_temp_dir("find");
    let expected = dir.join("jdk").join("bin").join(java_file_name());
    fs::create_dir_all(expected.parent().unwrap()).unwrap();
    fs::write(&expected, b"fake").unwrap();

    let found = mml_jvms::find_java_from_path(&dir).expect("应找到 Java 可执行文件");
    assert_eq!(found, expected);

    // 空目录，只含无关文件，应返回 None
    let empty = make_temp_dir("find-empty");
    fs::write(empty.join("readme.txt"), b"nothing here").unwrap();
    assert!(mml_jvms::find_java_from_path(&empty).is_none());
}

/// add_item / remove：添加有效 Java、持久化到配置、再移除
#[test]
fn add_and_remove_item_roundtrip() {
    let dir = setup();

    let fake_dir = make_temp_dir("fake-java");
    let fake = create_fake_java(&fake_dir);

    // 无效路径应返回 None，且不进入列表
    let bad = make_temp_dir("bad-java");
    let bad_file = bad.join("not-java.txt");
    fs::write(&bad_file, b"not java").unwrap();
    assert!(
        mml_jvms::add_item(
            Some("ut-bad".to_string()),
            bad_file.to_string_lossy().into_owned()
        )
        .is_none(),
        "非 Java 文件不应添加成功"
    );
    assert!(mml_jvms::get_java_info("ut-bad").is_none());

    // 添加有效 Java
    let added = mml_jvms::add_item(
        Some("ut-fake".to_string()),
        fake.to_string_lossy().into_owned(),
    );
    assert_eq!(
        added.as_deref().map(|item| item.name.clone()),
        Some("ut-fake".to_string()),
        "添加成功应返回名称"
    );

    let info = mml_jvms::get_java_info("ut-fake").expect("添加后应能查到");
    assert_eq!(info.major_version, 17);
    assert_eq!(info.version, "17.0.2");

    // 同名重复添加应覆盖而非报错
    let added = mml_jvms::add_item(
        Some("ut-fake".to_string()),
        fake.to_string_lossy().into_owned(),
    );
    assert_eq!(
        added.as_deref().map(|item| item.name.clone()),
        Some("ut-fake".to_string())
    );

    // 移除
    mml_jvms::remove("ut-fake");
    assert!(
        mml_jvms::get_java_info("ut-fake").is_none(),
        "移除后应查不到"
    );

    let _ = fs::remove_dir_all(&fake_dir);
    let _ = fs::remove_dir_all(&bad);
}

/// load：从配置加载 Java 列表，无效 Java 生成占位条目，主版本号为 -1
///
/// `load()` 内部用 `tokio::task::spawn` 异步逐个检测，需要运行时驱动；
/// 检测完成后会触发变更事件
#[tokio::test]
async fn load_config_with_invalid_java_creates_placeholder() {
    let dir = setup();

    let fake_dir = make_temp_dir("invalid");
    let bad_file = fake_dir.join("broken-java.txt");
    fs::write(&bad_file, b"broken").unwrap();

    // 预置配置中的 Java 列表，即内存中的全局配置
    {
        let mut config = mml_config::write_config();
        config.java_list.push(JvmConfigObj {
            name: "ut-placeholder".to_string(),
            local: bad_file.to_string_lossy().into_owned(),
        });
    }

    mml_jvms::load();

    // 轮询等待异步检测完成，最多 10 秒
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut info = None;
    while Instant::now() < deadline {
        if let Some(found) = mml_jvms::get_java_info("ut-placeholder") {
            info = Some(found);
            break;
        }
        // 让出执行权，驱动 load 内部 spawn 的异步任务
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(10));
    }

    let info = info.expect("load 应为无效 Java 创建占位条目");
    assert_eq!(info.major_version, -1, "无效 Java 的主版本号应为 -1");
    assert!(info.version.is_empty());
    assert_eq!(info.arch, ArchEnum::Unknown);
    assert!(
        mml_jvms::get_all_java()
            .iter()
            .any(|item| item.name == "ut-placeholder")
    );

    // 清理，避免影响其他用例
    mml_jvms::remove("ut-placeholder");
    {
        let mut config = mml_config::write_config();
        config.java_list.clear();
    }
    let _ = fs::remove_dir_all(&fake_dir);
}

// 说明：RUN_DIR 由后台保存线程异步写入 config.json，无法在用例结束时安全删除，
// 目录名带进程号，会残留在 target/temp 下，可由外部清理脚本统一回收
