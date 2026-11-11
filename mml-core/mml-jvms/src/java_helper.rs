//! Java 可执行文件探测：运行 `java -version` 解析出版本、发行版与架构

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    thread,
    time::{Duration, Instant},
};

use mml_sys::{ArchEnum, process_helper};

use crate::JavaInfoObj;

/// 从版本字符串提取主版本号
///
/// 传统格式：1.8.0_201 -> 8
/// 新格式：11.0.2 -> 11, 17.0.1 -> 17
///
/// - `version`: 输入版本号
///
/// # 返回值
///
/// 无法解析时返回 0
fn get_major_version(version: &str) -> i32 {
    if version.starts_with("1.") {
        // 传统版本: 1.8.0 -> 8
        version
            .split('.')
            .nth(1)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    } else {
        // 新版本: 11.0.2 -> 11
        version
            .split('.')
            .next()
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    }
}

/// 从 `java -version` 输出与安装路径识别发行商
///
/// 版本行首词只区分 `openjdk` / `java`，真正的发行商写在运行时行里：
/// `OpenJDK Runtime Environment Temurin-...`、`... Zulu...`；Oracle JDK 只写 `Java(TM)`。
/// 路径只作兜底，因为 `C:\Program Files\Java\openjdk\...` 里也可能装的是 Temurin。
///
/// - `output`: `java -version` 的 stdout 与 stderr
/// - `path`: Java 可执行文件路径
/// - `java_type`: 版本行首词，`openjdk` / `java`
///
/// # 返回值
///
/// 发行商名；认不出时回落到 `OpenJDK` / `Java`
fn detect_vendor(output: &str, path: &Path, java_type: &str) -> String {
    const KNOWN: &[(&str, &str)] = &[
        ("zulu", "Zulu"),
        ("azul", "Zulu"),
        ("temurin", "Temurin"),
        ("adoptium", "Temurin"),
        ("adoptopenjdk", "Temurin"),
        ("semeru", "Semeru"),
        ("openj9", "Semeru"),
        ("ibm", "Semeru"),
        ("corretto", "Amazon Corretto"),
        ("microsoft", "Microsoft"),
        ("liberica", "Liberica"),
        ("bellsoft", "Liberica"),
        ("dragonwell", "Dragonwell"),
        ("graalvm", "GraalVM"),
        ("sapmachine", "SapMachine"),
        ("kona", "Tencent Kona"),
        ("bisheng", "Huawei Bisheng"),
        ("jetbrains", "JetBrains Runtime"),
        ("jbr", "JetBrains Runtime"),
        ("oracle", "Oracle"),
    ];

    let lower = output.to_ascii_lowercase();
    let path_lower = path.to_string_lossy().to_ascii_lowercase();

    // 运行时行里的厂商名优先；路径只在输出里认不出时兜底
    for (needle, name) in KNOWN {
        if lower.contains(needle) {
            return (*name).to_string();
        }
    }
    for (needle, name) in KNOWN {
        if path_lower.contains(needle) {
            return (*name).to_string();
        }
    }

    // Oracle JDK 的运行时不写 Oracle，只写 Java(TM) / Java HotSpot
    if lower.contains("java(tm)") || lower.contains("java hotspot") {
        return "Oracle".to_string();
    }
    if lower.contains("openjdk") || java_type.eq_ignore_ascii_case("openjdk") {
        return "OpenJDK".to_string();
    }
    if java_type.eq_ignore_ascii_case("java") {
        return "Java".to_string();
    }

    if java_type.is_empty() {
        "Java".to_string()
    } else {
        java_type.to_string()
    }
}

/// 获取 Java 信息
///
/// - `file`: 需要检测的 Java 可执行文件路径
///
/// # 返回值
///
/// 解析 `java -version` 成功返回 Java 信息，路径无效或输出异常返回 `None`
pub fn test_java<P: AsRef<Path>>(file: P) -> Option<JavaInfoObj> {
    let path = file.as_ref().to_path_buf();

    if !path.exists() || !path.is_file() {
        return None;
    }

    let working_dir = path
        .parent()
        .and_then(|parent| parent.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // 扫描软件偶尔会把刚解压出来的 exe 卡住，别让一次检测把整个安装挂死
    let mut child = match process_helper::hidden_command(&path)
        .arg("-version")
        .current_dir(working_dir)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return None,
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(_) => return None,
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}\n{}", stderr, stdout);

    let mut parsed: Option<(String, String)> = None;
    for line in combined.lines() {
        let line = line.trim();
        if line.contains(" version ") || line.contains("\"") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                parsed = Some((
                    parts[0].to_string(),
                    parts[2].trim_matches('"').to_string(),
                ));
                break;
            }
        }
    }

    let (java_type, version) = parsed?;
    let is64 = combined.contains("64-Bit") || combined.contains("64-bit");
    let arch = if cfg!(target_arch = "aarch64") {
        if is64 {
            ArchEnum::AArch64
        } else {
            ArchEnum::Arm
        }
    } else if is64 {
        ArchEnum::X86_64
    } else {
        ArchEnum::X86
    };

    let major_version = get_major_version(&version);
    let vendor = detect_vendor(&combined, &path, &java_type);

    Some(JavaInfoObj {
        name: format!("{}-{}-{}", &java_type, &version, arch.to_string()),
        path,
        version,
        arch,
        java_type,
        major_version,
        vendor,
    })
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
#[path = "../tests/unit/java_helper_tests.rs"]
mod tests;
