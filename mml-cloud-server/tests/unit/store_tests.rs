use super::*;

fn root() -> PathBuf {
    mml_testutil::temp_path(format!("cloud-recovery-{}", Uuid::new_v4()))
}

#[test]
fn interrupted_replacement_restores_old_snapshot() {
    let root = root();
    let store = Store::new(&root).unwrap();
    let relative = format!(
        "accounts/{}/{}/worlds/{}",
        sha1(b"client"),
        Uuid::new_v4(),
        sha1(b"world")
    );
    let target = root.join(&relative);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let stage = store.staging().unwrap();
    fs::create_dir_all(stage.0.join("old/files")).unwrap();
    fs::write(stage.0.join("old/files/level.dat"), b"old").unwrap();
    fs::write(
        stage.0.join("transaction.json"),
        serde_json::to_vec(&Transaction { target: relative }).unwrap(),
    )
    .unwrap();
    let stage_path = stage.0.clone();
    drop(stage);
    assert!(
        stage_path.exists(),
        "retain the only old snapshot until recovery"
    );
    drop(store);
    let _store = Store::new(&root).unwrap();
    assert_eq!(fs::read(target.join("files/level.dat")).unwrap(), b"old");
    assert!(!stage_path.exists());
}

#[test]
fn committed_replacement_keeps_new_snapshot() {
    let root = root();
    let store = Store::new(&root).unwrap();
    let relative = format!("accounts/{}/{}/config.zip", sha1(b"client"), Uuid::new_v4());
    let target = root.join(&relative);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"new").unwrap();
    let stage = store.staging().unwrap();
    fs::write(stage.0.join("old"), b"old").unwrap();
    fs::write(
        stage.0.join("transaction.json"),
        serde_json::to_vec(&Transaction { target: relative }).unwrap(),
    )
    .unwrap();
    let stage_path = stage.0.clone();
    drop(stage);
    drop(store);
    let _store = Store::new(&root).unwrap();
    assert_eq!(fs::read(target).unwrap(), b"new");
    assert!(!stage_path.exists());
}

#[test]
fn metadata_recovery_never_changes_world_content() {
    let root = root();
    let store = Store::new(&root).unwrap();
    let instance = root
        .join("accounts")
        .join(sha1(b"client"))
        .join(Uuid::new_v4().to_string());
    let world = instance.join("worlds").join(sha1(b"world")).join("files");
    fs::create_dir_all(&world).unwrap();
    fs::write(instance.join("instance.json.old"), br#"{"Enabled":true}"#).unwrap();
    fs::write(instance.join("instance.json.new"), b"incomplete").unwrap();
    fs::write(world.join("instance.json.old"), b"world old").unwrap();
    fs::write(world.join("instance.json.new"), b"world new").unwrap();
    drop(store);
    let _store = Store::new(&root).unwrap();
    let meta: Instance = load(&instance.join("instance.json")).unwrap();
    assert!(meta.enabled);
    assert!(!instance.join("instance.json.new").exists());
    assert_eq!(
        fs::read(world.join("instance.json.old")).unwrap(),
        b"world old"
    );
    assert_eq!(
        fs::read(world.join("instance.json.new")).unwrap(),
        b"world new"
    );
}

#[test]
fn recovery_marker_only_accepts_snapshot_targets() {
    let prefix = format!("accounts/{}/{}", sha1(b"client"), Uuid::new_v4());
    for value in [
        "../outside".to_string(),
        format!("{prefix}/instance.json"),
        format!("{prefix}/worlds/{}/files", sha1(b"world")),
        "accounts/config.zip".into(),
    ] {
        assert!(Store::transaction_path(&value).is_err(), "{value}");
    }
}

