use super::*;

/// 所有内置配置 UUID 都应被 check_uuid 识别
#[test]
fn check_builtin_uuids() {
    for uuid in UUIDS.iter() {
        assert!(check_uuid(*uuid), "内置 UUID {uuid} 应被识别");
    }
}

/// 随机 UUID 不应被识别为配置 UUID
#[test]
fn check_random_uuid() {
    assert!(!check_uuid(Uuid::new_v4()));
    assert!(!check_uuid(Uuid::nil()));
}

/// 内置 UUID 互不重复
#[test]
fn builtin_uuids_unique() {
    let mut list = UUIDS.clone();
    list.sort();
    let before = list.len();
    list.dedup();
    assert_eq!(list.len(), before, "内置 UUID 不应重复");
    assert_eq!(before, UUIDS.len());
}

/// mix_uuid 基于 UUIDv5，同参数结果确定，不同参数结果不同
#[test]
fn mix_uuid_deterministic() {
    let a = mix_uuid(CONFIG_UUID, AUTH_UUID);
    let b = mix_uuid(CONFIG_UUID, AUTH_UUID);
    assert_eq!(a, b, "同参数混合结果应一致");
    assert_ne!(a, mix_uuid(AUTH_UUID, CONFIG_UUID), "参数顺序影响结果");
    assert_ne!(a, CONFIG_UUID);
    assert_ne!(a, AUTH_UUID);
    assert_eq!(a.get_version_num(), 5, "混合结果应是 UUIDv5");
}

