use super::*;

/// 不存在实例目录的路径：下载任务按它判断，必然没有
const NO_DIR: &str = "target/temp/not-an-instance";

#[test]
fn idle_instance_has_no_busy_reason() {
    let id = Uuid::new_v4();
    assert!(busy_of(&id, Path::new(NO_DIR)).is_none());
}

#[test]
fn task_makes_instance_busy() {
    let id = Uuid::new_v4();
    let guard = crate::windows::instance_task::begin(&id.to_string()).expect("占位");
    assert_eq!(busy_of(&id, Path::new(NO_DIR)), Some(Busy::Task));
    drop(guard);
    assert!(busy_of(&id, Path::new(NO_DIR)).is_none());
}

/// 启动预约期间算忙，删除 / 改名会被拦住；预约释放后恢复
#[test]
fn launching_makes_instance_busy() {
    let id = Uuid::new_v4();
    let guard = crate::windows::main::launch_gate::begin_launch(id).expect("预约启动");
    assert_eq!(busy_of(&id, Path::new(NO_DIR)), Some(Busy::Task));
    drop(guard);
    assert!(busy_of(&id, Path::new(NO_DIR)).is_none());
}

