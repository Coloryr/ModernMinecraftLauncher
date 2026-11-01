use super::*;

/// 能不能装只看 `game` 数组，加载器版本列表非空不代表支持
#[test]
fn support_follows_game_list() {
    let meta = LoaderMeta {
        loaders: vec!["0.16.0".to_string()],
        games: ["1.20.1".to_string()].into_iter().collect(),
    };

    assert!(meta_supports(&meta, "1.20.1"));
    assert!(!meta_supports(&meta, "1.12.2"));
}

/// 版本号按段落比大小，1.12.2 不该被当成 1.20.1 以上
#[test]
fn version_ge_compares_by_segment() {
    assert!(version_ge("1.20.1", "1.20.1"));
    assert!(version_ge("1.21", "1.20.1"));
    assert!(!version_ge("1.12.2", "1.20.1"));
}

