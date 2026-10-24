//! 导出信息与目录浏览
//!
//! 导出前先看这一份：整合包元信息、要打包的文件与在线文件映射；
//! 目录浏览只读，给前端的文件选择用。

use std::{collections::HashMap, path::{Path, PathBuf}, sync::{atomic::{AtomicBool}}};

use crate::err_keys;

use mml_base::{file_item::FileHash, hash_helper::{self, HashType}};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::loader::LoaderType;
use tauri::{AppHandle, Emitter};

use crate::{dtos::export_dto::{ExportEntryDto, ExportInfoDto, ExportProgressDto}, dtos::log_dto::LogFocusDto, listens};

use crate::windows::{config::resolve_under, instance_snapshot};


/// 导出任务进行中，同时只允许一个导出任务
pub(crate) static EXPORT_RUNNING: AtomicBool = AtomicBool::new(false);

/// 切换目标实例事件：窗口已存在时再次打开会推送，前端据此切换实例
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 目标实例 uuid
#[gui_macros::emit]
pub fn emit_export_focus(app: &AppHandle, event: LogFocusDto) {
    let _ = app.emit(listens::EXPORT_FOCUS, event);
}

/// 导出进度事件：打包各文件时推进，结束时发 done / failed
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 当前进度快照
#[gui_macros::emit]
pub(super) fn emit_export_progress(app: &AppHandle, event: ExportProgressDto) {
    let _ = app.emit(listens::EXPORT_PROGRESS, event);
}

/// 收集目录下全部文件
///
/// - `path`: 待收集的目录
///
/// # 返回值
///
/// 目录下的全部文件；读取失败或遇到链接时终止导出。
pub(super) fn collect_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    super::archive::files_under(path)
}

/// 读 mod_info.json 里带在线来源的那些，键是文件名
///
/// 判据是 modid 与 fileid 都非空：只有一个的话拼不出清单项。
///
/// - `game`: 实例设置快照
///
/// # 返回值
///
/// 文件名 → 来源信息
pub(crate) fn online_of(game: &InstanceSettingObj) -> HashMap<String, OnlineInfoObj> {
    let mut map = HashMap::new();
    for info in game.read_online_info().into_values() {
        if !info.modid.is_empty() && !info.fileid.is_empty() {
            map.insert(info.file.clone(), info);
        }
    }
    map
}

/// 递归累加目录里所有文件的大小
///
/// - `path`: 目录
///
/// # 返回值
///
/// 字节数；读不到的条目略过，链接不跟进，免得绕圈或算进别的目录
fn dir_size(path: &Path) -> u64 {
    let Ok(items) = std::fs::read_dir(path) else { return 0 };
    let mut total = 0;
    for item in items.flatten() {
        let Ok(kind) = item.file_type() else { continue };
        if kind.is_symlink() { continue; }
        if kind.is_dir() {
            total += dir_size(&item.path());
        } else if kind.is_file() {
            total += item.metadata().map(|meta| meta.len()).unwrap_or(0);
        }
    }
    total
}

/// 列一个目录的直接子项，路径转成相对游戏目录，顺带补上在线来源
///
/// - `dir`: 待列的目录
/// - `base`: 游戏目录，即 `.minecraft`
/// - `online`: 在线来源表，见 [`online_of`]
///
/// # 返回值
///
/// 见 [`ExportEntryDto`]；目录在前、同类按路径排序，读不到目录返回内核错误串
pub(super) fn list_entries(
    dir: &Path,
    base: &Path,
    online: &HashMap<String, OnlineInfoObj>,
) -> Result<Vec<ExportEntryDto>, String> {
    let mut list = Vec::new();

    for item in std::fs::read_dir(dir).map_err(|err| err.to_string())? {
        let Ok(item) = item else {
            continue;
        };
        let path = item.path();
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let is_dir = path.is_dir();
        let info = online.get(&item.file_name().to_string_lossy().to_string());

        list.push(ExportEntryDto {
            path: rel.to_string_lossy().replace('\\', "/"),
            dir: is_dir,
            size: if is_dir {
                dir_size(&path)
            } else {
                item.metadata().map(|meta| meta.len()).unwrap_or(0)
            },
            url: info.map(|x| x.url.clone()).unwrap_or_default(),
            mod_id: info.map(|x| x.modid.clone()),
            file_id: info.map(|x| x.fileid.clone()),
        });
    }

    list.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.path.cmp(&b.path)));
    Ok(list)
}

/// 获取实例导出信息：实例元数据加 `mods` 目录下的模组
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ExportInfoDto`]；uuid 非法或实例不存在返回错误文案
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub fn export_get_info(uuid: String) -> Result<ExportInfoDto, String> {
    let game = instance_snapshot(&uuid)?;
    let online = online_of(&game);
    let mods_path = game.get_mods_path();
    let mods = if mods_path.exists() {
        list_entries(&mods_path, &game.get_game_path(), &online)?
    } else {
        Vec::new()
    };

    Ok(ExportInfoDto {
        can_bundle_launcher: cfg!(windows),
        name: game.name.clone(),
        custom_loader: game.loader == LoaderType::Custom,
        mods,
    })
}

/// 浏览游戏目录下的一个目录，给"文件选择"当懒加载的树用
///
/// - `uuid`: 实例 uuid
/// - `dir`: 相对游戏目录的路径；空串表示游戏目录本身
///
/// # 返回值
///
/// 该目录的直接子项；路径非法返回 `err.fileName`，目录不存在返回 `err.fileNotFound`
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub fn export_list_dir(uuid: String, dir: String) -> Result<Vec<ExportEntryDto>, String> {
    let game = instance_snapshot(&uuid)?;
    let base = game.get_game_path();

    // 根目录不走路径守卫：那里允许空串
    let path = if dir.trim().is_empty() {
        base.clone()
    } else {
        resolve_under(&base, &dir)?
    };
    if !path.is_dir() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }

    let online = online_of(&game);
    list_entries(&path, &base, &online)
}

/// 浏览实例目录下的一个目录，给「直接压缩」的文件树用
///
/// 与 [`export_list_dir`] 的区别只在根：这里从实例目录（含 `.minecraft`、`game.json`）起，
/// 返回的路径也相对实例目录，导出时按同一口径解回绝对路径。
///
/// - `uuid`: 实例 uuid
/// - `dir`: 相对实例目录的路径；空串表示实例目录本身
///
/// # 返回值
///
/// 该目录的直接子项；路径非法返回 `err.fileName`，目录不存在返回 `err.fileNotFound`
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub fn export_list_instance_dir(uuid: String, dir: String) -> Result<Vec<ExportEntryDto>, String> {
    let game = instance_snapshot(&uuid)?;
    let base = game.get_base_path();
    let path = if dir.trim().is_empty() {
        base.clone()
    } else {
        resolve_under(&base, &dir)?
    };
    if !path.is_dir() {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    }

    let online = online_of(&game);
    list_entries(&path, &base, &online)
}

/// 导出条目的文件元数据
pub(super) struct ExportMeta {
    /// 文件大小，单位字节
    pub(super) size: usize,
    /// 文件哈希
    pub(super) hash: FileHash,
}

/// 算导出条目要用的大小与校验值
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 见 [`ExportMeta`]；元数据或哈希读取失败时返回错误。
pub(super) fn export_meta(file: &Path) -> Result<ExportMeta, String> {
    let meta = std::fs::metadata(file).map_err(|e| e.to_string())?;
    let sha1 = hash_helper::gen_hash_from_file(HashType::Sha1, file).map_err(|e| e.to_string())?;
    let sha512 = hash_helper::gen_hash_from_file(HashType::Sha512, file).map_err(|e| e.to_string())?;

    Ok(ExportMeta {
        size: meta.len() as usize,
        hash: FileHash::Sha1Sha512(sha1, sha512),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_size_sums_nested_files() {
        let dir = crate::test_support::temp_dir("export-dir-size");
        std::fs::create_dir_all(dir.join("config/nested")).unwrap();
        std::fs::write(dir.join("config/a.txt"), b"abc").unwrap();
        std::fs::write(dir.join("config/nested/b.txt"), b"defgh").unwrap();
        assert_eq!(dir_size(&dir.join("config")), 8);
        assert_eq!(dir_size(&dir.join("missing")), 0);
        crate::test_support::cleanup(&[&dir]);
    }
}

