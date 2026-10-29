//! 新文件落地：替换、清理与判重
//!
//! 先算好同目录同名与旧文件处置，再落盘并收尾。

use std::path::Path;

use mml_base::file_item::LaterRun;
use mml_game::GameInstance;
use mml_game::game_mods::{ModObj, add_disable_suffix};
use mml_game::launcher::ModPackType;
use mml_game::launcher::mod_update_obj::ModUpdateRecordObj;
use mml_game::{curseforge, modrinth};
use mml_names::names;

use crate::dtos::ModUpdateApplyDto;
use crate::err_keys;

use super::newfile::{NewFileData, PendingUpdate, fetch_one};
use crate::windows::resource::mods::find_mod;

/// 准备一条更新：把平台数据拼成下载项，并攒好收尾要用的信息
///
/// 正常情况下平台数据由 [`super::newfile::fetch_new_files`] 批量取好，这里只做本地
/// 拼装；批量那一趟没拿到时退回 [`fetch_one`] 单独问一次。
///
/// - `instance`: 实例句柄
/// - `dir`: mods 目录，调用方取一次共用
/// - `item`: 要更新的那一条
/// - `data`: 这条的新文件平台数据；批量取那一趟没拿到时是 `None`
///
/// # 返回值
///
/// 见 [`PendingUpdate`]；旧文件找不到、平台数据缺失、文件号非法返回错误串
pub(super) async fn prepare_one(
    instance: GameInstance,
    dir: &Path,
    item: &ModUpdateApplyDto,
    data: Option<NewFileData>,
) -> Result<PendingUpdate, String> {
    let source = ModPackType::from_string(&item.source);
    if !matches!(source, ModPackType::CurseForge | ModPackType::Modrinth) {
        return Err(err_keys::SOURCE_TYPE.to_string());
    }

    // 旧文件按 uuid 定位；顺带记下它是不是禁用着，更新完要接着禁用
    let old = find_mod(instance.clone(), &item.uuid).await?;

    // 批量那一趟没拿到：退回逐个取。慢一点，但一次批量请求出问题不会把整批
    // 都判成"文件找不到"，批量只当加速用
    let data = match data {
        Some(data) => data,
        None => fetch_one(source, item).await?,
    };

    let (mut obj, info) = match data {
        NewFileData::CurseForge(mut data) => {
            let obj = curseforge::make_file_item_obj(&mut data, dir);
            let info = curseforge::make_file_online_info_obj(&mut data, names::GAME_MODS_DIR);
            (obj, info)
        }
        NewFileData::Modrinth(data) => {
            // Modrinth 偶尔给一个 files 都不带的版本；两个构造函数都会直接取 files[0]
            if data.files.is_empty() {
                return Err(err_keys::FILE_NOT_FOUND.to_string());
            }
            let obj = modrinth::make_download_obj(&data, dir);
            let info = modrinth::make_file_online_obj(&data, names::GAME_MODS_DIR);
            (obj, info)
        }
    };

    // 换之前那一份的文件号：覆盖在线信息之前先取出来，给更新记录用
    let old_file_id = {
        let game = instance.read().unwrap();
        game.read_online_info()
            .get(&item.project_id)
            .map(|data| data.fileid.clone())
            .unwrap_or_default()
    };
    // 记录在这一步就拼好：等下载完再回头问平台是多余的，那时在线信息也已经被覆盖
    let record = ModUpdateRecordObj {
        file: info.file.clone(),
        old_version: item.old_version.clone(),
        version: item.new_version.clone(),
        old_file_id,
        file_id: info.fileid.clone(),
        time: chrono::Local::now().fixed_offset(),
        sha1: info.sha1.clone(),
    };

    // 换旧文件交给下载器的后续操作：**下完一个换一个**，不用等整批下完，
    // mods 里也不会新旧同名文件同时存在。同名的不交给它 —— 那会把刚下好的这份挪走
    if old.file != obj.file {
        obj.later = LaterRun::ReplaceOld(old.file.clone());
    }

    Ok(PendingUpdate {
        obj,
        old: old.file,
        disable: old.disable,
        project_id: item.project_id.clone(),
        name: item.name.clone(),
        info,
        record,
    })
}

/// 收尾一条更新：删旧文件、补禁用后缀、写回在线信息与更新记录
///
/// 调用方**先确认新文件已经下好**，这里不再碰网络。
///
/// - `instance`: 实例句柄
/// - `entry`: 准备阶段攒下的那条
///
/// # 返回值
///
/// 删旧文件或写盘失败返回错误串
pub(super) fn finish_one(instance: &GameInstance, entry: PendingUpdate) -> Result<(), String> {
    // 正常情况旧文件已由下载器的 `LaterRun::ReplaceOld` 换掉（下完一个换一个），
    // 这里只是兜底；同名时 `stale_file` 本来就返回 `None`
    if let Some(stale) = stale_file(&entry.old, &entry.obj.file) {
        old_delete(stale)?;
    }
    // 原本禁用着：给新文件补上后缀，别让更新把用户禁用的模组启用了
    if entry.disable {
        add_disable_suffix(&entry.obj.file).map_err(|err| err.to_string())?;
    }

    let game = instance.read().unwrap();
    let mut online = game.read_online_info();
    online.insert(entry.project_id.clone(), entry.info);
    game.save_online_info(&online);

    // 记一条更新记录：从哪个版本换到哪个版本、两边的文件号、时间与新文件的 SHA1。
    // 记录是附属品，写失败只记日志，不让这次更新算失败
    if let Err(err) = game.append_mod_update(&entry.project_id, &entry.name, entry.record) {
        mml_log::error_type(err);
    }
    Ok(())
}

/// 更新完之后要删掉的旧文件，新旧同一个路径时返回 `None`
///
/// 平台换了内容却没改名是有的：这时下载已经覆盖了同一个路径，
/// 再删就是把刚下好的那份删掉。
///
/// - `old`: 旧文件路径
/// - `new`: 新文件路径
///
/// # 返回值
///
/// 要删的旧文件；新旧同一个路径时没有
pub(super) fn stale_file<'a>(old: &'a Path, new: &Path) -> Option<&'a Path> {
    (!same_path(old, new)).then_some(old)
}

/// 两个路径是不是同一个文件，只比字面、不碰文件系统
///
/// Windows 上大小写不敏感：新旧文件名只差大小写时是**同一个文件**，
/// 下载已经把它换了内容，此时再按"旧路径"删一次就会把刚下好的那份删掉。
///
/// - `old`: 旧文件路径
/// - `new`: 新文件路径
///
/// # 返回值
///
/// 是同一个文件返回 `true`
pub(super) fn same_path(old: &Path, new: &Path) -> bool {
    if cfg!(windows) {
        old.to_string_lossy().to_lowercase() == new.to_string_lossy().to_lowercase()
    } else {
        old == new
    }
}

/// 把旧文件移进回收站
///
/// - `path`: 旧文件路径
///
/// # 返回值
///
/// 删除失败返回错误串
pub(super) fn old_delete(path: &Path) -> Result<(), String> {
    ModObj {
        file: path.to_path_buf(),
        ..Default::default()
    }
    .delete()
    .map_err(|err| err.to_string())
}
