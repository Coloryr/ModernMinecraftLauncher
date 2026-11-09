use super::*;

/// 带纤细皮肤与披风的 textures 属性
const SLIM: &str = "eyJ0aW1lc3RhbXAiOjE2MDAwMDAwMDAwMDAsInByb2ZpbGVJZCI6IjA2OWE3OWY0NDRlOTQ3MjZhNWJlZmNhOTBlMzhhYWY1IiwicHJvZmlsZU5hbWUiOiJOb3RjaCIsInRleHR1cmVzIjp7IlNLSU4iOnsidXJsIjoiaHR0cDovL3RleHR1cmVzLm1pbmVjcmFmdC5uZXQvdGV4dHVyZS9hYmMiLCJtZXRhZGF0YSI6eyJtb2RlbCI6InNsaW0ifX0sIkNBUEUiOnsidXJsIjoiaHR0cDovL3RleHR1cmVzLm1pbmVjcmFmdC5uZXQvdGV4dHVyZS9jYXBlIn19fQ==";
/// 只有 timestamp 等字段、没有任何纹理的 textures 属性
const EMPTY: &str = "eyJ0aW1lc3RhbXAiOjE2MDAwMDAwMDAwMDAsInByb2ZpbGVJZCI6IjA2OWE3OWY0NDRlOTQ3MjZhNWJlZmNhOTBlMzhhYWY1IiwicHJvZmlsZU5hbWUiOiJOb3RjaCIsInRleHR1cmVzIjp7fX0=";

/// 应解析出皮肤与披风地址，皮肤带 slim 标记
#[test]
fn test_parse_textures_slim() {
    let obj = parse_textures(SLIM).expect("应能解析");

    let skin = obj.textures.skin.expect("应有皮肤");
    assert_eq!(skin.url, "http://textures.minecraft.net/texture/abc");
    assert_eq!(skin.metadata.expect("应有元数据").model, "slim");

    let cape = obj.textures.cape.expect("应有披风");
    assert_eq!(cape.url, "http://textures.minecraft.net/texture/cape");
}

/// 没有纹理时皮肤与披风都应为 `None`
#[test]
fn test_parse_textures_empty() {
    let obj = parse_textures(EMPTY).expect("应能解析");

    assert!(obj.textures.skin.is_none());
    assert!(obj.textures.cape.is_none());
}

/// 非法 base64 / 非法 JSON 应返回 `None`
#[test]
fn test_parse_textures_invalid() {
    assert!(parse_textures("!!!").is_none());
    assert!(parse_textures("aGVsbG8=").is_none());
}
