//! 实例目录文件监视
//!
//! 监听实例根目录的创建 / 删除事件，基于 `notify` crate，
//! 供实例列表在目录被外部改动时刷新。

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self},
    },
    thread,
    time::Duration,
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};
use notify::{EventKind, RecursiveMode, Watcher};

use crate::error::task_err;
use crate::launcher_path::instance_path;

/// 监听是否生效开关
static ENABLE_WATCHER: AtomicBool = AtomicBool::new(false);
/// 暂停层数：启动器自己增删实例时压住监听，避免把自己的操作当成外部改动
static PAUSE_DEPTH: AtomicUsize = AtomicUsize::new(0);

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
        // watcher 一旦 drop 就停止监听，必须把它留在监听线程里
        let _watcher = watcher;
        loop {
            for event in &rx {
                match event {
                    Ok(event) => {
                        if !ENABLE_WATCHER.load(Ordering::Acquire)
                            || PAUSE_DEPTH.load(Ordering::Acquire) > 0
                        {
                            continue;
                        }

                        // Windows 的 ReadDirectoryChangesW 不区分文件与目录，
                        // 事件统一是 Create(Any) / Remove(Any)，不能只认 Folder
                        match event.kind {
                            EventKind::Create(_) => {
                                // 新文件夹自动载入为实例，game.json 尚未写出时跳过；
                                // 本启动器创建实例时监听已暂停，uuid 已存在的忽略
                                for path in &event.paths {
                                    load_created(path);
                                }
                            }
                            EventKind::Remove(_) => {
                                // 文件夹被外部删除时，把对应实例从列表移除；
                                // 本启动器删除实例时记录已先行更新，这里匹配不到即跳过
                                for path in &event.paths {
                                    remove_deleted(path);
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

/// 载入外部新建的实例目录
///
/// - `path`: 新建的目录；不是实例目录或 game.json 还没写出时跳过
fn load_created(path: &Path) {
    if let Some(obj) = instance_path::load_instance(path)
        && crate::get_instance(&obj.uuid).is_none()
    {
        // 外部新建的实例不知道原分组，落默认分组；uuid 冲突时登记会换新值
        let uuid = {
            let game = crate::add_to_group(obj, None);
            game.read().unwrap().uuid
        };
        crate::invoke_watch_change(crate::InstanceWatchChange::Add(uuid));
    }
}

/// 外部删除实例目录后摘掉列表记录
///
/// - `path`: 被删除的目录
///
/// 目录还在时不动：事件可能晚于同名目录重建，误删会把刚建好的实例丢掉。
fn remove_deleted(path: &Path) {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let uuid = crate::get_instances().iter().find_map(|item| {
        let obj = item.read().unwrap();
        (obj.dir == name).then_some(obj.uuid)
    });
    let Some(uuid) = uuid else {
        return;
    };

    let missing = crate::get_instance(&uuid)
        .map(|game| !game.read().unwrap().get_base_path().exists())
        .unwrap_or(false);
    if missing {
        crate::remove_instance_record(&uuid);
        crate::invoke_watch_change(crate::InstanceWatchChange::Remove(uuid));
    }
}

/// 开始监听
pub(crate) fn start_watch() {
    ENABLE_WATCHER.store(true, Ordering::Release);
}

/// 暂停监听的守卫，离开作用域自动恢复
///
/// 用层数而不是布尔：调用方可能嵌套，先退出的那一层不能把监听提前打开。
pub(crate) struct WatchPause;

impl WatchPause {
    /// 压住监听
    pub(crate) fn new() -> Self {
        PAUSE_DEPTH.fetch_add(1, Ordering::AcqRel);
        Self
    }
}

impl Drop for WatchPause {
    fn drop(&mut self) {
        PAUSE_DEPTH.fetch_sub(1, Ordering::AcqRel);
    }
}
