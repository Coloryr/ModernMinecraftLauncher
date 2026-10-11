//! 扫描游戏版本

use std::path::{Path, PathBuf};

use mml_base::archives::BaseArchive;
use mml_names::names;
use mml_sys::path_helper;

use crate::{add_game::PackType, other_launcher};

/// 扫描文件夹下的游戏版本
///
/// 按"越像越先试"的顺序：先认启动器布局，最后才逐个目录嗅探。顺序就是性能——
/// 官方 / MMC 的数据目录会在前两步命中，不会去解析 `config` / `mods` 里的杂项 json。
/// 依次尝试：目录或它的直接子目录是启动器数据目录、目录本身就是实例、目录就是
/// `versions` / `instances` 本身、直接子目录本身就是实例目录。
///
/// - `path`: 待扫描的目录
///
/// # 返回值
///
/// 可导入的游戏实例路径列表
pub fn scan_game_from_path<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let root = path.as_ref();
    let mut list = Vec::new();

    // 1. 这个目录 / 它的直接子目录，当作启动器数据目录去找 versions、instances
    let mut roots = vec![root.to_path_buf()];
    roots.extend(path_helper::get_dirs(root));
    for item in &roots {
        collect_launcher_instances(item, &mut list);
        if !list.is_empty() {
            return list;
        }
    }

    // 2. 目录本身就是实例
    if other_launcher::is_minecraft_version(root) {
        return vec![root.to_path_buf()];
    }
    if other_launcher::is_mmc_version(root) {
        return vec![root.to_path_buf()];
    }

    // 3. 选的就是 versions / instances 目录本身：它下面一个个就是实例
    if root.ends_with(names::VERSION_DIR) {
        for item in path_helper::get_dirs(root) {
            if other_launcher::is_minecraft_version(&item) {
                list.push(item);
            }
        }
        if !list.is_empty() {
            return list;
        }
    }
    if root.ends_with(names::INSTANCE_DIR) {
        for item in path_helper::get_dirs(root) {
            if other_launcher::is_mmc_version(&item) {
                list.push(item);
            }
        }
        if !list.is_empty() {
            return list;
        }
    }

    // 4. 兜底：直接子目录本身就是实例目录
    for item in path_helper::get_dirs(root) {
        if other_launcher::is_minecraft_version(&item) {
            list.push(item);
            continue;
        }
        if other_launcher::is_mmc_version(&item) {
            list.push(item);
        }
    }

    list
}

/// 从"启动器数据目录"里收集实例：官方的 `versions/*` 与 MMC 的 `instances/*`
///
/// 走到这里说明目录确实有这个布局，`versions` / `instances` 都是存在的目录，
/// 所以下面的每个子目录都按实例去判定，不做额外过滤 —— 目录名与 json 名不一致的
/// 版本目录也得认出来，如 `1.20.1-forge/` 里放的是 `1.20.1-forge-47.2.0.json`。
///
/// - `root`: 启动器数据目录
/// - `out`: 结果追加到这里
fn collect_launcher_instances(root: &Path, out: &mut Vec<PathBuf>) {
    let versions = root.join(names::VERSION_DIR);
    if versions.is_dir() {
        for item in path_helper::get_dirs(&versions) {
            if other_launcher::is_minecraft_version(&item) {
                out.push(item);
            }
        }
        // 这个目录下已经找到版本了，就不用再看 MMC 的 instances
        if !out.is_empty() {
            return;
        }
    }

    let instances = root.join(names::INSTANCE_DIR);
    if instances.is_dir() {
        for item in path_helper::get_dirs(&instances) {
            if other_launcher::is_mmc_version(&item) {
                out.push(item);
            }
        }
    }
}

/// 检测压缩包类型
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 识别出的包类型；打不开或无法识别返回 `None`
pub fn test_archive_type<P: AsRef<Path>>(path: P) -> Option<PackType> {
    if let Some(ext) = path.as_ref().extension()
        && ext == names::MRPACK_EXT
    {
        return Some(PackType::Modrinth);
    }

    let arch = BaseArchive::open(path);
    if arch.is_err() {
        return None;
    }

    let arch = arch.unwrap();
    for item in arch.entries() {
        if item.is_dir {
            if item.name.starts_with(".minecraft/") || item.name.ends_with(".exe") {
                return Some(PackType::LauncherPack);
            }
        } else {
            if item.name == names::GAME_FILE {
                return Some(PackType::ArchivePack);
            } else if item.name == names::HMCLFILE {
                return Some(PackType::HMCL);
            } else if item.name == names::MMCCFG_FILE {
                return Some(PackType::MMC);
            } else if item.name == names::MANIFEST_FILE {
                return Some(PackType::CurseForge);
            } else if item.name == names::SERVER_MANIFEST_FILE {
                return Some(PackType::HMCLServer);
            } else if item.name == names::MODRINTH_FILE {
                return Some(PackType::Modrinth);
            }
        }
    }

    None
}
