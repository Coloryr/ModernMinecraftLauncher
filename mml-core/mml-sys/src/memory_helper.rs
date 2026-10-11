//! 物理内存查询

/// 获取内存大小
///
/// # 返回值
///
/// 返回物理内存总量，单位 MiB；查询失败时返回 `u64::MAX`
#[inline(always)]
pub fn get_memory_size() -> u64 {
    get_memory_size_inner()
}

/// 获取剩余内存大小
///
/// # 返回值
///
/// 返回可用物理内存，单位 MiB；查询失败时返回 `u64::MAX`
#[inline(always)]
pub fn get_memory_free() -> u64 {
    get_memory_free_inner()
}

/// Windows 实现：物理内存总量，单位 MiB
#[cfg(target_os = "windows")]
fn get_memory_size_inner() -> u64 {
    memory_status()
        .map(|ex| ex.ullTotalPhys / 1024 / 1024)
        .unwrap_or(u64::MAX)
}

/// Windows 实现：可用物理内存，单位 MiB
#[cfg(target_os = "windows")]
fn get_memory_free_inner() -> u64 {
    memory_status()
        .map(|ex| ex.ullAvailPhys / 1024 / 1024)
        .unwrap_or(u64::MAX)
}

/// 查询一次内存状态
///
/// `dwLength` 必须先填成结构体大小：`MEMORYSTATUSEX::default()` 给的是 0，
/// 而 `GlobalMemoryStatusEx` 拿它做版本校验，为 0 会直接失败，总量与可用量就只能拿到 `u64::MAX`。
#[cfg(target_os = "windows")]
fn memory_status() -> Option<windows::Win32::System::SystemInformation::MEMORYSTATUSEX> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut ex = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };

    unsafe { GlobalMemoryStatusEx(&mut ex) }.ok().map(|_| ex)
}

/// Linux 实现：读 /proc/meminfo 的 MemTotal，单位 MiB
#[cfg(target_os = "linux")]
fn get_memory_size_inner() -> u64 {
    use crate::path_helper;
    use std::path::Path;

    let path = Path::new("/proc/meminfo");
    if path.exists() {
        if let Ok(data) = path_helper::read_text(path) {
            let datas = data.lines();
            for item in datas {
                if item.starts_with("MemTotal:") {
                    let parts: Vec<&str> = item
                        .split_whitespace()
                        .filter(|item| !item.is_empty())
                        .collect();
                    if parts.len() >= 2
                        && let Ok(data) = parts[1].parse::<u64>()
                    {
                        return data / 1024;
                    }
                }
            }
        }

        u64::MAX
    } else {
        u64::MAX
    }
}

/// Linux 实现：读 /proc/meminfo 的 MemFree，单位 MiB
#[cfg(target_os = "linux")]
fn get_memory_free_inner() -> u64 {
    use crate::path_helper;
    use std::path::Path;

    let path = Path::new("/proc/meminfo");
    if path.exists() {
        if let Ok(data) = path_helper::read_text(path) {
            let datas = data.lines();
            for item in datas {
                if item.starts_with("MemFree:") {
                    let parts: Vec<&str> = item
                        .split_whitespace()
                        .filter(|item| !item.is_empty())
                        .collect();
                    if parts.len() >= 2
                        && let Ok(data) = parts[1].parse::<u64>()
                    {
                        return data / 1024;
                    }
                }
            }
        }

        u64::MAX
    } else {
        u64::MAX
    }
}

/// macOS 实现：读 sysctl hw.memsize，单位 MiB
#[cfg(target_os = "macos")]
fn get_memory_size_inner() -> u64 {
    use crate::process_helper;

    let res = process_utils::run_command_arg("sysctl", &["hw.memsize"]);
    if res.is_err() {
        return u64::MAX;
    }

    let res = res.unwrap();
    for item in res {
        if item.starts_with("hw.memsize:") {
            let parts: Vec<&str> = item
                .split_whitespace()
                .filter(|item| !item.is_empty())
                .collect();
            if parts.len() >= 2
                && let Ok(data) = parts[1].parse::<u64>()
            {
                return data / 1024 / 1024;
            }
        }
    }

    return u64::MAX;
}

/// macOS 实现：读 vm_stat 的 Pages free 与页大小，单位 MiB
#[cfg(target_os = "macos")]
fn get_memory_free_inner() -> u64 {
    use crate::process_helper;

    let res = process_utils::run_command("vm_stat");
    if res.is_err() {
        return u64::MAX;
    }

    let res = res.unwrap();
    let mut free_pages = 0u64;
    let mut page_size = 4096u64;
    for item in res {
        if item.starts_with("Pages free:") {
            let parts: Vec<&str> = item
                .split_whitespace()
                .filter(|item| !item.is_empty())
                .collect();
            if parts.len() >= 3
                && let Ok(data) = parts[2].trim_end_matches('.').parse::<u64>()
            {
                free_pages = data;
            }
        } else if item.starts_with("page size of") {
            let parts: Vec<&str> = item
                .split_whitespace()
                .filter(|item| !item.is_empty())
                .collect();
            if parts.len() >= 4
                && let Ok(data) = parts[3].parse::<u64>()
            {
                page_size = data;
            }
        }
    }

    if free_pages > 0 {
        free_pages * page_size / 1024 / 1024
    } else {
        u64::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 获取内存大小：必须是真实数值，**不能是哨兵值**
    ///
    /// 这里刻意断言 `!= u64::MAX`：以前只断言 `!= 0`，而哨兵值也能通过 ——
    /// Windows 上 `dwLength` 没填导致 `GlobalMemoryStatusEx` 次次失败的问题因此长期没被发现。
    #[test]
    fn test_get_memory_size() {
        let total = get_memory_size();
        assert_ne!(total, u64::MAX, "内存总量查询失败（返回了哨兵值）");
        // 现代机器至少 256 MiB 内存
        assert!(total >= 256, "内存总量过小: {total} MiB");
    }

    /// 剩余内存：必须是真实数值，且不超过总量
    #[test]
    fn test_get_memory_free() {
        let total = get_memory_size();
        let free = get_memory_free();
        assert_ne!(free, u64::MAX, "可用内存查询失败（返回了哨兵值）");
        assert!(free <= total, "剩余内存 {free} 不应大于总量 {total}");
    }
}
