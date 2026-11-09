use super::*;

/// 名字校验：合法字符放行，空名/路径注入/超长拒绝
#[test]
fn skin_id_validation() {
    assert_eq!(skin_id("steve").unwrap(), "custom:steve");
    assert!(skin_id("A-b_9").is_ok());
    assert!(skin_id(&"x".repeat(64)).is_ok());

    assert!(skin_id("").is_err());
    assert!(skin_id("../evil").is_err());
    assert!(skin_id("a b").is_err());
    assert!(skin_id("minecraft:stone").is_err());
    assert!(skin_id(&"x".repeat(65)).is_err());
}
