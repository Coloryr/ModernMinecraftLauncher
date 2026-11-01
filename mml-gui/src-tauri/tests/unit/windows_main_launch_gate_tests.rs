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

