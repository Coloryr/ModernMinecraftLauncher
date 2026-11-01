//! Java 可执行文件探测：运行 `java -version` 解析出版本、发行版与架构

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use mml_sys::ArchEnum;

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

    let output = match Command::new(&path)
        .arg("-version")
        .current_dir(working_dir)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
    {
        Ok(output) => output,
        Err(_) => return None,
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}\n{}", stderr, stdout);

    for line in combined.lines() {
        let line = line.trim();
        if line.contains(" version ") || line.contains("\"") {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() >= 3 {
                let java_type = parts[0].to_string();
                let version = parts[2].trim_matches('"').to_string();
                let is64 = combined.contains("64-Bit") || combined.contains("64-bit");

                let arch = if cfg!(target_arch = "aarch64") {
                    if is64 {
                        ArchEnum::AArch64
                    } else {
                        ArchEnum::Arm
                    }
                } else {
                    if is64 {
                        ArchEnum::X86_64
                    } else {
                        ArchEnum::X86
                    }
                };

                let major_version = get_major_version(&version);

                return Some(JavaInfoObj {
                    name: format!("{}-{}-{}", &java_type, &version, arch.to_string()),
                    path,
                    version,
                    arch,
                    java_type,
                    major_version,
                });
            }
        }
    }

    None
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
#[path = "../tests/unit/java_helper_tests.rs"]
mod tests;
