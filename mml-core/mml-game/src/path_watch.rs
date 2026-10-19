//! 实例目录文件监视
//!
//! 监听实例根目录的创建 / 删除事件，基于 `notify` crate，
//! 供实例列表在目录被外部改动时刷新。

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self},
    },
    thread,
    time::Duration,
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};
use notify::{
    EventKind, RecursiveMode, Watcher,
    event::{CreateKind, RemoveKind},
};

use crate::launcher_path::instance_path;
use crate::error::{task_err};

/// 监听是否生效开关
static ENABLE_WATCHER: AtomicBool = AtomicBool::new(false);

/// 初始化实例目录监视
///
/// # 返回值
///
/// 成功返回 `Ok(())`；创建监听器失败返回对应错误
pub(crate) fn init_watch() -> CoreResult<()> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(tx).map_err(task_err)?;

    watcher
        .watch(
            &instance_path::get_instance_dir(),
            RecursiveMode::NonRecursive,
        )
        .map_err(task_err)?;

    thread::spawn(move || {
        loop {
            for event in &rx {
                match event {
                    Ok(event) => {
                        if !ENABLE_WATCHER.load(Ordering::Acquire) {
                            continue;
                        }

                        match event.kind {
                            EventKind::Create(CreateKind::Folder) => {
                                // 新文件夹自动载入为实例，game.json 尚未写出时跳过；
                                // 本启动器创建 / 改名实例时监听已暂停，uuid 已存在的忽略
                                for path in &event.paths {
                                    if let Some(obj) = instance_path::load_instance(path)
                                        && crate::get_instance(&obj.uuid).is_none()
                                    {
                                        // 外部新建的实例不知道原分组，落默认分组
                                        crate::add_to_group(obj, None);
                                    }
                                }
                            }
                            EventKind::Remove(RemoveKind::Folder) => {
                                // 文件夹被外部删除时，把对应实例从列表移除；
                                // 本启动器删除 / 改名实例时记录已先行更新，这里匹配不到即跳过
                                for path in &event.paths {
                                    let name = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();

                                    let uuid = crate::get_instances().iter().find_map(|item| {
                                        let obj = item.read().unwrap();
                                        (obj.dir == name).then_some(obj.uuid)
                                    });

                                    if let Some(uuid) = uuid {
                                        crate::remove_instance_record(&uuid);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        mml_log::error_type(ErrorType::FileSystemError(FileSystemErrorData {
                            path: instance_path::get_instance_dir(),
                            error: e.to_string(),
                        }));
                    }
                }
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    Ok(())
}

/// 开始监听，实例创建 / 删除期间暂停，完成后恢复
pub(crate) fn start_watch() {
    ENABLE_WATCHER.store(true, Ordering::Release);
}

/// 停止监听
pub(crate) fn stop_watch() {
    ENABLE_WATCHER.store(false, Ordering::Release);
}
