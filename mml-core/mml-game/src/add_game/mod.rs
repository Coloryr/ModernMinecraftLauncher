//! 添加游戏实例
//!
//! 导入文件夹、导入压缩包，以及从整合包 / 在线实例地址安装。
//! 按「识别包类型 → 各格式导入 → 安装入口」拆开，公开接口仍从本模块导出。

mod archive;
mod detect;
mod folder;
mod install;
mod launcher_archive;
mod launcher_pack;
mod modpack;
mod online;
mod server_pack;

pub use self::detect::{DetectedPack, PackType, detect_pack, detect_pack_of};
pub use self::folder::add_game_folder;
pub use self::install::{install_archive_from_file, install_archive_from_url};
pub use self::launcher_pack::{
    LauncherVersion, find_minecraft_prefix, pick_primary, scan_versions, version_folder_has_data,
};
pub(crate) use self::modpack::download_with_progress;
pub use self::online::{install_curseforge, install_modrinth};
pub use self::server_pack::{
    fetch_server_pack, install_server_pack_from_url, install_server_pack_from_url_with_skip,
    normalize_server_pack_url, server_pack_base_url, upgrade_server_pack,
};

use std::path::PathBuf;

use mml_names::i18_items::error_type::{ErrorType, PathNotExistsData};

/// 整合包元数据文件名
pub(super) const MODRINTH_INDEX_FILE: &str = "modrinth.index.json";
/// 在线实例清单文件名
pub(super) const SERVER_PACK_SUFFIX: &str = "/server.json";

/// 路径不存在转 PathNotExists
pub(super) fn path_not_exists(path: PathBuf) -> ErrorType {
    ErrorType::PathNotExists(PathNotExistsData { path })
}

#[cfg(test)]
#[path = "../../tests/unit/add_game_tests.rs"]
mod tests;
