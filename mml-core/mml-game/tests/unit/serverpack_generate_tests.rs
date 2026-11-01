use super::*;
use crate::serverpack::serverpack_obj::ServerArchiveItemObj;

/// 一个带版本与加载器的实例
fn game() -> InstanceSettingObj {
    InstanceSettingObj {
        name: "测试".into(),
        version: "1.21.1".into(),
        loader: LoaderType::Forge,
        loader_version: Some("52.0.1".into()),
        ..Default::default()
    }
}

#[test]
fn archive_name_appends_zip() {
    assert_eq!(archive_name("config"), "config.zip");
    assert_eq!(archive_name("kubejs"), "kubejs.zip");
}

#[test]
fn server_pack_carries_instance_metadata() {
    let pack = build_server_pack(
        &game(),
        "服务器",
        "1.0.0",
        "说明",
        vec![ServerItemObj {
            file: "mods/a.jar".into(),
            pid: Some("123".into()),
            fid: Some("456".into()),
            sha1: Some("abc".into()),
            ..Default::default()
        }],
        vec![ServerArchiveItemObj {
            file: archive_name("config"),
            dir: "config".into(),
            delete_old: true,
            url: "https://example.invalid/config.zip".into(),
            ..Default::default()
        }],
    );

    assert_eq!(pack.name, "服务器");
    assert_eq!(pack.version, "1.21.1");
    assert_eq!(pack.loader, LoaderType::Forge);
    assert_eq!(pack.loader_version.as_deref(), Some("52.0.1"));
    assert_eq!(pack.pack_version, "1.0.0");
    assert_eq!(pack.text, "说明");
    assert_eq!(pack.online_list.len(), 1);
    assert_eq!(pack.archive_list.len(), 1);
}

#[test]
fn server_pack_survives_json_roundtrip() {
    let pack = build_server_pack(
        &game(),
        "服务器",
        "1.0.0",
        "",
        vec![ServerItemObj {
            file: "mods/a.jar".into(),
            pid: Some("123".into()),
            fid: Some("456".into()),
            url: Some("https://example.invalid/a.jar".into()),
            sha1: Some("abc".into()),
            ..Default::default()
        }],
        vec![ServerArchiveItemObj {
            file: "config.zip".into(),
            dir: "config".into(),
            delete_old: true,
            url: "https://example.invalid/config.zip".into(),
            sha1: Some("def".into()),
            ..Default::default()
        }],
    );

    let text = serde_json::to_string(&pack).unwrap();
    // 客户端按大驼峰键解析，键名写错这条就会断
    assert!(text.contains("\"Files\""), "{text}");
    assert!(text.contains("\"Archives\""), "{text}");
    assert!(text.contains("\"PackVersion\""), "{text}");
    let back: ServerPackObj = serde_json::from_str(&text).unwrap();
    assert_eq!(back.online_list[0].file, "mods/a.jar");
    assert_eq!(back.archive_list[0].dir, "config");
    assert!(back.archive_list[0].delete_old);
}

#[test]
fn hmcl_addons_cover_supported_loaders() {
    let mut item = game();
    let addons = hmcl_addons(&item).unwrap();
    assert_eq!(addons.len(), 2);
    assert_eq!(
        (addons[0].id.as_str(), addons[0].version.as_str()),
        ("game", "1.21.1")
    );
    assert_eq!(
        (addons[1].id.as_str(), addons[1].version.as_str()),
        ("forge", "52.0.1")
    );

    for (loader, key) in [
        (LoaderType::NeoForge, "neoforge"),
        (LoaderType::Fabric, "fabric"),
        (LoaderType::Quilt, "quilt"),
    ] {
        item.loader = loader;
        assert_eq!(hmcl_addons(&item).unwrap()[1].id, key);
    }
}

#[test]
fn hmcl_addons_drop_unsupported_loaders() {
    let mut item = game();
    for loader in [
        LoaderType::OptiFine,
        LoaderType::LiteLoader,
        LoaderType::Cleanroom,
        LoaderType::Custom,
    ] {
        item.loader = loader;
        assert!(hmcl_addons(&item).is_none(), "{loader:?}");
    }
    // 原版只要游戏版本那一条
    item.loader = LoaderType::Normal;
    let addons = hmcl_addons(&item).unwrap();
    assert_eq!(addons.len(), 1);
    // 有加载器却没写版本，拼不出组件
    item.loader = LoaderType::Forge;
    item.loader_version = None;
    assert!(hmcl_addons(&item).is_none());
}

#[test]
fn hmcl_pack_keeps_file_api_and_files() {
    let pack = build_hmcl_pack(
        "服务器",
        "1.0.0",
        "说明",
        "https://example.invalid/pack",
        hmcl_addons(&game()).unwrap(),
        vec![HMCLServerFileObj {
            path: "mods/a.jar".into(),
            hash: "abc".into(),
        }],
    );
    assert_eq!(pack.name, "服务器");
    assert_eq!(pack.file_api, "https://example.invalid/pack");
    assert_eq!(pack.files[0].path, "mods/a.jar");
    assert_eq!(pack.addons.len(), 2);
}

