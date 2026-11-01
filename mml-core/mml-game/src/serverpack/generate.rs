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
        // 图标由调用方按实际文件补，见 server_instance 的 generate_mml
        icon: None,
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
        LoaderType::OptiFine
        | LoaderType::LiteLoader
        | LoaderType::Cleanroom
        | LoaderType::Custom => {
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
#[path = "../../tests/unit/serverpack_generate_tests.rs"]
mod tests;
