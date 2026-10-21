//! CurseForge API 的离线测试。
//!
//! 重点覆盖：
//! - 各数据结构 `*_obj` 对真实 CurseForge API 响应结构的反序列化
//! - API Key 的设置与读取行为，使用假密钥 `test-key`，绝不使用真实密钥
//!
//! 不联网：所有样例 JSON 均为内嵌字符串，结构与 CurseForge REST API v1
//! 的真实响应一致。

use mml_names::i18_items::error_type::ErrorType;
use mml_net::curseforge_api::{
    CLASS_MOD, CLASS_MODPACK, GAME_ID, categories_obj::CurseForgeCategoriesObj,
    file_obj::CurseFogreFilePageObj, file_obj::CurseForgeFileDataObj, file_obj::CurseForgeFileObj,
    get_key, list_obj::CurseForgeListObj, list_obj::CurseForgeListPageObj, set_key,
    version_obj::CurseForgeVersionObj, version_obj::CurseForgeVersionTypeObj,
};

/// CurseForge 搜索接口 `/mods/search` 的真实响应结构
const SEARCH_JSON: &str = r#"{
    "data": [
        {
            "id": 238222,
            "gameId": 432,
            "classId": 6,
            "slug": "jei",
            "name": "Just Enough Items (JEI)",
            "links": {
                "websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/jei",
                "wikiUrl": "https://github.com/mezz/JustEnoughItems/wiki",
                "issuesUrl": "https://github.com/mezz/JustEnoughItems/issues"
            },
            "summary": "Item and Recipe viewing mod for Minecraft",
            "status": 4,
            "downloadCount": 300000000,
            "isFeatured": true,
            "primaryCategoryId": 423,
            "categories": [
                {
                    "id": 423,
                    "gameId": 432,
                    "name": "Map and Information",
                    "slug": "map-and-information",
                    "iconUrl": "https://media.forgecdn.net/avatars/categories/10.png",
                    "classId": 6
                }
            ],
            "authors": [
                {
                    "id": 123,
                    "name": "mezz",
                    "url": "https://www.curseforge.com/members/mezz",
                    "avatarUrl": "https://media.forgecdn.net/avatars/123.png"
                }
            ],
            "logo": {
                "id": 456,
                "modId": 238222,
                "title": "JEI logo",
                "thumbnailUrl": "https://media.forgecdn.net/avatars/thumbnails/456.png",
                "url": "https://media.forgecdn.net/avatars/456.png"
            },
            "screenshots": [
                {
                    "id": 1,
                    "title": "shot",
                    "description": "a screenshot",
                    "thumbnailUrl": "https://media.forgecdn.net/attachments/1/2/thumb.png",
                    "url": "https://media.forgecdn.net/attachments/1/2/shot.png"
                }
            ],
            "dateCreated": "2016-04-20T00:00:00Z",
            "dateModified": "2026-01-01T00:00:00Z",
            "dateReleased": "2016-04-20T00:00:00Z",
            "allowModDistribution": true,
            "gamePopularityRank": 10
        }
    ],
    "pagination": {
        "index": 0,
        "pageSize": 20,
        "resultCount": 1,
        "totalCount": 12345
    }
}"#;

/// CurseForge 文件列表接口 `/mods/{id}/files` 的真实响应结构
const FILE_PAGE_JSON: &str = r#"{
    "data": [
        {
            "id": 5152805,
            "gameId": 432,
            "modId": 238222,
            "isAvailable": true,
            "displayName": "JEI 1.20.4-forge 15.3.0.4",
            "fileName": "jei-1.20.4-forge-15.3.0.4.jar",
            "releaseType": 1,
            "fileStatus": 4,
            "hashes": [
                { "value": "0123456789abcdef0123456789abcdef01234567", "algo": 1 },
                { "value": "fedcba9876543210", "algo": 2 }
            ],
            "fileDate": "2024-01-31T18:44:57.907Z",
            "fileLength": 1532451,
            "downloadCount": 873000,
            "downloadUrl": "https://www.curseforge.com/api/v1/mods/238222/files/5152805",
            "gameVersions": ["1.20.4", "Forge"],
            "dependencies": [
                { "modId": 63462, "relationType": 3 }
            ],
            "exposeAsAlternative": null,
            "parentProjectFileId": null
        }
    ],
    "pagination": {
        "index": 0,
        "pageSize": 50,
        "resultCount": 1,
        "totalCount": 913
    }
}"#;

/// CurseForge 单文件接口 `/mods/{id}/files/{fileId}` 的响应，`downloadUrl` 可为 null
const FILE_OBJ_NULL_URL_JSON: &str = r#"{
    "data": {
        "id": 4279300,
        "gameId": 432,
        "modId": 60089,
        "isAvailable": true,
        "displayName": "OptiFine HD U I7",
        "fileName": "OptiFine-1.19.2_HD_U_I7.jar",
        "releaseType": 1,
        "fileStatus": 4,
        "hashes": [],
        "fileDate": "2022-10-01T00:00:00Z",
        "fileLength": 2468574,
        "downloadCount": 1,
        "downloadUrl": null,
        "gameVersions": ["1.19.2"]
    }
}"#;

/// CurseForge 分类接口 `/categories` 的响应，只保留 `classId` 非 null 的叶子分类
const CATEGORIES_JSON: &str = r#"{
    "data": [
        {
            "id": 4471,
            "gameId": 432,
            "name": "Modpacks",
            "slug": "modpacks",
            "iconUrl": "https://media.forgecdn.net/avatars/categories/16.png",
            "dateModified": "2022-01-01T00:00:00Z",
            "isClass": true,
            "classId": 0,
            "displayIndex": 1
        },
        {
            "id": 423,
            "gameId": 432,
            "name": "Map and Information",
            "slug": "map-and-information",
            "iconUrl": "https://media.forgecdn.net/avatars/categories/10.png",
            "isClass": false,
            "classId": 6,
            "displayIndex": 2
        }
    ]
}"#;

/// CurseForge 版本列表接口 `/games/{id}/versions` 的响应
const VERSIONS_JSON: &str = r#"{
    "data": [
        { "type": 3, "versions": ["1.20.4", "1.20.3"] },
        { "type": 4, "versions": ["1.19.2"] }
    ]
}"#;

/// CurseForge 版本类型接口 `/games/{id}/version-types` 的响应，`id` 为整数
const VERSION_TYPES_JSON: &str = r#"{
    "data": [
        { "id": 628, "name": "Minecraft 1.20.4" },
        { "id": 17, "name": "Minecraft 1.7.10" }
    ]
}"#;

/// 项目详情接口 `/mods/{id}` 的响应
const MOD_INFO_JSON: &str = r#"{
    "data": {
        "id": 361387,
        "gameId": 432,
        "classId": 6,
        "slug": "sodium",
        "name": "Sodium",
        "links": { "websiteUrl": "https://modrinth.com/mod/sodium" },
        "summary": "A modern rendering engine",
        "downloadCount": 50000000,
        "categories": [],
        "authors": [{ "name": "jellysquid3", "avatarUrl": "" }],
        "logo": { "url": "https://media.forgecdn.net/avatars/789.png" },
        "screenshots": [],
        "dateModified": "2026-02-01T00:00:00Z"
    }
}"#;

/// 搜索响应反序列化：字段映射应与真实响应一致
#[test]
fn deserialize_search_page() {
    let obj: CurseForgeListPageObj = serde_json::from_str(SEARCH_JSON).unwrap();

    assert_eq!(obj.data.len(), 1);
    let item = &obj.data[0];
    assert_eq!(item.id, 238222);
    assert_eq!(item.class_id, 6);
    assert_eq!(item.name, "Just Enough Items (JEI)");
    assert_eq!(
        item.links.website_url,
        "https://www.curseforge.com/minecraft/mc-mods/jei"
    );
    assert_eq!(item.download_count, 300000000);
    assert_eq!(item.categories.len(), 1);
    assert_eq!(item.categories[0].name, "Map and Information");
    assert_eq!(item.categories[0].class_id, 6);
    assert_eq!(item.authors[0].name, "mezz");
    assert_eq!(
        item.logo.url,
        Some(String::from("https://media.forgecdn.net/avatars/456.png"))
    );
    assert_eq!(
        item.screenshots[0].url,
        "https://media.forgecdn.net/attachments/1/2/shot.png"
    );
    assert_eq!(item.date_modified, "2026-01-01T00:00:00Z");

    assert_eq!(obj.pagination.total_count, 12345);
}

/// 文件列表响应反序列化：哈希、依赖、分页
#[test]
fn deserialize_file_page() {
    let obj: CurseFogreFilePageObj = serde_json::from_str(FILE_PAGE_JSON).unwrap();

    assert_eq!(obj.data.len(), 1);
    let file = &obj.data[0];
    assert_eq!(file.id, 5152805);
    assert_eq!(file.mod_id, 238222);
    assert_eq!(file.display_name, "JEI 1.20.4-forge 15.3.0.4");
    assert_eq!(file.file_name, "jei-1.20.4-forge-15.3.0.4.jar");
    assert_eq!(file.file_length, 1532451);
    assert_eq!(file.hashes.len(), 2);
    // SHA1 (algo=1) 提取
    assert_eq!(file.sha1_hash(), "0123456789abcdef0123456789abcdef01234567");
    // downloadUrl 存在时不应为 None
    assert!(file.download_url.is_some());
    let deps = file.dependencies.as_ref().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].mod_id, 63462);
    assert_eq!(deps[0].relation_type, 3);

    assert_eq!(obj.pagination.total_count, 913);
}

/// 单文件响应：downloadUrl 为 null 时应解出 None，且 fix_download_url 可补齐
#[test]
fn deserialize_file_with_null_download_url() {
    let obj: CurseForgeFileObj = serde_json::from_str(FILE_OBJ_NULL_URL_JSON).unwrap();

    let mut file = obj.data;
    assert_eq!(file.file_name, "OptiFine-1.19.2_HD_U_I7.jar");
    assert_eq!(file.download_url, None);
    assert_eq!(file.sha1_hash(), "");

    file.fix_download_url();
    assert_eq!(
        file.download_url.as_deref(),
        Some("https://edge.forgecdn.net/files/4279/300/OptiFine-1.19.2_HD_U_I7.jar")
    );
}

/// 分类响应反序列化
#[test]
fn deserialize_categories() {
    let obj: CurseForgeCategoriesObj = serde_json::from_str(CATEGORIES_JSON).unwrap();

    assert_eq!(obj.data.len(), 2);
    assert_eq!(obj.data[0].name, "Modpacks");
    assert_eq!(obj.data[1].class_id, 6);
}

/// 版本列表响应反序列化
#[test]
fn deserialize_versions() {
    let obj: CurseForgeVersionObj = serde_json::from_str(VERSIONS_JSON).unwrap();

    assert_eq!(obj.data.len(), 2);
    assert_eq!(obj.data[0].verion_type, 3);
    assert_eq!(obj.data[0].versions, vec!["1.20.4", "1.20.3"]);
}

/// 版本类型响应反序列化
#[test]
fn deserialize_version_types() {
    let obj: CurseForgeVersionTypeObj = serde_json::from_str(VERSION_TYPES_JSON).unwrap();

    assert_eq!(obj.data.len(), 2);
    assert_eq!(obj.data[0].id, 628);
    assert_eq!(obj.data[0].name, "Minecraft 1.20.4");
    assert_eq!(obj.data[1].id, 17);
}

/// 项目详情响应反序列化
#[test]
fn deserialize_mod_info() {
    let obj: CurseForgeListObj = serde_json::from_str(MOD_INFO_JSON).unwrap();

    assert_eq!(obj.data.id, 361387);
    assert_eq!(obj.data.name, "Sodium");
    assert_eq!(obj.data.class_id, 6);
}

/// 常量取值：与 CurseForge API 文档一致
#[test]
fn api_constants() {
    assert_eq!(GAME_ID, 432);
    assert_eq!(CLASS_MODPACK, 4471);
    assert_eq!(CLASS_MOD, 6);
}

/// API Key 的设置与读取，使用假密钥。
///
/// `API_KEY` 是全局 `OnceLock`，只能设置一次，因此本测试集中验证：
/// 未设置时返回 `KeyIsNull`，设置后返回原值；重复设置第一次的值不会被覆盖。
#[test]
fn key_set_and_get() {
    // 尚未设置时应返回 KeyIsNull 错误
    // 注意：若其它测试进程内已提前设置，此分支不会触发，因此仅在未设置时断言
    if let Err(err) = get_key() {
        assert!(matches!(err, ErrorType::KeyIsNull));
    }

    set_key("test-key");
    assert_eq!(get_key().unwrap(), "test-key");

    // OnceLock 语义：再次设置不生效
    set_key("another-key");
    assert_eq!(get_key().unwrap(), "test-key");
}

/// 文件对象默认值：全字段默认构造后应能正常序列化/反序列化往返
#[test]
fn file_data_obj_roundtrip() {
    let obj = CurseForgeFileDataObj::default();
    let json = serde_json::to_string(&obj).unwrap();
    let back: CurseForgeFileDataObj = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, 0);
    assert_eq!(back.download_url, None);
}
