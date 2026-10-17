//! 整合包安装任务的共享注册表，添加实例窗口与下载整合包窗口共用
//!
//! 原先这段基础设施住在 `add_modpack.rs` 里，而 `add.rs` 本地压缩包导入也要用它 ——
//! 于是两个窗口模块互相 `use`，成了一个环。抽到这里之后依赖是单向的：
//! `add` → `modpack_task` ← `add_modpack`，两个窗口之间只剩"下载整合包用添加实例窗口的
//! 重名确认回调"这一条，方向是 `add_modpack` → `add`。
//!
//! 任务与窗口生命周期解耦：窗口关了任务照跑，进度通过 `add-modpack-status` 事件广播。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use mml_game::gui_hook::{AddModPackGui, AddModPackState, IAddModPackGui};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::dtos::add_modpack_dto::{ModPackStatusDto, ModPackTaskDto};
use crate::err_keys;
use crate::listens;
use crate::windows::add_resource::SourceInfo;

/// 进行中的整合包安装任务，键为 pid+fid，同键不重复安装
pub(super) static DOWNLOAD_NOW: LazyLock<RwLock<HashMap<SourceInfo, Arc<ModPackTask>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 任务主/子进度快照，各回调分别更新字段
pub(super) struct ModPackProgress {
    /// 安装阶段 ID，见 [`pack_state_id`]
    pub(super) state: String,
    /// 主进度：已完成数
    pub(super) now: u32,
    /// 主进度：总数
    pub(super) total: u32,
    /// 子任务说明文字
    pub(super) sub_text: Option<String>,
    /// 子任务进度：已完成数
    pub(super) sub_now: u32,
    /// 子任务进度：总数
    pub(super) sub_total: u32,
}

/// 一个整合包安装任务：进度 + 取消令牌 + 终态标记
///
/// 终态为 done / failed / cancelled 的任务保留在表里供前端展示，
/// 由 `add_modpack_clear_done` 清除。
pub(super) struct ModPackTask {
    /// 任务 ID，安装开始时生成，与最终实例 uuid 无关
    pub(super) uuid: Uuid,
    /// 整合包来源平台 ID
    pub(super) source: String,
    /// 整合包项目 ID
    pub(super) pid: String,
    /// 整合包文件 ID
    pub(super) fid: String,
    /// 显示名，安装开始时从列表缓存取
    pub(super) name: String,
    /// 这次任务在干什么，取值 `install` / `upgrade`；前端据此换提示文案
    pub(super) kind: String,
    /// 取消令牌
    pub(super) cancel: CancellationToken,
    /// 主/子进度快照
    pub(super) progress: Mutex<ModPackProgress>,
    /// 安装完成
    pub(super) done: AtomicBool,
    /// 安装失败
    pub(super) failed: AtomicBool,
    /// 已取消
    pub(super) cancelled: AtomicBool,
    /// 失败错误文案，failed 为 true 时有值
    pub(super) error: Mutex<Option<String>>,
    /// 安装出的实例 uuid，成功后回填
    pub(super) instance_uuid: Mutex<Option<String>>,
}

impl ModPackTask {
    /// 转前端 DTO，快照当前进度与终态
    ///
    /// # 返回值
    ///
    /// 任务此刻的跨 IPC 快照
    fn dto(&self) -> ModPackTaskDto {
        let progress = self.progress.lock().unwrap();
        ModPackTaskDto {
            uuid: self.uuid.to_string(),
            source: self.source.clone(),
            pid: self.pid.clone(),
            fid: self.fid.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            state: progress.state.clone(),
            now: progress.now,
            total: progress.total,
            sub_text: progress.sub_text.clone(),
            sub_now: progress.sub_now,
            sub_total: progress.sub_total,
            done: self.done.load(Ordering::Acquire),
            failed: self.failed.load(Ordering::Acquire),
            cancelled: self.cancelled.load(Ordering::Acquire),
            error: self.error.lock().unwrap().clone(),
            instance_uuid: self.instance_uuid.lock().unwrap().clone(),
        }
    }
}

/// 整合包安装任务总览快照，事件负载与查询返回共用
///
/// - `app`: 应用句柄，用它判断整合包窗口是否开着
///
/// # 返回值
///
/// 当前全部任务与窗口开闭状态
pub(super) fn build_status(app: &AppHandle) -> ModPackStatusDto {
    let map = DOWNLOAD_NOW.read().unwrap();
    ModPackStatusDto {
        window_open: app.get_webview_window("mml-add_modpack").is_some(),
        tasks: map.values().map(|task| task.dto()).collect(),
    }
}

/// 整合包安装任务总览事件，任务增删 / 进度变化 / 终态时广播
///
/// - `app`: 应用句柄，广播用它
/// - `dto`: 广播出去的总览快照
#[gui_macros::emit]
pub fn emit_add_modpack_status(app: &AppHandle, dto: ModPackStatusDto) {
    let _ = app.emit(listens::ADD_MODPACK_STATUS, dto);
}

/// 本地压缩包导入用的占位项目 ID
///
/// 任务表以 pid + fid 为键，而本地包**没有**项目 ID，也不该假装有。用这个固定值占位、
/// 把包路径当文件 ID，同一个包就不会重复安装。它只活在任务表里：`running_pids` /
/// `installed_modpack_pids` 那边只拿它给**在线列表**打角标，不会与真实项目 ID 撞车。
pub(crate) const LOCAL_PACK_PID: &str = "local";

/// 一次登记成功的整合包安装任务：任务句柄与它的安装进度回调
///
/// 两者都是调用方在安装期间必须拿住的东西：任务句柄用于收尾记终态，回调交给内核安装流程。
pub(crate) struct RegisteredTask {
    /// 任务句柄，已登记进任务表，同一 pid + fid 不会再登记第二个
    pub(crate) task: Arc<ModPackTask>,
    /// 交给内核安装流程的进度回调，进度写回 `task` 并广播总览事件
    pub(crate) gui: AddModPackGui,
}

/// 登记一个整合包安装任务，同 pid+fid 不重复安装；返回任务句柄与安装进度回调
///
/// 在线安装 [`add_modpack_install`] 与本地压缩包导入 [`super::add::add_import_archive`]
/// 共用它：两条路径产生的任务、进度事件、取消与终态处理完全一致，
/// 标题栏指示器、进度弹窗、主窗口列表刷新都复用那一套。
///
/// `cancel` 由调用方给：本地导入复用**添加实例窗口**那份令牌，
/// 这样窗口上的取消按钮与任务上的取消停掉的是同一次安装。
///
/// - `app`: 应用句柄，事件与回调桥接都用它
/// - `key`: 任务键，即下载源加项目与文件 ID；同键不重复安装
/// - `source`: 整合包来源平台 ID
/// - `name`: 显示名，取自整合包列表缓存
/// - `kind`: 这次任务在干什么，取值 `install` / `upgrade`
/// - `cancel`: 取消令牌，调用方持有同一份
///
/// # 返回值
///
/// 见 [`RegisteredTask`]；同键已有任务在跑时返回错误文案
pub(crate) fn register_task(
    app: &AppHandle,
    key: SourceInfo,
    source: &str,
    name: String,
    kind: &str,
    cancel: CancellationToken,
) -> Result<RegisteredTask, String> {
    let task = Arc::new(ModPackTask {
        uuid: Uuid::new_v4(),
        source: source.to_string(),
        pid: key.pid.clone(),
        fid: key.fid.clone(),
        name,
        kind: kind.to_string(),
        cancel: cancel.clone(),
        progress: Mutex::new(ModPackProgress {
            state: pack_state_id(AddModPackState::DownloadPack).into(),
            now: 0,
            total: 0,
            sub_text: None,
            sub_now: 0,
            sub_total: 0,
        }),
        done: AtomicBool::new(false),
        failed: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        error: Mutex::new(None),
        instance_uuid: Mutex::new(None),
    });

    {
        let mut map = DOWNLOAD_NOW.write().unwrap();
        if map.contains_key(&key) {
            return Err(String::from(err_keys::ALREADY_DOWNLOADING));
        }
        map.insert(key, task.clone());
    }
    emit_add_modpack_status(app, build_status(app));

    let pack_gui: AddModPackGui = Some(Arc::new(TaskPackGui {
        task: task.clone(),
        app: app.clone(),
    }));
    Ok(RegisteredTask {
        task,
        gui: pack_gui,
    })
}

/// 记录安装终态并广播
///
/// - `app`: 应用句柄，广播总览与实例变更都用它
/// - `task`: 目标任务
/// - `cancel`: 该任务的取消令牌，用来区分失败与被取消
/// - `res`: 安装结果
///
/// `res` 为 `Ok` 时回填新实例 uuid 并广播 instance-change，主窗口据此刷新并选中；
/// 为 `Err` 时**令牌已取消就记为"已取消"**，取消可能来自别的入口，例如添加实例窗口自己的
/// 取消按钮，那条路径不会去置 cancelled 标记；否则记失败原因
pub(crate) fn finish_task(
    app: &AppHandle,
    task: &ModPackTask,
    cancel: &CancellationToken,
    res: Result<Uuid, String>,
) {
    match res {
        Ok(uuid) => {
            *task.instance_uuid.lock().unwrap() = Some(uuid.to_string());
            // 装完了就是装完了：取消标记要清掉。用户恰在成功前点了取消的话，
            // 两个标记会同时为真，DTO 注释里三者互斥，前端会同时显示"已完成"和"已取消"
            task.cancelled.store(false, Ordering::Release);
            task.done.store(true, Ordering::Release);
            crate::windows::main::emit_instance_change(app, "add");
        }
        Err(err) => {
            if cancel.is_cancelled() {
                task.cancelled.store(true, Ordering::Release);
            } else {
                *task.error.lock().unwrap() = Some(err);
                task.failed.store(true, Ordering::Release);
            }
        }
    }
    emit_add_modpack_status(app, build_status(app));
}

/// 安装阶段 ID，与前端 i18n 键对应
///
/// - `state`: 内核安装状态
///
/// # 返回值
///
/// `downloadPack` / `readInfo` / `getInfo` / `downloadFile` / `extract` / `done` 之一
pub(super) fn pack_state_id(state: AddModPackState) -> &'static str {
    match state {
        AddModPackState::DownloadPack => "downloadPack",
        AddModPackState::ReadInfo => "readInfo",
        AddModPackState::GetInfo => "getInfo",
        AddModPackState::DownloadFile => "downloadFile",
        AddModPackState::Extract => "extract",
        AddModPackState::Done => "done",
    }
}

/// 整合包安装进度回调：把安装状态 / 进度写进任务并广播总览事件
pub(super) struct TaskPackGui {
    /// 回调对应的任务，进度写进它
    task: Arc<ModPackTask>,
    /// 应用句柄，广播总览事件用
    app: AppHandle,
}

impl TaskPackGui {
    /// 更新任务进度并广播总览事件
    ///
    /// - `f`: 就地改写进度快照的闭包
    fn update(&self, f: impl FnOnce(&mut ModPackProgress)) {
        {
            let mut progress = self.task.progress.lock().unwrap();
            f(&mut progress);
        }
        emit_add_modpack_status(&self.app, build_status(&self.app));
    }
}

impl IAddModPackGui for TaskPackGui {
    /// 设置安装阶段
    ///
    /// - `state`: 内核安装状态
    fn set_state(&self, state: AddModPackState) {
        self.update(|progress| progress.state = pack_state_id(state).into());
    }

    /// 设置主进度
    ///
    /// - `value`: 已完成数
    /// - `all`: 总数，`None` 时按 0 处理
    fn set_now(&self, value: usize, all: Option<usize>) {
        self.update(|progress| {
            progress.now = value as u32;
            progress.total = all.unwrap_or(0) as u32;
        });
    }

    /// 设置子任务说明文字
    ///
    /// - `text`: 说明文字，`None` 表示当前没有子任务
    fn set_sub_text(&self, text: Option<String>) {
        self.update(|progress| progress.sub_text = text);
    }

    /// 设置子任务进度
    ///
    /// - `value`: 子任务已完成数
    /// - `all`: 子任务总数，`None` 时按 0 处理
    fn set_sub_now(&self, value: usize, all: Option<usize>) {
        self.update(|progress| {
            progress.sub_now = value as u32;
            progress.sub_total = all.unwrap_or(0) as u32;
        });
    }
}
