use std::{path::Path, thread, time::Duration};

use mml_names::i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType, FileSystemErrorData};

use mml_sys::path_helper;
use uuid::Uuid;

use crate::launcher_path::instance_path::{self};

use crate::*;

pub fn delete_instance(uuid: &Uuid) -> CoreResult<()> {
    let instance = get_instance(uuid).ok_or(ErrorType::ArgEmpty(ArgEmptyData::UUID))?;

    // 启动器自己的删除不走外部监听，避免移除记录与删除命令各广播一次
    let _watch = path_watch::WatchPause::new();

    // 先删实例文件并移入回收站，失败则保留实例数据，避免 UI 消失但文件还在
    instance.read().unwrap().delete_files()?;

    remove_instance_record(uuid);
    Ok(())
}

/// 从实例列表移除记录，不删除文件，实例目录监视用
///
/// - `uuid`: 实例 UUID
pub(crate) fn remove_instance_record(uuid: &Uuid) {
    // 删除前把名字留一份，统计窗口里已删除的实例还得靠它显示
    if let Some(game) = get_instance(uuid) {
        let name = game.read().unwrap().name.clone();
        game_count::remember_name(uuid, &name);
    }
    INSTANCES.write().unwrap().remove(uuid);
    // 从分组表里摘掉它，但**不删空分组**：空分组是合法状态。
    // 这里以前有一句"顺手清掉空分组"，正是"空分组留不住"的原因
    game_group::forget_instance(uuid);

    invoke_change(InstanceChange::RemoveInstance(*uuid));
}

/// 给实例目录改名，遇到短暂占用时重试
///
/// 杀毒 / 索引服务会在目录刚建好时短暂持有句柄，第一次改名可能报拒绝访问或共享冲突；
/// 这类错误过几十毫秒就消失，重试几次即可，别把瞬时占用报成改名失败。
///
/// - `from`: 原目录
/// - `to`: 新目录
///
/// # 返回值
///
/// 改名成功返回 `Ok(())`；重试后仍失败返回最后一次的错误
fn rename_instance_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    const RETRY: u32 = 6;

    let mut last = None;
    for attempt in 0..RETRY {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(err) => {
                // 5 = ACCESS_DENIED，32 = SHARING_VIOLATION，33 = LOCK_VIOLATION
                let transient = matches!(err.raw_os_error(), Some(5) | Some(32) | Some(33));
                last = Some(err);
                if !transient || attempt + 1 == RETRY {
                    break;
                }
                thread::sleep(Duration::from_millis(30 * u64::from(attempt + 1)));
            }
        }
    }
    Err(last.unwrap())
}

/// 重命名实例，名字与实例目录一起改
///
/// 新名字与其他实例重复时拒绝；成功后发一次变更事件刷新前端列表。
///
/// - `uuid`: 实例 UUID
/// - `name`: 新名字
///
/// # 返回值
///
/// 成功返回 `Ok(())`；名字为空、不合法、重复或目录改名失败返回对应错误
pub fn rename_instance(uuid: &Uuid, name: &str) -> CoreResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    name_check::check_name(name)?;

    let instance = get_instance(uuid).ok_or(ErrorType::ArgEmpty(ArgEmptyData::UUID))?;

    // 新名字与别的实例重复时拒绝，自己保持原名除外
    if have_instance_name(name) && instance.read().unwrap().name != name {
        return Err(ErrorType::InstanceNameExists(name.to_string()));
    }

    let old_base = instance.read().unwrap().get_base_path();

    // 实例目录跟着改名，目录名由名字派生，改名成功后再更新名字并保存
    let new_dir = path_helper::replace_path_name(name);
    let new_base = old_base
        .parent()
        .map(|p| p.join(&new_dir))
        .unwrap_or_else(|| old_base.clone());
    if old_base != new_base && old_base.exists() {
        rename_instance_dir(&old_base, &new_base).map_err(|e| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: old_base,
                error: e.to_string(),
            })
        })?;
    }

    let mut obj = instance.write().unwrap();
    obj.name = name.to_string();
    obj.dir = new_dir;
    obj.save();

    drop(obj);
    game_count::remember_name(uuid, name);

    // 重命名不改分组归属，分组表按 uuid 记录、与名字无关，
    // 但要让前端刷新那一行，所以照旧发一次带当前分组的变更事件
    invoke_change(InstanceChange::MoveGroup(*uuid, get_instance_group(uuid)));
    Ok(())
}

/// 复制实例：整份实例目录复制到新名字下，再登记成新实例
///
/// 复制的是目录的完整内容（游戏文件、模组、存档、配置），新实例沿用源实例的分组；
/// 新实例的 uuid 重新生成，目录名由新名字派生。
///
/// - `uuid`: 源实例 UUID
/// - `name`: 新实例名，前后空白会被去掉
///
/// # 返回值
///
/// 返回新实例句柄；名字为空、不合法、重名、源实例不存在或复制失败返回对应错误
pub fn copy_instance(uuid: &Uuid, name: &str) -> CoreResult<GameInstance> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }
    name_check::check_name(name)?;
    if have_instance_name(name) {
        return Err(ErrorType::InstanceNameExists(name.to_string()));
    }

    let instance = get_instance(uuid).ok_or(ErrorType::ArgEmpty(ArgEmptyData::UUID))?;
    let (src, mut obj) = {
        let src = instance.read().unwrap();
        (src.get_base_path(), src.clone())
    };

    let dir = path_helper::replace_path_name(name);
    let dest = instance_path::get_instance_dir().join(&dir);
    if dest.exists() {
        return Err(ErrorType::InstanceNameExists(name.to_string()));
    }

    // 监视一直停到新实例登记完成：中途恢复的话，复制出来的目录会被当成外部导入，
    // 带着源实例的 uuid 再登记一遍
    let _watch = path_watch::WatchPause::new();
    let copied = path_helper::copy_dir(&src, &dest);
    if let Err(e) = copied {
        // 失败留下的半份目录要清掉：里面已经有一份 game.json，下次启动会被当成实例导入
        let _ = std::fs::remove_dir_all(&dest);
        return Err(e);
    }

    obj.uuid = Uuid::new_v4();
    obj.name = name.to_string();
    obj.dir = dir;
    obj.save();

    let game = add_to_group(obj, get_instance_group(uuid));
    Ok(game)
}

/// 按图标字节判断扩展名；GIF 原样保留，其余常见格式也给出真实扩展
///
/// - `data`: 图标原始字节
///
/// # 返回值
///
/// `gif` / `png` / `jpg` / `webp`；认不出时按 png 兼容旧行为
pub(crate) fn icon_extension(data: &[u8]) -> &'static str {
    if data.starts_with(b"GIF8") {
        "gif"
    } else if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "webp"
    } else {
        "png"
    }
}
