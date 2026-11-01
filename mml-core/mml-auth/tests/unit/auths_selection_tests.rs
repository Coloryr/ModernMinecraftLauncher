use super::*;

#[test]
fn lobby_lease_blocks_switch_delete_and_clear_until_dropped() {
    let account = LoginObj {
        uuid: "lobby-player".into(),
        auth_type: AuthType::OAuth,
        ..Default::default()
    };
    let key = account.get_key();
    *CURRENT_USER.write().unwrap() = Some(key.clone());
    AUTHS.write().unwrap().insert(key.clone(), account.clone());
    let lease = lock_current().unwrap();
    assert!(current_is_locked());
    assert!(lock_current().is_none());
    assert!(!set_current(None));
    assert!(!set_current(Some(UserKeyObj {
        uuid: "another-player".into(),
        auth_type: AuthType::OAuth
    })));
    assert!(!account.delete());
    clear_auths();
    assert_eq!(get_current(), Some(key.clone()));
    assert!(get(&key.uuid, key.auth_type).is_some());
    drop(lease);
    assert!(!current_is_locked());
    assert!(lock_current().is_some());
    *CURRENT_USER.write().unwrap() = None;
    AUTHS.write().unwrap().clear();
}

