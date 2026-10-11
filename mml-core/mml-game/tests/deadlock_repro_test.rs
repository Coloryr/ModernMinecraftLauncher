//! 复现并确认 `add_optifine` / `add_liteloader` 的**自死锁**
//!
//! 代码事实见 mml-core/mml-game/src/launcher_path/version_path.rs：
//!
//! ```text
//! add_liteloader:
//!     let mut list = LITE_LOADER.write().unwrap();   // ① 持写锁
//!     ...
//!     save_liteloader();                             // ② 内部 LITE_LOADER.read()
//!
//! save_liteloader:
//!     let list = LITE_LOADER.read().unwrap();        // ← 同线程再取读锁 → 永久阻塞
//! ```
//!
//! `std::sync::RwLock` 不可重入/不可升级，所以 ② 必然死锁。
//! 本测试用**同样的锁使用模式**复现该阻塞，用来证明：
//! - 现写法在写锁内调 save 会卡死；
//! - 收窄作用域、先释放写锁再 save 不会卡死。
//!
//! 这样修复就有据可依，而不是"看起来对"。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// 模拟 save_liteloader：内部取**读锁**
fn save_like(list: &RwLock<HashMap<String, Arc<String>>>) -> usize {
    let guard = list.read().unwrap();
    guard.len()
}

/// 现写法：**持有写锁**时调用 save，会死锁
fn add_current_style(list: &RwLock<HashMap<String, Arc<String>>>, item: String) -> usize {
    let mut guard = list.write().unwrap();
    guard.insert(item.clone(), Arc::new(item));
    // 写锁仍在作用域内，此时 save 去取读锁 → 同线程自死锁
    save_like(list)
}

/// 修复写法：**先释放写锁**再 save
fn add_fixed_style(list: &RwLock<HashMap<String, Arc<String>>>, item: String) -> usize {
    {
        let mut guard = list.write().unwrap();
        guard.insert(item.clone(), Arc::new(item));
    } // ← 写锁在此释放
    save_like(list)
}

/// 在一个独立线程里跑 f，看它能否在 timeout 内完成
fn run_with_timeout<F>(f: F, timeout: Duration) -> bool
where
    F: FnOnce() + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    rx.recv_timeout(timeout).is_ok()
}

/// 断言现写法在超时内完不成，即确实自死锁
#[test]
fn current_style_deadlocks() {
    let list = Arc::new(RwLock::new(HashMap::new()));
    let l = Arc::clone(&list);
    let done = run_with_timeout(
        move || {
            add_current_style(&l, "1.12".to_string());
        },
        Duration::from_millis(800),
    );
    println!("现写法（写锁内调 save）：{}", if done { "正常完成" } else { "卡死（符合预期）" });
    assert!(
        !done,
        "现写法竟然没死锁？说明我对 RwLock 语义的判断有误，需重新分析"
    );
}

/// 断言收窄写锁作用域后不再死锁
#[test]
fn fixed_style_does_not_deadlock() {
    let list = Arc::new(RwLock::new(HashMap::new()));
    let l = Arc::clone(&list);
    let done = run_with_timeout(
        move || {
            add_fixed_style(&l, "1.12".to_string());
        },
        Duration::from_millis(800),
    );
    println!(
        "修复写法（先释放写锁再 save）：{}",
        if done { "正常完成（符合预期）" } else { "卡死" }
    );
    assert!(done, "修复写法不应死锁");
}
