use super::*;

#[test]
fn busy_only_while_guarded() {
    let id = Uuid::new_v4();
    assert!(!is_busy(&id));
    let guard = begin(&id.to_string()).expect("占位");
    assert!(is_busy(&id));
    drop(guard);
    assert!(!is_busy(&id));
}

#[test]
fn two_tasks_on_one_instance_need_two_drops() {
    let id = Uuid::new_v4();
    let first = begin(&id.to_string()).expect("占位");
    let second = begin(&id.to_string()).expect("占位");
    drop(first);
    assert!(is_busy(&id));
    drop(second);
    assert!(!is_busy(&id));
}

#[test]
fn deleting_instance_refuses_new_task() {
    let id = Uuid::new_v4();
    let deleting = crate::windows::main::deleting::begin(id).expect("标记删除中");
    assert!(begin(&id.to_string()).is_err());
    drop(deleting);
    assert!(begin(&id.to_string()).is_ok());
}

#[test]
fn invalid_uuid_is_an_error() {
    assert!(begin("not-a-uuid").is_err());
}

