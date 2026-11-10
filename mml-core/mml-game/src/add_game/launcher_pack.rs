//! 官方启动器版本包导入
//!
//! 在压缩包里找版本 json，挑出主版本，其余版本按需解压。

use super::path_not_exists;

use std::path::Path;

use mml_base::{
    archives::{ArchiveEntryInfo, BaseArchive, BaseArchiveGui},
    serialize_tools::MiniJsonObj,
};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType},
    names,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState},
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::version_path,
    other_launcher::official_obj::OfficialObj,
};

/// 压缩包内扫描到的官方版本信息
pub struct LauncherVersion {
    /// 版本 json 在压缩包内的完整条目名
    pub entry: String,
    /// 版本号：隔离时取 `versions/{name}/` 的文件夹名，未隔离时取 json 的 id
    pub version_name: String,
    /// 解析出的官方版本信息
    pub obj: OfficialObj,
    /// 版本 json 是否位于 `versions/{name}/` 下，即是否开启版本隔离
    pub isolated: bool,
}

/// 查找压缩包内 `.minecraft` 文件夹所在目录前缀，含末尾分隔符
///
/// - `entries`: 压缩包条目列表
///
/// 兼容压缩包整体套一层文件夹的情况，如 `mygame/.minecraft/...` 返回 `mygame/.minecraft/`。
///
/// # 返回值
///
/// `.minecraft` 所在目录前缀，含末尾分隔符；找不到返回 `None`
pub fn find_minecraft_prefix(entries: &[ArchiveEntryInfo]) -> Option<String> {
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let norm = entry.name.replace('\\', "/");
        let parts: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
        if let Some(index) = parts.iter().position(|item| *item == names::GAME_DIR) {
            return Some(format!("{}/", parts[..=index].join("/")));
        }
    }
    None
}

/// 扫描 `.minecraft` 下可解析为官方版本 json 的文件
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
///
/// 版本 json 位于 `versions/{name}/` 下视为开启了版本隔离。
///
/// # 返回值
///
/// 扫描到的版本列表
pub fn scan_versions(archive: &BaseArchive, mc_prefix: &str) -> Vec<LauncherVersion> {
    let mut versions = Vec::new();
    for entry in archive.entries() {
        if entry.is_dir {
            continue;
        }
        let norm = entry.name.replace('\\', "/");
        let Some(rel) = norm.strip_prefix(mc_prefix) else {
            continue;
        };
        if !rel.ends_with(names::JSON_DOT_EXT) {
            continue;
        }
        let Ok(data) = archive.read(&entry.name) else {
            continue;
        };
        let Ok(obj) = OfficialObj::from_reader(data.as_slice()) else {
            continue;
        };
        if obj.id.is_empty() {
            continue;
        }

        let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
        let isolated = parts.first() == Some(&"versions") && parts.len() >= 2;
        let version_name = if isolated {
            parts[1].to_string()
        } else {
            obj.id.clone()
        };

        versions.push(LauncherVersion {
            entry: entry.name.clone(),
            version_name,
            obj,
            isolated,
        });
    }
    versions
}

/// 判断 `versions/{name}/` 文件夹内是否有游戏资源
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
/// - `name`: 版本文件夹名
///
/// 版本隔离开启时游戏资源存放在该文件夹内；若只有版本 json/jar，说明仍是未隔离的
/// 常规 `.minecraft` 布局，游戏资源在 `.minecraft` 根目录。
///
/// # 返回值
///
/// 该版本文件夹内是否有除版本 json/jar 以外的游戏资源
pub fn version_folder_has_data(archive: &BaseArchive, mc_prefix: &str, name: &str) -> bool {
    let folder = format!("{mc_prefix}versions/{name}/");
    archive.entries().iter().any(|entry| {
        if entry.is_dir {
            return false;
        }
        let norm = entry.name.replace('\\', "/");
        let Some(rest) = norm.strip_prefix(&folder) else {
            return false;
        };
        !rest.is_empty() && rest != format!("{name}.json") && rest != format!("{name}.jar")
    })
}

/// 读取官方启动器 `launcher_profiles.json` 中记录的版本号
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
///
/// # 返回值
///
/// 记录的版本号列表；文件或字段缺失返回 `None`
fn read_last_version_ids(archive: &BaseArchive, mc_prefix: &str) -> Option<Vec<String>> {
    let target = format!("{mc_prefix}{}", names::LAUNCHER_PROFILES_FILE);
    let entry = archive
        .entries()
        .iter()
        .find(|entry| !entry.is_dir && entry.name.replace('\\', "/") == target)?;
    let data = archive.read(&entry.name).ok()?;
    let json = MiniJsonObj::from_stream(data.as_slice()).ok()?;

    let mut list = Vec::new();
    if let Some(map) = json.as_object()
        && let Some(profiles) = map.get_object("profiles")
    {
        for (_, profile) in profiles.iter() {
            if let Some(profile) = profile.as_object() {
                let id = profile.get_string("lastVersionId");
                if !id.is_empty() {
                    list.push(id);
                }
            }
        }
    }
    Some(list)
}

/// 选择要导入的版本，返回它在列表中的下标
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
/// - `versions`: 扫描到的版本列表
///
/// # 返回值
///
/// 选中版本的下标：优先官方启动器最后使用的版本，其次开启版本隔离且文件夹内有
/// 游戏资源的版本；列表为空返回 `None`
pub fn pick_primary(
    archive: &BaseArchive,
    mc_prefix: &str,
    versions: &[LauncherVersion],
) -> Option<usize> {
    if versions.is_empty() {
        return None;
    }
    if versions.len() == 1 {
        return Some(0);
    }

    // 优先使用官方启动器配置中最后使用的版本
    if let Some(ids) = read_last_version_ids(archive, mc_prefix) {
        for id in ids {
            if let Some(index) = versions
                .iter()
                .position(|item| item.obj.id == id || item.version_name == id)
            {
                return Some(index);
            }
        }
    }

    // 其次优先开启版本隔离且版本文件夹内有游戏资源的版本
    for (index, version) in versions.iter().enumerate() {
        if version.isolated && version_folder_has_data(archive, mc_prefix, &version.version_name) {
            return Some(index);
        }
    }

    Some(0)
}

/// 导入其他启动器的压缩包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖版本 json 解析出的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// 压缩包内可能是整个 `.minecraft` 文件夹，可能整体套了一层文件夹。
/// 通过扫描其中可解析为 [`OfficialObj`] 的版本 json 判断游戏版本与版本隔离：
/// - 版本 json 直接位于 `.minecraft` 下 → 未开启版本隔离，直接导入 `.minecraft` 内的游戏资源；
/// - 版本 json 位于 `.minecraft/versions/{name}/` 下 → 开启了版本隔离，将该版本文件夹内的资源导入。
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或找不到 `.minecraft` 目录返回对应错误
pub(super) async fn launcher_pack<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open_readonly(&file)?;

    // 查找 `.minecraft` 文件夹
    let mc_prefix = match find_minecraft_prefix(archive.entries()) {
        Some(prefix) => prefix,
        None => return Err(ErrorType::DataNotFound(DataNotFoundData::Info)),
    };

    // 扫描可解析的版本 json，并选择要导入的版本
    let mut versions = scan_versions(&archive, &mc_prefix);
    let primary_index = pick_primary(&archive, &mc_prefix, &versions);

    // 版本隔离：版本 json 在 `versions/{name}/` 下 且 该版本文件夹内有游戏资源
    let isolated = primary_index
        .filter(|&index| versions[index].isolated)
        .map(|index| version_folder_has_data(&archive, &mc_prefix, &versions[index].version_name))
        .unwrap_or(false);

    // 提前取出版本文件夹名，供解压时判断路径
    let version_name = primary_index.map(|index| versions[index].version_name.clone());
    let version_prefix = version_name
        .as_ref()
        .filter(|_| isolated)
        .map(|name| format!("{mc_prefix}versions/{name}/"));

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 创建实例设置
    let mut obj = if let Some(index) = primary_index {
        let LauncherVersion {
            obj,
            entry: _,
            version_name: _,
            isolated: _,
        } = versions.remove(index);
        // 版本列表未加载时直接使用 json 中的版本号
        let fallback = if obj.inherits_from.is_empty() {
            obj.id.clone()
        } else {
            obj.inherits_from.clone()
        };
        let mut obj = obj.to_instance();
        if obj.version.is_empty() {
            obj.version = fallback;
        }
        obj
    } else {
        InstanceSettingObj {
            version: version_path::get_latest_version(),
            ..Default::default()
        }
    };

    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    let game_path = game.read().unwrap().get_game_path();
    let unselect = unselect.unwrap_or_default();

    archive.extract_where(
        |entry| {
            if entry.is_dir {
                return None;
            }
            if unselect.iter().any(|item| item == &entry.name) {
                return None;
            }
            let norm = entry.name.replace('\\', "/");
            let Some(rel) = norm.strip_prefix(&mc_prefix) else {
                return None;
            };
            if let Some(prefix) = &version_prefix {
                // 版本隔离：只导入版本文件夹内的游戏资源
                let Some(rest) = norm.strip_prefix(prefix) else {
                    return None;
                };
                let name = version_name.as_ref().unwrap();
                if rest == format!("{name}.json") || rest == format!("{name}.jar") {
                    return None;
                }
                Some(game_path.join(rest))
            } else {
                // 未开启版本隔离：直接导入 `.minecraft` 下的游戏资源
                Some(game_path.join(rel))
            }
        },
        archive_gui.as_deref(),
    )?;

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}
