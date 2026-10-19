//! 把外部文件／目录整体拷进实例
//!
//! 支持目录与压缩包，冲突按公共口径改名或跳过。

use std::path::{Path, PathBuf};

use mml_game::GameInstance;
use mml_sys::path_helper;

use crate::dtos::ResourceDropDto;
use crate::err_keys;

use crate::windows::resource::{KIND_DATAPACKS, KIND_SAVES, datapack_dir, dir_of, parse_instance};
use super::common::{MODE_CHECK, MODE_SKIP, MODE_RENAME, unique_name, file_name};
use super::saves::{import_saves};


/// 导入实现，同步执行，在阻塞线程里跑
///
/// - `instance`: 实例句柄
/// - `kind`: 分类
/// - `paths`: 拖入项的绝对路径
/// - `mode`: 同名冲突的处理方式
/// - `parent`: 数据包页的存档目录名
///
/// # 返回值
///
/// 与 [`resource_import_files`] 同一份结果；没有可用文件时返回 `err.dropNoFile`
pub(super) fn import(
    instance: GameInstance,
    kind: &str,
    paths: &[String],
    mode: &str,
    parent: Option<String>,
) -> Result<ResourceDropDto, String> {
    // 只认文件：拖进来的目录一律跳过，前端也不会为目录显示导入遮罩，这里是兜底
    let files: Vec<PathBuf> = paths
        .iter()
        .map(PathBuf::from)
        .filter(|item| item.is_file())
        .collect();
    if files.is_empty() {
        return Err(String::from(err_keys::DROP_NO_FILE));
    }

    // 取一份设置快照后**立刻放锁**，与 mod.rs 的 block_on_instance 同一口径
    let game = instance.read().unwrap().clone();
    match kind {
        KIND_SAVES => import_saves(&game, &files, mode),
        // 数据包挂在某个存档下：落点要配合 parent 这个存档目录名，没传就报错
        KIND_DATAPACKS => {
            let dir = datapack_dir(&game, parent.as_deref().unwrap_or_default())?;
            copy_into(&dir, &files, mode)
        }
        // 其余分类按目录名取，不认识的在 dir_of 里报错
        _ => {
            let dir = dir_of(&game, kind)?;
            copy_into(&dir, &files, mode)
        }
    }
}

/// 把文件复制进指定目录，与实例无关，便于单测
///
/// - `dir`: 目标目录，不存在时会创建
/// - `files`: 待复制的文件路径
/// - `mode`: 同名冲突的处理方式
///
/// # 返回值
///
/// 本次结果：`imported` / `skipped` / `conflicts` 三个名单
///
/// 只认文件：传进来的目录一律跳过，前端不会为目录显示导入遮罩，这里是兜底。
pub(super) fn copy_into(dir: &Path, files: &[PathBuf], mode: &str) -> Result<ResourceDropDto, String> {
    let mut result = ResourceDropDto::default();
    if mode != MODE_CHECK {
        path_helper::create_dir_all(dir).map_err(|err| err.to_string())?;
    }

    for file in files.iter().filter(|item| item.is_file()) {
        let Some(name) = file_name(file) else {
            continue;
        };
        let target = dir.join(&name);
        let exists = target.exists();
        if mode == MODE_CHECK {
            if exists {
                result.conflicts.push(name);
            }
            continue;
        }
        if exists {
            match mode {
                MODE_SKIP => {
                    result.skipped.push(name);
                    continue;
                }
                MODE_RENAME => {
                    let unique = unique_name(dir, &name);
                    path_helper::copy_file(file, &unique.path).map_err(|err| err.to_string())?;
                    result.imported.push(unique.name);
                    continue;
                }
                // 剩下的都直接覆盖，含 overwrite 与没传清楚的兜底
                _ => {}
            }
        }
        path_helper::copy_file(file, &target).map_err(|err| err.to_string())?;
        result.imported.push(name);
    }
    Ok(result)
}

/// 拖拽导入：把拖入的文件放进当前分类对应的目录
///
/// - `uuid`: 实例 uuid
/// - `kind`: 分类，与窗口左侧分类对应的后端 kind 串一致，如 `shaders` 页传 `shaderpacks`
/// - `paths`: 拖入项的绝对路径，目录会被跳过
/// - `mode`: 同名冲突的处理方式，取值 `check` / `overwrite` / `skip` / `rename`；
///   `check` 只预检不落盘
/// - `parent`: 只有数据包那一页要传，即存档目录名，落点 `saves/<parent>/datapacks`
///
/// # 返回值
///
/// 本次结果：`imported` / `skipped` / `conflicts` 三个名单，`check` 模式下只有
/// `conflicts`；拖入项全是目录时返回 `err.dropNoFile`
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_import_files(
    uuid: String,
    kind: String,
    paths: Vec<String>,
    mode: String,
    parent: Option<String>,
) -> Result<ResourceDropDto, String> {
    // 读的过程中删实例会被拦住，见 resource::instance_busy
    let _op = crate::windows::resource::model::begin_if_open(&uuid);
    let instance = parse_instance(&uuid)?;
    // 复制 / 解压都是磁盘活：放阻塞线程，别占住主线程，与 add_import_* 同一条路子
    tauri::async_runtime::spawn_blocking(move || import(instance, &kind, &paths, &mode, parent))
        .await
        .map_err(|err| err.to_string())?
}
