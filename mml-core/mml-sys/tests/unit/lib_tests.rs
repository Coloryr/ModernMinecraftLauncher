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

