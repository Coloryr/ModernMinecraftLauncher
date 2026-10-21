//! 验证：`tokio::sync::watch` 的唤醒到底能不能跨 runtime
//!
//! 背景：怀疑"请求在 runtime A 等待、`abort_all()` 在 runtime B 广播"会导致
//! 唤醒失效。但 `tokio::sync` 的通道 waker 是普通 `Waker`，理论上与 runtime 无关，
//! 本测试把三种情形都跑一遍对比。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 情形 1：发送端与接收端在同一个 runtime
#[test]
fn watch_wake_same_runtime() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (tx, mut rx) = tokio::sync::watch::channel(0u64);
    let woke = Arc::new(AtomicBool::new(false));
    let woke_clone = Arc::clone(&woke);

    let receiver = rt.spawn(async move {
        // 等一个"比当前值更新"的值
        let _ = rx.changed().await;
        woke_clone.store(*rx.borrow() == 1, Ordering::SeqCst);
    });

    std::thread::sleep(Duration::from_millis(100));
    tx.send(1).unwrap();
    rt.block_on(receiver).unwrap();
    println!("同 runtime：唤醒={}", woke.load(Ordering::SeqCst));
    assert!(woke.load(Ordering::SeqCst), "同 runtime 内唤醒应生效");
}

/// 情形 2：接收端在 runtime A 里等待，发送端 `send` 在另一个 OS 线程上调用
///
/// 注意：`send` 本身是同步的，不要求调用者在某个 tokio runtime 里——
/// 这正对应实际场景：Tauri 同步命令里调 `abort_all()`。
#[test]
fn watch_wake_across_threads() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (tx, mut rx) = tokio::sync::watch::channel(0u64);
    let woke = Arc::new(AtomicBool::new(false));
    let woke_clone = Arc::clone(&woke);

    // 接收端待在 runtime A 里
    let receiver = rt.spawn(async move {
        let _ = rx.changed().await;
        woke_clone.store(*rx.borrow() == 1, Ordering::SeqCst);
    });

    std::thread::sleep(Duration::from_millis(100));

    // 发送端在裸 OS 线程里调用 send，不在任何 tokio runtime 上下文里
    let handle = std::thread::spawn(move || {
        tx.send(1).unwrap();
    });
    handle.join().unwrap();

    rt.block_on(receiver).unwrap();
    println!(
        "跨线程（裸 OS 线程发送）：唤醒={}",
        woke.load(Ordering::SeqCst)
    );
    assert!(
        woke.load(Ordering::SeqCst),
        "跨线程 send 应该也能唤醒——否则说明我的怀疑成立"
    );
}

/// 情形 3：接收端在 runtime A，发送端在另一个独立 runtime B 里
#[test]
fn watch_wake_across_runtimes() {
    let rt_a = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let rt_b = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (tx, mut rx) = tokio::sync::watch::channel(0u64);
    let woke = Arc::new(AtomicBool::new(false));
    let woke_clone = Arc::clone(&woke);

    let receiver = rt_a.spawn(async move {
        let _ = rx.changed().await;
        woke_clone.store(*rx.borrow() == 1, Ordering::SeqCst);
    });

    std::thread::sleep(Duration::from_millis(100));

    // 在 runtime B 里发送
    rt_b.block_on(async move {
        tx.send(1).unwrap();
    });

    rt_a.block_on(receiver).unwrap();
    println!(
        "跨 runtime（B 里发送）：唤醒={}",
        woke.load(Ordering::SeqCst)
    );
    assert!(
        woke.load(Ordering::SeqCst),
        "跨 runtime send 应该也能唤醒——否则说明我的怀疑成立"
    );
}
