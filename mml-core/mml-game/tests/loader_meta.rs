//! 加载器与整合包类型枚举测试：独立 ID 双向转换、Forge JSON 名字构造、下载源判断。
//!
//! 纯离线测试，不依赖全局初始化。

use mml_game::add_game::PackType;
use mml_game::launcher::{ModPackType, get_source_type};
use mml_game::launcher_path::version_path::get_forge_json_name;
use mml_game::loader::LoaderType;

/// 加载器类型：id 与 from_id 双向转换，且 ids() 列表覆盖全部变体
#[test]
fn loader_type_id_round_trip() {
    let ids = LoaderType::ids();
    assert_eq!(ids.len(), 8);

    for id in ids {
        let loader = LoaderType::from_string(id).expect("ids() 里的 ID 必须能解析");
        assert_eq!(loader.to_string(), id, "{id} 应与解析结果一致");
    }

    // 未知 ID 返回 None
    assert!(LoaderType::from_string("unknown").is_none());
    assert!(LoaderType::from_string("").is_none());
}

/// 加载器类型前缀，用于拼接版本目录名
#[test]
fn loader_type_prefixes() {
    // 前缀 = id，保持与版本文件夹命名一致
    for id in LoaderType::ids() {
        let loader = LoaderType::from_string(id).unwrap();
        assert_eq!(loader.to_string(), id);
    }
}

/// 整合包类型：id 与 from_id 双向转换。
#[test]
fn mod_pack_type_id_round_trip() {
    for id in ["curseforge", "modrinth", "mcmod", "serverpack", "none"] {
        let t = ModPackType::from_string(id);
        assert_eq!(t.to_string(), id, "{id} 应与解析结果一致");
    }

    // 未知 ID 回退 None
    assert_eq!(ModPackType::from_string("unknown"), ModPackType::None);

    // 整数序列化顺序，由 serde_repr 决定
    assert_eq!(serde_json::to_value(ModPackType::CurseForge).unwrap(), 0);
    assert_eq!(serde_json::to_value(ModPackType::Modrinth).unwrap(), 1);
    assert_eq!(serde_json::to_value(ModPackType::None).unwrap(), 4);
}

/// 压缩包类型，用于添加实例入口：id 与 from_id 双向转换
#[test]
fn pack_type_id_round_trip() {
    let ids = PackType::ids();
    assert_eq!(ids.len(), 7);

    for id in ids {
        let t = PackType::from_id(id).expect("ids() 里的 ID 必须能解析");
        assert_eq!(t.id(), id, "{id} 应与解析结果一致");
    }

    assert!(PackType::from_id("unknown").is_none());
}

/// 下载源判断：纯数字 pid/fid 视为 CurseForge，否则视为 Modrinth。
#[test]
fn source_type_detection() {
    // CurseForge 的 projectID / fileID 是数字
    assert_eq!(get_source_type("12345", "67890"), ModPackType::CurseForge);
    // Modrinth 的 pid/fid 是 slug 字符串
    assert_eq!(get_source_type("abc", "def"), ModPackType::Modrinth);
    // 数字与非数字混合 → Modrinth
    assert_eq!(get_source_type("12345", "def"), ModPackType::Modrinth);
    assert_eq!(get_source_type("abc", "67890"), ModPackType::Modrinth);
}

/// Forge / NeoForge 版本 JSON 文件名构造，补充分支覆盖
#[test]
fn forge_json_name_variants() {
    // 带 hotfix 的 Forge 版本号
    assert_eq!(
        get_forge_json_name("1.16.5", "1.16.5-36.2.39", false, false),
        "forge-1.16.5-36.2.39.json"
    );

    // NeoForge 新版 1.20.2+：版本号独立，不带 mc 前缀
    assert_eq!(
        get_forge_json_name("1.21.1", "21.1.77", true, false),
        "neoforge-21.1.77.json"
    );
    assert_eq!(
        get_forge_json_name("1.21.1", "21.1.77", true, true),
        "neoforge-21.1.77-install.json"
    );

    // NeoForge 老版 1.20.2 之前：回退 forge-{mc}-{version}，版本号不带 mc 前缀
    assert_eq!(
        get_forge_json_name("1.20.1", "47.1.3", true, false),
        "forge-1.20.1-47.1.3.json"
    );
    assert_eq!(
        get_forge_json_name("1.20.1", "47.1.3", true, true),
        "forge-1.20.1-47.1.3-install.json"
    );

    // 普通 Forge 的 install 变体
    assert_eq!(
        get_forge_json_name("1.12.2", "1.12.2-14.23.5.2860", false, true),
        "forge-1.12.2-14.23.5.2860-install.json"
    );
}
