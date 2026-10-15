//! 系统交互模块，封装平台相关的进程 / 路径 / 内存 / 剪贴板等操作
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`clipboard_helper`] | 剪贴板操作 |
//! | [`java_scan_helper`] | Java 运行时扫描 |
//! | [`memory_helper`] | 物理内存查询 |
//! | [`open_helper`] | 浏览器 / 资源管理器 / 默认程序打开 |
//! | [`path_helper`] | 路径与文件操作工具 |
//! | [`process_helper`] | 进程查询与命令执行 |
//! | [`protocol_helper`] | URL 协议注册 |
//! | [`shortcut_helper`] | 快捷方式创建 |
//! | [`text_diff`] | 文本文件的行级差异 |

use core::fmt;
use std::sync::LazyLock;

pub mod clipboard_helper;
pub mod java_scan_helper;
pub mod memory_helper;
pub mod open_helper;
pub mod path_helper;
pub mod process_helper;
pub mod protocol_helper;
pub mod shortcut_helper;
pub mod text_diff;

/// 操作系统类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// 未知操作系统
    None,
    /// Microsoft Windows
    Windows,
    /// Linux 通用发行版
    Linux,
    /// Apple macOS
    MacOS,
    /// Alpine Linux，使用 musl libc
    AlpineLinux,
    /// IBM AIX
    AIX,
    /// Oracle Solaris
    Solaris,
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Os::Windows => write!(f, "Windows"),
            Os::Linux => write!(f, "Linux"),
            Os::MacOS => write!(f, "MacOS"),
            Os::None => write!(f, "Unknown"),
            Os::AlpineLinux => write!(f, "Alpine Linux"),
            Os::AIX => write!(f, "AIX"),
            Os::Solaris => write!(f, "Solaris"),
        }
    }
}

/// CPU 架构枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchEnum {
    /// x86 32 位
    X86,
    /// x86_64 64 位
    X86_64,
    /// ARM 32 位
    Arm,
    /// ARM 64 位 (AArch64)
    AArch64,
    /// 未知架构
    Unknown,
}

impl Default for ArchEnum {
    fn default() -> Self {
        ArchEnum::Unknown
    }
}

impl fmt::Display for ArchEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchEnum::X86 => write!(f, "x86"),
            ArchEnum::X86_64 => write!(f, "x86_64"),
            ArchEnum::Arm => write!(f, "arm"),
            ArchEnum::AArch64 => write!(f, "aarch64"),
            ArchEnum::Unknown => write!(f, "unknown"),
        }
    }
}

/// 系统信息结构体
///
/// 在首次访问时通过 [`get_system_info()`] 惰性初始化并缓存。
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// 操作系统类型
    pub os: Os,
    /// CPU 架构
    pub system_arch: ArchEnum,
    /// 系统名称，完整描述，如 `"windows"`、`"linux"`
    pub system_name: String,
    /// Linux 发行版标识，如 `"ubuntu"`、`"arch"`；非 Linux 为空字符串
    pub distribution: String,
    /// 格式化的系统描述字符串，如 `"Os:Windows Arch:x86_64"`
    pub system: String,
    /// 是否为 ARM 处理器
    pub is_arm: bool,
    /// 是否为 64 位操作系统
    pub is_64_bit: bool,
}

/// 全局系统信息缓存，惰性初始化
static SYSTEM_INFO: LazyLock<SystemInfo> = LazyLock::new(|| SystemInfo::new());

/// 获取系统信息，首次调用时自动检测并缓存
///
/// # 返回值
///
/// 返回缓存系统信息的副本
pub fn get_system_info() -> SystemInfo {
    SYSTEM_INFO.clone()
}

/// 读取 Linux 发行版标识
///
/// 解析 `/etc/os-release` 中的 `ID=` 字段。
fn get_linux_distribution() -> String {
    let content = std::fs::read_to_string("/etc/os-release").ok();
    if content.is_some() {
        let content = content.unwrap();
        for line in content.lines() {
            if line.starts_with("ID=") {
                return line[3..].trim_matches('"').to_string();
            }
        }
    }

    String::new()
}

impl SystemInfo {
    /// 初始化并获取系统信息
    fn new() -> Self {
        let arch = std::env::consts::ARCH;
        let is_arm = arch.starts_with("arm") || arch.starts_with("aarch64");
        let is_64_bit = cfg!(target_pointer_width = "64");

        let system_arch = match (is_64_bit, is_arm) {
            (true, true) => ArchEnum::AArch64,
            (true, false) => ArchEnum::X86_64,
            (false, true) => ArchEnum::Arm,
            (false, false) => ArchEnum::X86,
        };

        let os = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(target_os = "macos") {
            Os::MacOS
        } else {
            Os::None
        };

        let distribution = if os == Os::Linux {
            get_linux_distribution()
        } else {
            String::new()
        };

        let system_name = std::env::consts::OS.to_string();
        let system = format!("Os:{} Arch:{}", os, system_arch);

        Self {
            os,
            system_arch,
            system_name,
            system,
            is_arm,
            distribution,
            is_64_bit,
        }
    }
}

impl fmt::Display for SystemInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.system)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 系统信息与编译目标一致
    #[test]
    fn test_system_info_matches_target() {
        let info = get_system_info();

        // 操作系统
        let expected_os = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(target_os = "macos") {
            Os::MacOS
        } else {
            Os::None
        };
        assert_eq!(info.os, expected_os);

        // 架构
        assert_eq!(info.is_64_bit, cfg!(target_pointer_width = "64"));
        assert_eq!(
            info.is_arm,
            cfg!(target_arch = "arm") || cfg!(target_arch = "aarch64")
        );
        if cfg!(target_pointer_width = "64") {
            assert_eq!(info.system_arch, ArchEnum::X86_64);
        }

        // system_name 与 std::env::consts::OS 一致
        assert_eq!(info.system_name, std::env::consts::OS);

        // 非 Linux 平台发行版为空
        if info.os != Os::Linux {
            assert_eq!(info.distribution, "");
        }
    }

    /// get_system_info 多次调用返回一致缓存
    #[test]
    fn test_system_info_cached() {
        let a = get_system_info();
        let b = get_system_info();
        assert_eq!(a.os, b.os);
        assert_eq!(a.system, b.system);
        assert_eq!(a.system_arch, b.system_arch);
    }

    /// Os / ArchEnum / SystemInfo 的 Display 输出
    #[test]
    fn test_display() {
        assert_eq!(Os::Windows.to_string(), "Windows");
        assert_eq!(Os::Linux.to_string(), "Linux");
        assert_eq!(Os::MacOS.to_string(), "MacOS");
        assert_eq!(Os::None.to_string(), "Unknown");
        assert_eq!(Os::AlpineLinux.to_string(), "Alpine Linux");

        assert_eq!(ArchEnum::X86.to_string(), "x86");
        assert_eq!(ArchEnum::X86_64.to_string(), "x86_64");
        assert_eq!(ArchEnum::Arm.to_string(), "arm");
        assert_eq!(ArchEnum::AArch64.to_string(), "aarch64");
        assert_eq!(ArchEnum::Unknown.to_string(), "unknown");
        // Default 为 Unknown
        assert_eq!(ArchEnum::default(), ArchEnum::Unknown);

        let info = get_system_info();
        let text = info.to_string();
        assert!(text.starts_with("Os:"), "Display 应以 Os: 开头: {text}");
        assert!(text.contains("Arch:"), "Display 应包含 Arch:: {text}");
    }
}
