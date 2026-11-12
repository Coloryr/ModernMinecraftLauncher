//! 权限
//!
//! Unix 下给文件加可执行位，Java 运行文件单独处理。

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::io;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::{Command, Stdio};

/// 提升权限
///
/// - `path`: 要加上可执行位的文件路径
///
/// # 返回值
///
/// 命令执行成功返回 `Ok(())`，失败时返回相应错误
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn chmod(path: &str) -> io::Result<()> {
    let mut child = Command::new("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        writeln!(stdin, "chmod a+x {}", path)?;
        writeln!(stdin, "exit")?;
    }

    child.wait()?;
    Ok(())
}

/// 提升 Java 文件夹权限
///
/// - `path`: Java 可执行文件路径，按它推出要放权的目录
///
/// # 返回值
///
/// 命令执行成功返回 `Ok(())`，失败时返回相应错误
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn per_java_chmod(path: &str) -> io::Result<()> {
    let mut child = Command::new("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let info = Path::new(path);
    if let (Some(parent), Some(grandparent)) =
        (info.parent(), info.parent().and_then(|p| p.parent()))
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            writeln!(stdin, "chmod a+x {}/*", parent.display())?;
            writeln!(stdin, "chmod a+x {}/lib/*", grandparent.display())?;
            writeln!(stdin, "exit")?;
        }
    }

    child.wait()?;
    Ok(())
}
