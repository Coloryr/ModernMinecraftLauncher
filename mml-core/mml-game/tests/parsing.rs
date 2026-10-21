//! 反序列化测试：Modrinth / CurseForge / ServerPack 整合包格式，以及游戏实例配置。

use mml_base::file_item::{FileHash, FileItemObj};
use mml_game::curseforge::pack_obj::{CurseForgePackObj, FilesObj};
use mml_game::launcher::LogEncoding;
use mml_game::launcher::ModPackType;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::loader::LoaderType;
use mml_game::modrinth::make_pack_download_obj;
use mml_game::modrinth::pack_obj::{ModrinthPackFileObj, ModrinthPackObj};
use mml_game::mojang::VersionType;
use mml_game::serverpack::serverpack_obj::ServerPackObj;
use mml_net::modrinth_api::version_obj::HasheObj;
use serde_json::json;

/// Modrinth mrpack 的 `modrinth.index.json` 解析，字段名为 camelCase
#[test]
fn modrinth_pack_obj_parse() {
    let json = json!({
        "formatVersion": 1,
        "versionId": "14.0.0-beta.3",
        "name": "Fabulously Optimized",
        "summary": "Fabulously Optimized Modpack",
        "files": [
            {
                "path": "mods/BetterGrassify-1.8.7+fabric.26.2.jar",
                "hashes": {
                    "sha1": "0f4a890d07402280686a518579fa9fa02309e315",
                    "sha512": "7b70e796cea2ee57a6022108092517b6aa39d5a19b8da1de26685ae53b8fbb4717cd91ee1d232c54efa8e210ebe9b760d3dab79bf9a5fdb5ae75d97b461a1fab"
                },
                "env": { "client": "required", "server": "optional" },
                "downloads": ["https://cdn.modrinth.com/data/m5T5xmUy/versions/r4yqxYQl/BetterGrassify-1.8.7+fabric.26.2.jar"],
                "fileSize": 46949
            }
        ],
        "dependencies": {
            "fabric-loader": "0.19.3",
            "minecraft": "26.2"
        }
    });

    let obj: ModrinthPackObj = serde_json::from_value(json).unwrap();

    assert_eq!(obj.format_version, 1);
    assert_eq!(obj.version_id, "14.0.0-beta.3");
    assert_eq!(obj.name, "Fabulously Optimized");
    assert_eq!(obj.files.len(), 1);
    assert_eq!(
        obj.dependencies.get("minecraft").map(String::as_str),
        Some("26.2")
    );
    assert_eq!(
        obj.dependencies.get("fabric-loader").map(String::as_str),
        Some("0.19.3")
    );

    let file = &obj.files[0];
    assert_eq!(file.path, "mods/BetterGrassify-1.8.7+fabric.26.2.jar");
    assert_eq!(file.hashes.sha1, "0f4a890d07402280686a518579fa9fa02309e315");
    assert_eq!(file.hashes.sha512.len(), 128);
    assert_eq!(file.downloads.len(), 1);
    assert_eq!(file.file_size, 46949);
}

/// `_private_data` 是本项目私有字段，应解析为 `project`
#[test]
fn modrinth_pack_private_data() {
    let json = json!({
        "formatVersion": 1,
        "versionId": "v1",
        "name": "p",
        "summary": "",
        "files": [{
            "path": "mods/a.jar",
            "hashes": {"sha1": "s1", "sha512": "s512"},
            "downloads": [],
            "fileSize": 1,
            "_private_data": {"pid": "abc", "fid": "def"}
        }],
        "dependencies": {}
    });

    let obj: ModrinthPackObj = serde_json::from_value(json).unwrap();
    let project = obj.files[0].project.as_ref().expect("应解析出私有数据");
    assert_eq!(project.pid, "abc");
    assert_eq!(project.fid, "def");
}

/// `make_pack_download_obj`：从 Modrinth 整合包文件构建下载项
#[test]
fn make_pack_download_obj_hash() {
    let obj = ModrinthPackFileObj {
        path: "mods/example-1.0.jar".to_string(),
        hashes: HasheObj {
            sha1: "a".repeat(40),
            sha512: "b".repeat(128),
        },
        downloads: vec![
            "https://cdn.modrinth.com/data/abc/versions/def/example-1.0.jar".to_string(),
        ],
        file_size: 1024,
        project: None,
    };

    let item: FileItemObj = make_pack_download_obj(&obj, "game");
    assert_eq!(item.url, obj.downloads[0]);
    assert_eq!(item.name, obj.path);
    assert_eq!(
        item.file,
        std::path::PathBuf::from("game/mods/example-1.0.jar")
    );
    assert!(
        matches!(item.hash, FileHash::Sha1Sha512(ref s1, ref s512) if s1.len() == 40 && s512.len() == 128)
    );
}

/// CurseForge `manifest.json` 解析，字段名为驼峰式，含 projectID 与 fileID
#[test]
fn curseforge_pack_obj_parse() {
    let json = json!({
        "minecraft": {
            "version": "1.20.1",
            "modLoaders": [
                { "id": "forge-47.2.0", "primary": true }
            ]
        },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "Test Pack",
        "version": "1.0.0",
        "author": "test",
        "files": [
            { "projectID": 12345, "fileID": 67890, "required": true }
        ],
        "overrides": "overrides"
    });

    let obj: CurseForgePackObj = serde_json::from_value(json).unwrap();

    assert_eq!(obj.minecraft.version, "1.20.1");
    assert_eq!(obj.minecraft.mod_loaders[0].id, "forge-47.2.0");
    assert!(obj.minecraft.mod_loaders[0].primary);
    assert_eq!(obj.manifest_type, "minecraftModpack");
    assert_eq!(obj.manifest_version, 1);
    assert_eq!(obj.name, "Test Pack");
    assert_eq!(obj.version, "1.0.0");
    assert_eq!(obj.overrides, "overrides");

    let file: &FilesObj = &obj.files[0];
    assert_eq!(file.project_id, 12345);
    assert_eq!(file.file_id, 67890);
    assert!(file.required);
}

/// ServerPack 格式解析，字段名为 PascalCase
#[test]
fn server_pack_obj_parse() {
    let json = json!({
        "Name": "server1",
        "Version": "1.20.4",
        "Loader": 2,
        "LoaderVersion": "49.0.0",
        "Text": "欢迎",
        "PackVersion": "1.0",
        "Files": [
            {
                "File": "mods/foo.jar",
                "ProjectId": "abc",
                "FileId": "def",
                "Url": "https://example.com/foo.jar",
                "Sha1": "aaa",
                "Sha256": "bbb"
            }
        ],
        "Archives": [
            { "File": "config.zip", "Path": "config", "Over": true, "Url": "https://example.com/config.zip", "Sha1": "ccc" }
        ]
    });

    let obj: ServerPackObj = serde_json::from_value(json).unwrap();

    assert_eq!(obj.name, "server1");
    assert_eq!(obj.version, "1.20.4");
    // serde_repr：Loader 枚举按数字序列化，2 = Fabric
    assert_eq!(obj.loader, LoaderType::Fabric);
    assert_eq!(obj.loader_version.as_deref(), Some("49.0.0"));
    assert_eq!(obj.text, "欢迎");
    assert_eq!(obj.pack_version, "1.0");

    assert_eq!(obj.online_list.len(), 1);
    assert_eq!(obj.online_list[0].file, "mods/foo.jar");
    assert_eq!(obj.online_list[0].pid.as_deref(), Some("abc"));
    assert_eq!(
        obj.online_list[0].url.as_deref(),
        Some("https://example.com/foo.jar")
    );
    assert_eq!(obj.online_list[0].sha256.as_deref(), Some("bbb"));

    assert_eq!(obj.archive_list.len(), 1);
    assert_eq!(obj.archive_list[0].file, "config.zip");
    assert_eq!(obj.archive_list[0].dir, "config");
    assert!(obj.archive_list[0].delete_old);
    assert_eq!(obj.archive_list[0].sha1.as_deref(), Some("ccc"));
}

/// 游戏实例配置：字段名为 PascalCase，serde_repr 枚举按整数序列化
#[test]
fn instance_setting_round_trip() {
    let instance = InstanceSettingObj {
        uuid: uuid::Uuid::parse_str("f47ac10b-58cc-4372-a567-0e02b2c3d479").unwrap(),
        name: "test-inst".to_string(),
        dir: "test-inst".to_string(),
        version: "1.20.4".to_string(),
        loader: LoaderType::Forge,
        loader_version: Some("49.0.0".to_string()),
        is_modpack: true,
        modpack_type: ModPackType::CurseForge,
        encoding: LogEncoding::GBK,
        ..Default::default()
    };

    // 序列化：字段应为 PascalCase，枚举为整数
    let json = serde_json::to_value(&instance).unwrap();
    assert_eq!(json["Name"], "test-inst");
    assert_eq!(json["Version"], "1.20.4");
    assert_eq!(json["Loader"], 1); // LoaderType::Forge
    assert_eq!(json["ModPackType"], 0); // ModPackType::CurseForge
    assert_eq!(json["Encoding"], 1); // LogEncoding::GBK
    // 分组已迁到独立的 group_save.json，实例配置里不该再出现 GroupName
    assert!(json.get("GroupName").is_none());
    assert_eq!(json["UUID"], "f47ac10b-58cc-4372-a567-0e02b2c3d479");

    // 反序列化：还原字段
    let restored: InstanceSettingObj = serde_json::from_value(json).unwrap();
    assert_eq!(restored.uuid, instance.uuid);
    assert_eq!(restored.name, "test-inst");
    assert_eq!(restored.version, "1.20.4");
    assert_eq!(restored.loader, LoaderType::Forge);
    assert_eq!(restored.loader_version, Some("49.0.0".to_string()));
    assert_eq!(restored.is_modpack, true);
    assert_eq!(restored.modpack_type, ModPackType::CurseForge);
    assert_eq!(restored.encoding, LogEncoding::GBK);
    // 缺失字段使用默认值
    assert!(restored.jvm_arg.is_none());
    assert_eq!(restored.game_type, VersionType::Release);
}
