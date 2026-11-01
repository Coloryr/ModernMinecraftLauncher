use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// 带参数事件的添加、触发、移除
#[test]
fn test_event_arg_handler() {
    let event: EventArgHandler<i32> = EventArgHandler::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    let id1 = event.add_handler(move |e: &i32| {
        // 收到 0 时不计数
        if *e != 0 {
            c.fetch_add(1, Ordering::SeqCst);
        }
    });
    let c2 = counter.clone();
    let id2 = event.add_handler(move |_| {
        c2.fetch_add(1, Ordering::SeqCst);
    });
    assert_ne!(id1, id2, "每个处理器应有唯一 id");

    event.emit(1);
    assert_eq!(counter.load(Ordering::SeqCst), 2);

    // 移除一个处理器后只触发另一个
    event.remove_handel(id1);
    event.emit(2);
    assert_eq!(counter.load(Ordering::SeqCst), 3);

    // 移除不存在的 id 不应 panic
    event.remove_handel(999);
}

/// 不带参数事件的添加、触发、移除
#[test]
fn test_event_handler() {
    let event = EventHandler::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    let id = event.add_handler(move || {
        c.fetch_add(1, Ordering::SeqCst);
    });

    event.emit();
    event.emit();
    assert_eq!(counter.load(Ordering::SeqCst), 2);

    event.remove_handle(id);
    event.emit();
    assert_eq!(counter.load(Ordering::SeqCst), 2);

    // 移除不存在的 id 不应 panic
    event.remove_handle(12345);
}

/// 多个事件实例互不影响
#[test]
fn test_event_instances_isolated() {
    let e1 = EventHandler::new();
    let e2 = EventHandler::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    e1.add_handler(move || {
        c.fetch_add(1, Ordering::SeqCst);
    });
    e1.emit();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    e2.emit();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

