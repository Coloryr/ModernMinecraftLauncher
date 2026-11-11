use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use mml_names::names;

use super::*;

/// 唯一临时子目录，由进程号 + 自增计数拼成，避免并行冲突
fn make_temp_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = mml_testutil::temp_dir().join(format!(
        "mml-jvms-unit-{}-{}-{}",
        name,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 主版本号解析：传统 1.x 格式
#[test]
fn major_version_legacy_format() {
    assert_eq!(get_major_version("1.8.0_201"), 8);
    assert_eq!(get_major_version("1.7.0_80"), 7);
    assert_eq!(get_major_version("1.6"), 6);
}

/// 主版本号解析：新格式，JDK 9 起
#[test]
fn major_version_new_format() {
    assert_eq!(get_major_version("11.0.2"), 11);
    assert_eq!(get_major_version("17.0.1"), 17);
    assert_eq!(get_major_version("21.0.5+11"), 21);
    assert_eq!(get_major_version("21"), 21);
    assert_eq!(get_major_version("8"), 8);
}

/// 主版本号解析：非法输入回落为 0
#[test]
fn major_version_invalid_returns_zero() {
    assert_eq!(get_major_version(""), 0);
    assert_eq!(get_major_version("abc"), 0);
    assert_eq!(get_major_version("1.x"), 0);
    assert_eq!(get_major_version("x.8"), 0);
}

/// 不存在的路径返回 None
#[test]
fn test_java_missing_path_returns_none() {
    let dir = make_temp_dir("missing");
    let result = test_java(dir.join("no-such-java").join(names::JAVA_FILE));
    assert!(result.is_none());
}

/// 目录而非文件时返回 None
#[test]
fn test_java_directory_returns_none() {
    let dir = make_temp_dir("dir");
    assert!(test_java(&dir).is_none());
}

/// 存在但不是可执行 Java 的文件返回 None
#[test]
fn test_java_not_java_returns_none() {
    let dir = make_temp_dir("notjava");
    let file = dir.join("not-java.txt");
    fs::write(&file, b"i am not java").unwrap();

    // Windows 上 .txt 无法作为进程启动；Unix 上无可执行权限
    assert!(test_java(&file).is_none(), "普通文本文件不应被识别为 Java");
}

/// 用模拟输出验证 `java -version` 的解析逻辑，Windows 走批处理文件
#[cfg(windows)]
#[test]
fn test_java_parses_fake_bat_output() {
    let dir = make_temp_dir("fake-bat");
    // test_java 以 path.parent().parent() 作为工作目录，因此需要 bin 结构
    let bin = dir.join("jdk").join("bin");
    fs::create_dir_all(&bin).unwrap();

    let fake = bin.join("fake-java.bat");
    fs::write(
        &fake,
        "@echo off\r\necho fake java launcher 1>&2\r\necho openjdk version \"17.0.2\" 2024-01-16 1>&2\r\necho OpenJDK 64-Bit Server VM mixed mode 1>&2\r\nexit /b 0\r\n",
    )
    .unwrap();

    let info = test_java(&fake).expect("模拟 java 输出应被正确解析");
    assert_eq!(info.version, "17.0.2");
    assert_eq!(info.major_version, 17);
    assert_eq!(info.java_type, "openjdk");
    assert_eq!(info.vendor, "OpenJDK");
    // 输出包含 64-Bit，架构应与当前系统的 64 位架构一致
    assert_eq!(info.arch, mml_sys::get_system_info().system_arch);
    assert_eq!(info.path, fake);
    assert!(info.name.starts_with("openjdk-17.0.2-"));
}

/// 发行商识别：运行时行优先，路径只作兜底
#[test]
fn detect_vendor_from_output_and_path() {
    use std::path::Path;

    let empty = Path::new("");
    assert_eq!(
        detect_vendor(
            "openjdk version \"21.0.5\"\nOpenJDK Runtime Environment Temurin-21.0.5+11",
            empty,
            "openjdk"
        ),
        "Temurin"
    );
    assert_eq!(
        detect_vendor(
            "openjdk version \"21.0.5\"\nOpenJDK Runtime Environment Zulu21.38+21-CA",
            empty,
            "openjdk"
        ),
        "Zulu"
    );
    assert_eq!(
        detect_vendor(
            "java version \"21.0.5\"\nJava(TM) SE Runtime Environment",
            empty,
            "java"
        ),
        "Oracle"
    );
    assert_eq!(
        detect_vendor(
            "openjdk version \"21.0.5\"\nOpenJDK Runtime Environment (build 21)",
            empty,
            "openjdk"
        ),
        "OpenJDK"
    );
    assert_eq!(
        detect_vendor("java version \"1.8.0\"", empty, "java"),
        "Java"
    );
    // 输出里没有厂商名时才看路径
    assert_eq!(
        detect_vendor(
            "openjdk version \"21.0.5\"",
            Path::new("C:\\Program Files\\Zulu\\zulu-21"),
            "openjdk"
        ),
        "Zulu"
    );
}

/// 用模拟输出验证 `java -version` 的解析逻辑，Unix 走 shell 脚本
#[cfg(unix)]
#[test]
fn test_java_parses_fake_sh_output() {
    let dir = make_temp_dir("fake-sh");
    let bin = dir.join("jdk").join("bin");
    fs::create_dir_all(&bin).unwrap();

    let fake = bin.join("fake-java");
    fs::write(
        &fake,
        "#!/bin/sh\necho 'fake java launcher' >&2\necho 'openjdk version \"17.0.2\" 2024-01-16' >&2\necho 'OpenJDK 64-Bit Server VM mixed mode' >&2\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let info = test_java(&fake).expect("模拟 java 输出应被正确解析");
    assert_eq!(info.version, "17.0.2");
    assert_eq!(info.major_version, 17);
    assert_eq!(info.java_type, "openjdk");
    assert_eq!(info.arch, mml_sys::get_system_info().system_arch);
}

/// 可执行文件只输出口令、没有版本信息时返回 None
#[cfg(windows)]
#[test]
fn test_java_output_without_version_returns_none() {
    let dir = make_temp_dir("no-version");
    let bin = dir.join("jdk").join("bin");
    fs::create_dir_all(&bin).unwrap();

    let fake = bin.join("fake-no-version.bat");
    fs::write(&fake, "@echo off\r\necho hello world\r\nexit /b 0\r\n").unwrap();

    assert!(test_java(&fake).is_none(), "无版本行不应被识别为 Java");
}
