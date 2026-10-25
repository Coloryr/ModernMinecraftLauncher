//! 更新记录：读与清
//!
//! 记录落在实例的更新表里，给「已更新过什么」一个可回看的清单。

use crate::dtos::{ModUpdateProjectDto, ModUpdateRecordDto};

use crate::windows::resource::parse_instance;

/// 读模组更新记录
///
/// 记录落在实例目录的 `mod_update.json`，按项目编号分表，由「下载更新」与「替换版本」
/// 在换文件成功后追加。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`ModUpdateProjectDto`]；**每个项目内部新的在前，项目之间按各自最新一条倒序**。
/// 实例不存在返回错误串，从没更新过时返回空表
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_mods_update_log(uuid: String) -> Result<Vec<ModUpdateProjectDto>, String> {
    let instance = parse_instance(&uuid)?;
    let list = instance.read().unwrap().read_mod_update();

    let mut groups: Vec<ModUpdateProjectDto> = list
        .into_iter()
        .map(|(project_id, item)| {
            // 文件里是旧的在前，给界面倒过来
            let mut update: Vec<ModUpdateRecordDto> = item
                .update
                .into_iter()
                .map(|record| ModUpdateRecordDto {
                    file: record.file,
                    old_version: record.old_version,
                    version: record.version,
                    old_file_id: record.old_file_id,
                    file_id: record.file_id,
                    time: record.time.timestamp_millis(),
                    sha1: record.sha1,
                })
                .collect();
            update.reverse();
            ModUpdateProjectDto {
                project_id,
                name: item.name,
                update,
            }
        })
        .collect();
    // 项目之间按最新一条的时间倒序；没有记录的项目排最后
    groups.sort_by_key(|group| std::cmp::Reverse(group.update.first().map_or(0, |item| item.time)));

    Ok(groups)
}

/// 删掉一条模组更新记录
///
/// 下标在该项目的更新历史里数，按 [`resource_mods_update_log`] 的**返回顺序**，
/// 即 0 是最近那一条。记录没有唯一 id，用下标定位；弹窗一次读一份、删完立刻重读，
/// 不存在并发改表。
///
/// - `uuid`: 实例 uuid
/// - `project_id`: 项目编号，即 `mod_update.json` 里的键
/// - `index`: 该项目内的记录下标，从 0 起，新的在前
///
/// # 返回值
///
/// 实例不存在或写盘失败返回错误串；项目不存在或下标越界时什么都不做
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_mods_update_log_remove(
    uuid: String,
    project_id: String,
    index: usize,
) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let game = instance.read().unwrap();
    let mut list = game.read_mod_update();
    let Some(entry) = list.get_mut(&project_id) else {
        return Ok(());
    };
    if index >= entry.update.len() {
        return Ok(());
    }
    // 界面按"新的在前"给下标，文件里是旧的在前
    let at = entry.update.len() - 1 - index;
    entry.update.remove(at);
    // 这个项目没有记录了就整项摘掉，别在界面上留一个空组
    let empty = entry.update.is_empty();
    if empty {
        list.remove(&project_id);
    }

    game.save_mod_update(&list).map_err(|err| err.to_string())
}
