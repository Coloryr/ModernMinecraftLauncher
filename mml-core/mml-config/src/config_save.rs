//! 配置文件后台保存模块
//!
//! 本模块实现了一个基于信号量的后台保存线程，用于将配置对象的
//! 持久化操作异步化，避免频繁的磁盘 IO 阻塞主线程。
//!
//! # 工作原理
//!
//! 1. [`save()`] 将序列化后的 JSON 加入队列，并通过信号量唤醒保存线程
//! 2. 同一个 UUID 的新任务会替换队列中的旧任务，实现去重
//! 3. 保存线程在后台循环等待信号量，被唤醒后批量执行保存
//! 4. 调用 [`stop()`] 后线程退出，退出前执行最后一次保存
//!
//! # 启动与停止
//!
//! - [`start()`] — 程序初始化时调用，启动后台保存线程
//! - [`stop()`] — 程序退出时调用，安全停止线程并执行最终保存

use std::{
    mem,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{Builder, JoinHandle},
};

use mml_base::serialize_tools;
use mml_log;
use mml_names::{
    i18,
    i18_items::{error_type::CoreResult, thread_type::ThreadType},
};
use mml_sys::path_helper;
use semrs::Semaphore;
use serde::Serialize;
use uuid::Uuid;

/// 单个配置保存任务
pub struct ConfigSaveObj {
    /// 序列化后的 JSON 字符串
    json: String,
    /// 目标文件路径
    file: PathBuf,
    /// 任务唯一标识，用于去重
    uuid: Uuid,
}

impl ConfigSaveObj {
    /// 创建保存任务
    ///
    /// - `obj`: 需要序列化保存的对象
    /// - `file`: 目标文件路径
    /// - `uuid`: 任务唯一标识，相同 uuid 的后继任务会替换旧任务
    ///
    /// # 返回值
    ///
    /// 序列化成功返回任务对象，失败返回相应错误
    pub fn new<T: Serialize>(obj: &T, file: PathBuf, uuid: Uuid) -> CoreResult<Self> {
        Self::build(obj, file, uuid, true)
    }

    /// 创建**紧凑**序列化的保存任务（不缩进）
    ///
    /// - `obj`: 需要序列化保存的对象
    /// - `file`: 目标文件路径
    /// - `uuid`: 任务唯一标识，相同 uuid 的后继任务会替换旧任务
    ///
    /// # 返回值
    ///
    /// 序列化成功返回任务对象，失败返回相应错误
    pub fn new_compact<T: Serialize>(obj: &T, file: PathBuf, uuid: Uuid) -> CoreResult<Self> {
        Self::build(obj, file, uuid, false)
    }

    /// 按是否缩进构造任务
    fn build<T: Serialize>(obj: &T, file: PathBuf, uuid: Uuid, pretty: bool) -> CoreResult<Self> {
        let json = if pretty {
            serialize_tools::json_to_string(obj)?
        } else {
            serialize_tools::json_to_string_compact(obj)?
        };

        Ok(ConfigSaveObj { json, file, uuid })
    }

    /// 执行文件写入
    ///
    /// # 返回值
    ///
    /// 写盘成功返回 `Ok(())`，失败返回相应错误
    pub fn save(&self) -> CoreResult<()> {
        path_helper::write_text(&self.file, &self.json)
    }
}

/// 全局保存任务队列
static QUEUE: std::sync::LazyLock<Mutex<SaveQueue>> =
    std::sync::LazyLock::new(|| Mutex::new(SaveQueue::default()));

/// 保存队列及导入后禁止回写的文件集合。
#[derive(Default)]
struct SaveQueue {
    /// 等待写入的配置快照。
    pending: Vec<ConfigSaveObj>,
    /// 导入完成后由新进程接管的文件，本进程不再写入。
    replaced: std::collections::HashSet<PathBuf>,
}

/// 已替换的文件及调用方需要的结果。
pub struct ReplacedFiles<T> {
    /// 成功覆盖的文件路径。
    pub files: Vec<PathBuf>,
    /// 文件替换的返回数据。
    pub value: T,
}
/// 后台保存线程是否运行中
static IS_RUN: AtomicBool = AtomicBool::new(true);
/// 信号量，用于唤醒保存线程
static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
/// 后台保存线程句柄，stop 时 join，确保最后一次保存落盘后才返回
static HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
/// 文件替换与后台保存共用的写入屏障。
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// 将对象加入保存队列
///
/// 相同 `uuid` 的旧任务会被移除，避免重复保存同一个配置；加入后通过信号量唤醒后台保存线程。
///
/// - `uuid`: 任务标识，相同标识会去重
/// - `obj`: 需要保存的对象
/// - `file`: 目标文件路径
pub fn save<T: Serialize>(uuid: Uuid, obj: &T, file: impl AsRef<Path>) {
    enqueue(ConfigSaveObj::new(obj, file.as_ref().to_path_buf(), uuid).unwrap());
}

/// 将对象以**紧凑** JSON 加入保存队列，与 [`save`] 的区别只有不缩进
///
/// 条目多的数据文件（收藏、分组这类按条写盘的东西）用它：缩进会让文件大出好几倍，
/// 而这些文件没人手读。
///
/// - `uuid`: 任务标识，相同标识会去重
/// - `obj`: 需要保存的对象
/// - `file`: 目标文件路径
pub fn save_compact<T: Serialize>(uuid: Uuid, obj: &T, file: impl AsRef<Path>) {
    enqueue(ConfigSaveObj::new_compact(obj, file.as_ref().to_path_buf(), uuid).unwrap());
}

/// 把一个任务塞进队列并唤醒保存线程
///
/// - `task`: 已经序列化好的保存任务
fn enqueue(task: ConfigSaveObj) {
    let uuid = task.uuid;
    let mut queue = QUEUE.lock().unwrap();
    if !queue.replaced.is_empty() && queue.replaced.contains(&file_identity(&task.file)) {
        return;
    }
    queue.pending.retain(|item| item.uuid != uuid);
    queue.pending.push(task);

    SEM.get().unwrap().up();
}

/// 执行一次保存：取出队列中所有任务并写入磁盘
fn save_now() {
    let _guard = WRITE_LOCK.lock().unwrap();
    flush_queue();
}

fn flush_queue() {
    let items = {
        let mut queue = QUEUE.lock().unwrap();
        mem::take(&mut queue.pending)
    };
    for save_obj in items {
        if let Err(err) = save_obj.save() {
            mml_log::error_type(err);
        }
    }
}

/// 写完旧配置后独占替换文件，成功覆盖的文件在重启前不再接受旧快照。
/// - `action`: 替换操作，失败时须自行回滚，不得等待保存线程。
/// # 返回值
/// 替换结果；失败时继续接受原有配置保存。
pub fn replace_before_restart<T, E>(
    action: impl FnOnce() -> Result<ReplacedFiles<T>, E>,
) -> Result<T, E> {
    let _guard = WRITE_LOCK.lock().unwrap();
    flush_queue();
    let result = action()?;
    let mut queue = QUEUE.lock().unwrap();
    queue
        .replaced
        .extend(result.files.iter().map(|path| file_identity(path)));
    let SaveQueue { pending, replaced } = &mut *queue;
    pending.retain(|task| !replaced.contains(&file_identity(&task.file)));
    Ok(result.value)
}

fn file_identity(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// 启动后台配置保存线程
///
/// 创建一个命名线程，循环等待信号量，被唤醒时执行批量保存。
/// 应在程序初始化阶段调用一次。
pub fn start() {
    SEM.get_or_init(|| Arc::new(Semaphore::new(0)));

    let handle = Builder::new()
        .name(i18::get_thread(ThreadType::ConfigSaveThread))
        .spawn(|| {
            while IS_RUN.load(Ordering::Acquire) {
                SEM.get().unwrap().down();

                save_now();
            }

            // 线程退出前执行最后一次保存
            save_now();
        })
        .unwrap();

    *HANDLE.lock().unwrap() = Some(handle);
}

/// 停止后台保存线程
///
/// 设置运行标志为 false，唤醒线程使其退出，并阻塞等待线程结束——
/// 线程退出前会执行最后一次保存，因此本函数返回即保证队列内任务全部落盘。
pub fn stop() {
    // 未调用过 start，信号量未初始化，无事可做
    let Some(sem) = SEM.get() else {
        return;
    };
    IS_RUN.store(false, Ordering::Release);
    // 唤醒线程，使其离开 down() 检查 IS_RUN 并退出
    sem.up();
    // 等待线程执行完最后一次保存
    if let Some(handle) = HANDLE.lock().unwrap().take() {
        let _ = handle.join();
    }
}

#[cfg(test)]
mod replacement_tests {
    use super::*;

    #[test]
    fn queued_snapshot_must_not_undo_file_replacement() {
        SEM.get_or_init(|| Arc::new(Semaphore::new(0)));
        let dir = mml_testutil::temp_path(format!("replacement-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("game.json");
        let other = dir.join("accounts.json");
        replace_before_restart(|| {
            save(Uuid::new_v4(), &"stale", &path);
            save(Uuid::new_v4(), &"account update", &other);
            std::fs::write(&path, "\"imported\"").unwrap();
            Ok::<_, ()>(ReplacedFiles {
                files: vec![path.canonicalize().unwrap()],
                value: (),
            })
        })
        .unwrap();
        save(Uuid::new_v4(), &"late stale snapshot", &path);
        save_now();
        let actual = std::fs::read_to_string(&path).unwrap();
        assert_eq!(actual, "\"imported\"");
        assert_eq!(
            std::fs::read_to_string(&other).unwrap(),
            "\"account update\""
        );

        let result = replace_before_restart::<(), _>(|| {
            save(Uuid::new_v4(), &"preserved after rollback", &other);
            Err("replacement failed")
        });
        assert!(result.is_err());
        save_now();
        assert_eq!(
            std::fs::read_to_string(&other).unwrap(),
            "\"preserved after rollback\""
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
