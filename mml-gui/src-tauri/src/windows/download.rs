//! 下载窗口
//!
//! 对接 `mml_downloader`：
//! - 实现 [`DownloadGuiHook`] 作为下载器的 UI 回调，把任务 / 文件状态变化转发为 download-task /
//!   download-item 事件，同时维护线程状态表，记当前文件、状态与速度采样
//! - `download_get_status` 查询任务 + 线程 + 总体速度快照，前端按固定间隔轮询
//! - `download_pause_all` / `download_resume_all` / `download_cancel_all` 全局控制下载
//!
//! 窗口本身由 `windows` 的 `create_window` 统一创建。

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, RwLock},
    time::Instant,
};

use mml_downloader::{
    DownloadTaskState,
    download_item::{DownloadItem, DownloadItemState},
};
use tauri::{AppHandle, Emitter};

use crate::{
    dtos::{
        DownloadItemEvent, DownloadStatusDto, DownloadTaskDto, DownloadTaskEvent, DownloadThreadDto,
    },
    listens,
};

/// 速度采样最小间隔，单位秒；间隔更短的回调不重算速度，避免抖动
const SPEED_WINDOW: f64 = 0.25;
/// 速度过期时间，单位秒；超过此时长没有进度回调，视为线程停滞，速度按 0 计
const SPEED_EXPIRE: f64 = 1.5;

/// 下载线程运行状态，回调写入、命令读取
struct ThreadInfo {
    /// 当前下载项，进度与字节数都取自它
    item: Arc<DownloadItem>,
    /// 当前文件名
    name: String,
    /// 状态 ID：`wait` / `download` / `getinfo` / `pause` / `init` / `action` / `done` / `error`
    state: String,
    /// 上次速度采样的字节数
    sample_bytes: u64,
    /// 上次速度采样时间
    sample_at: Instant,
    /// 采样得到的速度，单位字节每秒
    speed: f64,
}

impl ThreadInfo {
    /// 新建线程状态，速度采样起点取当前进度
    ///
    /// - `item`: 当前下载项
    /// - `name`: 当前文件名
    /// - `state`: 状态 ID
    ///
    /// # 返回值
    ///
    /// 采样起点已设好的线程状态
    fn new(item: Arc<DownloadItem>, name: String, state: String) -> Self {
        ThreadInfo {
            sample_bytes: item.get_now_size(),
            item,
            name,
            state,
            sample_at: Instant::now(),
            speed: 0.0,
        }
    }

    /// 用最新进度刷新速度采样：换文件时重置，判据是字节回退或文件名变化
    ///
    /// - `now`: 本次回调的时刻，与上次采样时刻的差就是速度分母
    fn sample(&mut self, now: Instant) {
        let bytes = self.item.get_now_size();
        if bytes < self.sample_bytes {
            // 换到新文件，断点续传的重置也算，重新起算
            self.sample_bytes = bytes;
            self.sample_at = now;
            self.speed = 0.0;
            return;
        }

        let elapsed = now.duration_since(self.sample_at).as_secs_f64();
        if elapsed >= SPEED_WINDOW {
            self.speed = (bytes - self.sample_bytes) as f64 / elapsed;
            self.sample_bytes = bytes;
            self.sample_at = now;
        }
    }
}

/// 下载线程状态表：线程序号 → 状态
static THREADS: LazyLock<RwLock<HashMap<u32, ThreadInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 下载线程 → 最近一次上报的文件名与状态两项
///
/// 下载器按 HTTP 块粒度回调 `update`，这里去重：同一线程
/// 文件名与状态均未变化时不重复发事件。
type LastItem = HashMap<u32, (String, u32)>;

/// 下载器 UI 回调桥接，把回调转发为 Tauri 事件
pub struct DownloadGuiHook {
    /// 应用句柄，事件经它转发到前端
    app: AppHandle,
    /// 各线程最近一次上报的文件名与状态，用于事件去重
    last: Mutex<LastItem>,
}

impl DownloadGuiHook {
    /// 创建回调桥接
    ///
    /// - `app`: 应用句柄，事件经它转发到前端
    ///
    /// # 返回值
    ///
    /// 去重表为空的桥接实例
    pub fn new(app: AppHandle) -> Self {
        DownloadGuiHook {
            app,
            last: Mutex::new(HashMap::new()),
        }
    }

    /// 文件名：优先用 FileItemObj.name，为空则取路径末段
    fn item_name(file: &Arc<DownloadItem>) -> String {
        if !file.base.name.is_empty() {
            return file.base.name.clone();
        }
        file.base
            .file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// 下载项状态 → 前端状态 ID
    fn state_id(state: DownloadItemState) -> String {
        match state {
            DownloadItemState::Wait => "wait",
            DownloadItemState::Download => "download",
            DownloadItemState::GetInfo => "getinfo",
            DownloadItemState::Pause => "pause",
            DownloadItemState::Init => "init",
            DownloadItemState::Action => "action",
            DownloadItemState::Done => "done",
            DownloadItemState::Error => "error",
        }
        .into()
    }

    /// 更新线程状态表，含速度采样
    ///
    /// - `thread`: 线程序号
    /// - `file`: 该线程当前的下载项
    /// - `name`: 当前文件名，换文件时用来重置采样起点
    /// - `state`: 状态 ID
    fn track(thread: u32, file: &Arc<DownloadItem>, name: &str, state: &str) {
        let now = Instant::now();
        let mut map = THREADS.write().unwrap();
        match map.get_mut(&thread) {
            Some(info) => {
                // 换文件时重置采样起点
                if info.name != name {
                    info.speed = 0.0;
                    info.sample_at = now;
                    info.sample_bytes = file.get_now_size();
                }
                info.item = file.clone();
                info.name = name.to_string();
                info.state = state.to_string();
                info.sample(now);
            }
            None => {
                let mut info = ThreadInfo::new(file.clone(), name.to_string(), state.to_string());
                info.sample(now);
                map.insert(thread, info);
            }
        }
    }
}

impl mml_downloader::IDownloadGui for DownloadGuiHook {
    /// 单文件状态回调：刷新线程状态表 → 转发资源下载进度 → 去重后发事件
    ///
    /// - `thread`: 线程序号
    /// - `file`: 该线程当前的下载项
    fn update(&self, thread: u32, file: &Arc<DownloadItem>) {
        let name = Self::item_name(file);
        let state = file.get_state();
        let state_num = state.state_to_int();
        let state_id = Self::state_id(state);

        // 线程状态表始终刷新，速度采样需要每次回调
        Self::track(thread, file, &name, &state_id);

        // 资源下载进度，供添加资源窗口顶部进度条用：按目标文件路径匹配任务表
        crate::windows::add_resource::on_download_item(file, &self.app);

        // 事件按文件名与状态去重，避免按块刷屏
        {
            let mut last = self.last.lock().unwrap();
            match last.get(&thread) {
                Some((old_name, old_state)) if *old_name == name && *old_state == state_num => {
                    return;
                }
                _ => {
                    last.insert(thread, (name.clone(), state_num));
                }
            }
        }

        emit_download_item(
            &self.app,
            DownloadItemEvent {
                thread,
                name,
                state: state_id,
            },
        );
    }

    /// 任务列表变化回调：任务增删 / 进度更新转发为事件
    ///
    /// - `state`: 本次变化，含新增任务、移除任务与任务进度更新三种
    fn update_task(&self, state: DownloadTaskState) {
        let event = match state {
            DownloadTaskState::AddTask(id) => DownloadTaskEvent {
                r#type: "add".into(),
                id,
                progress: 0.0,
            },
            DownloadTaskState::RemoveTask(id) => DownloadTaskEvent {
                r#type: "remove".into(),
                id,
                progress: 0.0,
            },
            DownloadTaskState::UpdateTask(obj) => DownloadTaskEvent {
                r#type: "update".into(),
                id: obj.id,
                progress: obj.progress,
            },
        };
        emit_download_task(&self.app, event);
    }
}

/// 任务列表事件
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 任务增删或进度变化
#[gui_macros::emit]
pub fn emit_download_task(app: &AppHandle, event: DownloadTaskEvent) {
    let _ = app.emit(listens::DOWNLOAD_TASK, event);
}

/// 下载线程文件状态事件
///
/// - `app`: 应用句柄，广播用它
/// - `event`: 某个线程当前文件与状态的变化
#[gui_macros::emit]
pub fn emit_download_item(app: &AppHandle, event: DownloadItemEvent) {
    let _ = app.emit(listens::DOWNLOAD_ITEM, event);
}

/// 查询下载状态快照：任务列表 + 线程状态 + 总体速度
///
/// # 返回值
///
/// 任务列表、各线程状态、总体速度与全局暂停标志；速度单位字节每秒
#[tauri::command]
pub fn download_get_status() -> DownloadStatusDto {
    let tasks: Vec<DownloadTaskDto> = mml_downloader::get_tasks()
        .into_iter()
        .map(|t| DownloadTaskDto {
            id: t.id,
            total: t.total,
            completed: t.completed,
            failed: t.failed,
            all_bytes: t.all_bytes,
            now_bytes: t.now_bytes,
            elapsed_ms: t.elapsed_ms,
            paused: t.paused,
        })
        .collect();

    let now = Instant::now();
    let paused = mml_downloader::is_paused_all();
    let mut speed = 0u64;
    let mut threads: Vec<DownloadThreadDto> = Vec::new();

    for (thread, info) in THREADS.read().unwrap().iter() {
        // 长时间没有进度回调时速度归零，停滞 / 暂停 / 已结束都算
        let idle = now.duration_since(info.sample_at).as_secs_f64() > SPEED_EXPIRE;
        let thread_speed = if idle { 0.0 } else { info.speed };
        speed += thread_speed as u64;

        // 全局暂停期间，工作中的线程状态同步显示为暂停
        let state = if paused && (info.state == "download" || info.state == "getinfo") {
            "pause".to_string()
        } else {
            info.state.clone()
        };

        threads.push(DownloadThreadDto {
            thread: *thread,
            name: info.name.clone(),
            state,
            progress: info.item.progress(),
            now_bytes: info.item.get_now_size(),
            all_bytes: info.item.get_all_size(),
            speed: thread_speed as u64,
        });
    }

    threads.sort_by_key(|t| t.thread);

    DownloadStatusDto {
        tasks,
        threads,
        speed,
        paused,
    }
}

/// 全局暂停：暂停所有下载任务，期间新增任务同样暂停；返回被暂停的任务数
///
/// # 返回值
///
/// 被暂停的任务数
#[tauri::command]
pub fn download_pause_all() -> usize {
    mml_downloader::pause_all()
}

/// 全局恢复：恢复所有下载任务并唤醒线程，返回被恢复的任务数
///
/// # 返回值
///
/// 被恢复的任务数
#[tauri::command]
pub fn download_resume_all() -> usize {
    mml_downloader::resume_all()
}

/// 全局停止：取消所有下载任务，返回被停止的任务数
///
/// # 返回值
///
/// 被停止的任务数
#[tauri::command]
pub fn download_cancel_all() -> usize {
    mml_downloader::cancel_all()
}
