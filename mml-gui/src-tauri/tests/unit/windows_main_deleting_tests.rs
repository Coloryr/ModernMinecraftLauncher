use super::*;

#[test]
fn visible_only_while_guarded() {
    let id = Uuid::new_v4();
    assert!(!is_deleting(&id));
    let guard = begin(id).expect("首次标记");
    assert!(is_deleting(&id));
    drop(guard);
    assert!(!is_deleting(&id));
}

#[test]
fn second_begin_is_rejected() {
    let id = Uuid::new_v4();
    let guard = begin(id).expect("首次标记");
    assert!(begin(id).is_none());
    drop(guard);
    assert!(begin(id).is_some());
}

#[test]
fn other_instances_are_not_affected() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let guard = begin(a).expect("首次标记");
    assert!(!is_deleting(&b));
    drop(guard);
    assert!(!is_deleting(&a));
}

