use super::*;

#[test]
fn sync_blocks_launch_import_and_second_sync_until_released() {
    let gate = Gate::default();
    let uuid = Uuid::new_v4();
    let sync = gate.sync(uuid).unwrap();
    assert!(gate.launch(uuid).is_err());
    assert!(gate.sync(uuid).is_err());
    assert!(gate.import().is_err());
    assert!(gate.launch(Uuid::new_v4()).is_ok());
    drop(sync);
    let launch = gate.launch(uuid).unwrap();
    assert!(gate.sync(uuid).is_err());
    drop(launch);
    let import = gate.import().unwrap();
    assert!(gate.sync(uuid).is_err());
    drop(import);
    assert!(gate.sync(uuid).is_ok());
}

#[test]
fn import_and_launch_reservations_are_exclusive() {
    let gate = Gate::default();
    let uuid = Uuid::new_v4();
    let launch = gate.launch(uuid).unwrap();
    assert!(gate.launch(uuid).is_err());
    assert!(gate.import().is_err());
    drop(launch);
    let import = gate.import().unwrap();
    assert!(gate.launch(uuid).is_err());
    assert!(gate.import().is_err());
    drop(import);
    assert!(gate.launch(uuid).is_ok());
}

#[test]
fn started_hides_starting_but_keeps_file_gate() {
    let gate = Gate::default();
    let uuid = Uuid::new_v4();
    let launch = gate.launch(uuid).unwrap();
    assert!(gate.starting(uuid), "进程没起来时应显示启动中");
    assert!(gate.launching(uuid), "启动线程还在跑时文件占用应保留");

    gate.mark_started(uuid);
    assert!(!gate.starting(uuid), "进程起来后界面不应再显示启动中");
    assert!(gate.launching(uuid), "启动后命令还没跑完时文件占用应保留");
    assert!(gate.launch(uuid).is_err(), "启动线程没结束前不能重复启动");

    drop(launch);
    assert!(!gate.launching(uuid));
    assert!(!gate.starting(uuid));
}

#[test]
fn successful_import_rejects_launches_until_restart() {
    let gate = Gate::default();
    let mut import = gate.import().unwrap();
    import.commit();
    drop(import);
    std::thread::scope(|scope| {
        scope
            .spawn(|| assert!(gate.launch(Uuid::new_v4()).is_err()))
            .join()
            .unwrap();
    });
    assert!(gate.import().is_err());
}

