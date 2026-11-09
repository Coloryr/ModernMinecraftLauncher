use super::*;

/// 造一个只填了作者 ID 的命中项
fn hit(author_id: &str) -> HitObj {
    HitObj {
        author_id: author_id.to_string(),
        ..Default::default()
    }
}

/// 作者 ID 去重、空 ID 跳过
#[test]
fn author_ids_dedups_and_skips_empty() {
    let hits = vec![hit("a"), hit("a"), hit(""), hit("b")];
    assert_eq!(author_ids(&hits), vec!["a".to_string(), "b".to_string()]);
    assert!(author_ids(&[]).is_empty());
}
