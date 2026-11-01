use super::*;

#[test]
fn queued_snapshot_must_not_undo_file_replacement() {
    SEM.get_or_init(|| Arc::new(Semaphore::new(0)));
    let dir = mml_testutil::temp_path(format!("replacement-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("game.json");
    let other = dir.join("accounts.json");
    replace_before_restart(|| {
        save(Uuid::new_v4(), &"stale", &path);
        save(Uuid::new_v4(), &"account update", &other);
        std::fs::write(&path, "\"imported\"").unwrap();
        Ok::<_, ()>(ReplacedFiles {
            files: vec![path.canonicalize().unwrap()],
            value: (),
        })
    })
    .unwrap();
    save(Uuid::new_v4(), &"late stale snapshot", &path);
    save_now();
    let actual = std::fs::read_to_string(&path).unwrap();
    assert_eq!(actual, "\"imported\"");
    assert_eq!(
        std::fs::read_to_string(&other).unwrap(),
        "\"account update\""
    );

    let result = replace_before_restart::<(), _>(|| {
        save(Uuid::new_v4(), &"preserved after rollback", &other);
        Err("replacement failed")
    });
    assert!(result.is_err());
    save_now();
    assert_eq!(
        std::fs::read_to_string(&other).unwrap(),
        "\"preserved after rollback\""
    );
    std::fs::remove_dir_all(dir).unwrap();
}

