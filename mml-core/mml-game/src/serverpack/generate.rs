//! 服务器实例清单生成
//!
//! "在线实例"有两份服务端清单要生成：MML 的 `server.json` 与 HMCL 的
//! `server-manifest.json`。这里只做"实例 + 文件条目 → 清单"的映射，
//! 文件的收集、哈希与落盘都在界面侧，所以全是纯函数，可以离线测。

use mml_names::names;

use crate::{
    launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
    other_launcher::hmcl_obj::{AddonsObj, HMCLServerFileObj, HMCLServerObj},
    serverpack::serverpack_obj::{ServerArchiveItemObj, ServerItemObj, ServerPackObj},
};

/// 随包同步的配置目录，存在才打包，不存在跳过
///
/// 这些目录里放的是模组配置文件，客户端没有在线来源可查，只能整份下发。
pub const CONFIG_DIRS: &[&str] = &[
    "config",
    "defaultconfigs",
    "kubejs",
    "scripts",
    "patchouli_books",
];

/// 配置目录对应的压缩包文件名
///
/// - `dir`: 游戏目录下的配置目录名，如 `config`
///
/// # 返回值
///
/// 形如 `config.zip`，与输出文件夹里实际写出的文件同名
pub fn archive_name(dir: &str) -> String {
    format!("{dir}.zip")
}

/// 按实例生成 MML 服务器实例清单
///
/// 版本、加载器、启动参数与窗口设置都取自实例：客户端靠这份清单得知该用什么环境启动。
///
/// - `game`: 源实例
/// - `name`: 清单里的实例名
/// - `version`: 服务器包版本，写进 `PackVersion`
/// - `text`: 服务器包说明，写进 `Text`
/// - `online_list`: 有在线来源的文件，即 `mods` 里的模组
/// - `archive_list`: 要随包下发的配置压缩包
///
/// # 返回值
///
/// 见 [`ServerPackObj`]
pub fn build_server_pack(
    game: &InstanceSettingObj,
    name: &str,
    version: &str,
    text: &str,
    online_list: Vec<ServerItemObj>,
    archive_list: Vec<ServerArchiveItemObj>,
) -> ServerPackObj {
    ServerPackObj {
        name: name.to_string(),
        version: game.version.clone(),
        loader: game.loader,
        loader_version: game.loader_version.clone(),
        run_arg: game.jvm_arg.clone(),
        window: game.window.clone(),
        start_server: game.start_server.clone(),
        advance_jvm: game.advance_jvm.clone(),
        text: text.to_string(),
        pack_version: version.to_string(),
        online_list,
        archive_list,
    }
}

/// 把实例的版本与加载器转成 HMCL 的组件列表
///
/// HMCL 只认原版 / Forge / NeoForge / Fabric / Quilt 这几种组件位：
/// OptiFine、LiteLoader、Cleanroom 与自定义加载器都表达不出来，
/// 加载器版本缺失时也拼不出组件，一律返回 `None`。
///
/// - `game`: 源实例
///
/// # 返回值
///
/// 组件列表，游戏版本永远在第一条；表达不了时返回 `None`
pub fn hmcl_addons(game: &InstanceSettingObj) -> Option<Vec<AddonsObj>> {
    let mut addons = vec![AddonsObj {
        id: names::GAME_KEY.to_string(),
        version: game.version.clone(),
    }];

    let key = match game.loader {
        LoaderType::Normal => return Some(addons),
        LoaderType::Forge => names::FORGE_KEY,
        LoaderType::NeoForge => names::NEOFORGE_KEY,
        LoaderType::Fabric => names::FABRIC_KEY,
        LoaderType::Quilt => names::QUILT_KEY,
        LoaderType::OptiFine | LoaderType::LiteLoader | LoaderType::Cleanroom | LoaderType::Custom => {
            return None;
        }
    };

    addons.push(AddonsObj {
        id: key.to_string(),
        version: game.loader_version.clone()?,
    });
    Some(addons)
}

/// 生成 HMCL 服务器清单
///
/// - `name`: 服务器实例名
/// - `version`: 清单版本
/// - `description`: 说明
/// - `file_api`: 文件下载源地址，客户端按 `file_api/路径` 拼出每个文件的下载地址
/// - `addons`: 游戏版本与加载器组件，见 [`hmcl_addons`]
/// - `files`: 要同步的文件，路径相对游戏目录
///
/// # 返回值
///
/// 见 [`HMCLServerObj`]
pub fn build_hmcl_pack(
    name: &str,
    version: &str,
    description: &str,
    file_api: &str,
    addons: Vec<AddonsObj>,
    files: Vec<HMCLServerFileObj>,
) -> HMCLServerObj {
    HMCLServerObj {
        name: name.to_string(),
        author: String::new(),
        version: version.to_string(),
        description: description.to_string(),
        file_api: file_api.to_string(),
        files,
        addons,
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!((addons[0].id.as_str(), addons[0].version.as_str()), ("game", "1.21.1"));
        assert_eq!((addons[1].id.as_str(), addons[1].version.as_str()), ("forge", "52.0.1"));

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
}
