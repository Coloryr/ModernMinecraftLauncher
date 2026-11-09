use super::*;

/// 目录地址补上清单文件名，末尾斜杠不算内容
#[test]
fn directory_url_gets_manifest_file() {
    assert_eq!(
        normalize_server_pack_url("https://example.com/pack/").unwrap(),
        "https://example.com/pack/server.json"
    );
    assert_eq!(
        normalize_server_pack_url("  https://example.com/pack  ").unwrap(),
        "https://example.com/pack/server.json"
    );
}

/// 已经是清单地址就原样留着，大小写不敏感
#[test]
fn manifest_url_is_kept() {
    assert_eq!(
        normalize_server_pack_url("https://example.com/pack/server.json").unwrap(),
        "https://example.com/pack/server.json"
    );
    assert_eq!(
        normalize_server_pack_url("https://example.com/pack/SERVER.JSON").unwrap(),
        "https://example.com/pack/SERVER.JSON"
    );
}

#[test]
fn empty_url_is_rejected() {
    assert!(normalize_server_pack_url("   ").is_err());
}

/// 实例里存的是发布目录：启动同步时自己拼 `server.json`
#[test]
fn instance_keeps_publish_directory() {
    assert_eq!(
        server_pack_base_url("https://example.com/pack/server.json"),
        "https://example.com/pack"
    );
    assert_eq!(
        server_pack_base_url("https://example.com/pack"),
        "https://example.com/pack"
    );
}
