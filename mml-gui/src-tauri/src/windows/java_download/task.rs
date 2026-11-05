//! Java 安装任务表
//!
//! 一次安装分三段：下载 zip（交给下载器）→ 解压 → 识别注册。
//! 每段的进度与结果都记在这里，并经 `java-install-status` 事件广播给界面；
//! 取消令牌也挂在任务上，界面点取消后由安装流程自己收尾。

use std::{
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::dtos::{JavaInstallStageDto, JavaInstallTaskDto};
use crate::listens;

use super::JavaInstallResult;

/// 进度事件的节流间隔
const EMIT_GAP: Duration = Duration::from_millis(120);

/// 一条安装任务
struct Task {
    /// 稳定包 id，由下载地址派生
    id: Uuid,
    /// 显示名
    name: String,
    /// 当前阶段
    stage: JavaInstallStageDto,
    /// 当前阶段进度，0.0 到 1.0
    progress: f32,
    /// 失败原因
    error: Option<String>,
    /// 取消令牌，点"取消"后置位
    cancel: CancellationToken,
}

/// 全部安装任务，按开始顺序
static TASKS: LazyLock<Mutex<Vec<Task>>> = LazyLock::new(|| Mutex::new(Vec::new()));
/// 上次广播时间，用来给高频进度节流
static LAST_EMIT: LazyLock<Mutex<Instant>> = LazyLock::new(|| Mutex::new(Instant::now()));

/// 当前任务快照
///
/// # 返回值
///
/// 按开始顺序排列的任务列表
pub fn list() -> Vec<JavaInstallTaskDto> {
    TASKS
        .lock()
        .unwrap()
        .iter()
        .map(|task| JavaInstallTaskDto {
            id: task.id.to_string(),
            name: task.name.clone(),
            stage: task.stage,
            progress: task.progress,
            error: task.error.clone(),
        })
        .collect()
}

/// 安装任务总览事件，增删 / 阶段变化 / 进度变化时广播
///
/// - `app`: 应用句柄，广播用它
/// - `tasks`: 广播出去的任务列表
#[gui_macros::emit]
pub fn emit_java_install_status(app: &AppHandle, tasks: Vec<JavaInstallTaskDto>) {
    let _ = app.emit(listens::JAVA_INSTALL_STATUS, tasks);
}

/// 广播当前任务列表
fn emit(app: &AppHandle) {
    emit_java_install_status(app, list());
}

/// 登记一条新任务并广播
///
/// - `app`: 应用句柄
/// - `id`: 稳定包 id
/// - `name`: 显示名
///
/// # 返回值
///
/// 这条任务的取消令牌，交给安装流程
pub fn begin(app: &AppHandle, id: Uuid, name: &str) -> CancellationToken {
    let cancel = CancellationToken::new();
    let mut tasks = TASKS.lock().unwrap();
    tasks.retain(|task| task.id != id);
    tasks.push(Task {
        id,
        name: name.to_string(),
        stage: JavaInstallStageDto::Download,
        progress: 0.0,
        error: None,
        cancel: cancel.clone(),
    });
    drop(tasks);
    emit(app);
    cancel
}

/// 取消一条正在跑的任务
///
/// 取消是协作式的：下载会立刻收手，解压按条目检查，识别阶段无法中断。
///
/// - `id`: 稳定包 id
///
/// # 返回值
///
/// 任务存在且仍在进行时返回 `true`
pub fn cancel(id: Uuid) -> bool {
    let tasks = TASKS.lock().unwrap();
    let Some(task) = tasks.iter().find(|task| task.id == id) else {
        return false;
    };
    if !matches!(
        task.stage,
        JavaInstallStageDto::Download
            | JavaInstallStageDto::Extract
            | JavaInstallStageDto::Probe
    ) {
        return false;
    }
    task.cancel.cancel();
    true
}

/// 换阶段并把进度清零
///
/// - `app`: 应用句柄
/// - `id`: 稳定包 id
/// - `stage`: 新阶段
pub fn set_stage(app: &AppHandle, id: Uuid, stage: JavaInstallStageDto) {
    if let Some(task) = TASKS.lock().unwrap().iter_mut().find(|task| task.id == id) {
        task.stage = stage;
        task.progress = 0.0;
    }
    emit(app);
}

/// 更新当前阶段进度，带节流
///
/// - `app`: 应用句柄
/// - `id`: 稳定包 id
/// - `progress`: 0.0 到 1.0
pub fn set_progress(app: &AppHandle, id: Uuid, progress: f32) {
    let value = progress.clamp(0.0, 1.0);
    if let Some(task) = TASKS.lock().unwrap().iter_mut().find(|task| task.id == id) {
        task.progress = value;
    }
    let mut last = LAST_EMIT.lock().unwrap();
    if last.elapsed() < EMIT_GAP {
        return;
    }
    *last = Instant::now();
    drop(last);
    emit(app);
}

/// 标记结束
///
/// - `app`: 应用句柄
/// - `id`: 稳定包 id
/// - `result`: 安装结果，决定 `done` / `cancelled` / `failed`
pub fn finish(app: &AppHandle, id: Uuid, result: JavaInstallResult) {
    if let Some(task) = TASKS.lock().unwrap().iter_mut().find(|task| task.id == id) {
        match result {
            JavaInstallResult::Done => {
                task.stage = JavaInstallStageDto::Done;
                task.progress = 1.0;
                task.error = None;
            }
            JavaInstallResult::Cancelled => {
                task.stage = JavaInstallStageDto::Cancelled;
                task.error = None;
            }
            JavaInstallResult::Failed(error) => {
                task.stage = JavaInstallStageDto::Failed;
                task.error = Some(error);
            }
        }
    }
    emit(app);
}

/// 这个包是不是已经在装了
///
/// id 由下载地址派生，换列表、换筛选都不变，所以按 id 判断即可。
///
/// - `id`: 稳定包 id
///
/// # 返回值
///
/// 已有同一条任务在跑时返回 `true`
pub fn is_running(id: Uuid) -> bool {
    TASKS.lock().unwrap().iter().any(|task| {
        task.id == id
            && matches!(
                task.stage,
                JavaInstallStageDto::Download
                    | JavaInstallStageDto::Extract
                    | JavaInstallStageDto::Probe
            )
    })
}

/// 清掉已经结束的任务
///
/// - `app`: 应用句柄
pub fn clear_finished(app: &AppHandle) {
    TASKS.lock().unwrap().retain(|task| {
        !matches!(
            task.stage,
            JavaInstallStageDto::Done
                | JavaInstallStageDto::Failed
                | JavaInstallStageDto::Cancelled
        )
    });
    emit(app);
}
