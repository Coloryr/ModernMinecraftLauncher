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

