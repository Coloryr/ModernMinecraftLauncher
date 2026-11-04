//! Java 安装任务表
//!
//! 一次安装分三段：下载 zip（交给下载器）→ 解压 → 识别注册。
//! 每段的进度与结果都记在这里，并经 `java-install-status` 事件广播给界面。

use std::{
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use mml_base::archives::IBaseArchiveGui;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dtos::{JavaInstallStageDto, JavaInstallTaskDto};
use crate::listens;

/// 进度事件的节流间隔
const EMIT_GAP: Duration = Duration::from_millis(120);

/// 一条安装任务
struct Task {
    /// 列表项 uuid
    id: Uuid,
    /// 显示名
    name: String,
    /// 当前阶段
    stage: JavaInstallStageDto,
    /// 当前阶段进度，0.0 到 1.0
    progress: f32,
    /// 失败原因
    error: Option<String>,
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
/// - `id`: 列表项 uuid
/// - `name`: 显示名
pub fn begin(app: &AppHandle, id: Uuid, name: &str) {
    let mut tasks = TASKS.lock().unwrap();
    tasks.retain(|task| task.id != id);
    tasks.push(Task {
        id,
        name: name.to_string(),
        stage: JavaInstallStageDto::Download,
        progress: 0.0,
        error: None,
    });
    drop(tasks);
    emit(app);
}

/// 换阶段并把进度清零
///
/// - `app`: 应用句柄
/// - `id`: 列表项 uuid
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
/// - `id`: 列表项 uuid
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

/// 标记结束：`error` 为 `None` 表示成功
///
/// - `app`: 应用句柄
/// - `id`: 列表项 uuid
/// - `error`: 失败原因
pub fn finish(app: &AppHandle, id: Uuid, error: Option<String>) {
    if let Some(task) = TASKS.lock().unwrap().iter_mut().find(|task| task.id == id) {
        match &error {
            Some(_) => task.stage = JavaInstallStageDto::Failed,
            None => {
                task.stage = JavaInstallStageDto::Done;
                task.progress = 1.0;
            }
        }
        task.error = error;
    }
    emit(app);
}

/// 这个名字是不是已经在装了
///
/// 列表每次拉取都会换一批 uuid，同名的包重新拉回来对不上 uuid，
/// 所以重装判断按名字来：同一个包在装时再点一次不会重复登记。
///
/// - `name`: 包的显示名
///
/// # 返回值
///
/// 已有同名任务在跑时返回 `true`
pub fn is_running_name(name: &str) -> bool {
    TASKS.lock().unwrap().iter().any(|task| {
        task.name == name
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
            JavaInstallStageDto::Done | JavaInstallStageDto::Failed
        )
    });
    emit(app);
}

/// 解压进度回调，把条目推进折算成任务进度
pub struct ExtractGui {
    /// 应用句柄
    app: AppHandle,
    /// 列表项 uuid
    id: Uuid,
    /// 解压总条目数，`start` 时下发
    total: AtomicUsize,
}

impl ExtractGui {
    /// 建一个解压进度回调
    ///
    /// - `app`: 应用句柄
    /// - `id`: 列表项 uuid
    ///
    /// # 返回值
    ///
    /// 给 `mml_jvms::extract_java` 用的回调
    pub fn new(app: &AppHandle, id: Uuid) -> Self {
        Self {
            app: app.clone(),
            id,
            total: AtomicUsize::new(0),
        }
    }
}

impl IBaseArchiveGui for ExtractGui {
    /// 记下总条目数
    fn start(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        set_progress(&self.app, self.id, 0.0);
    }

    /// 按已完成条目推进进度
    fn update(&self, _filename: Option<String>, current: usize) {
        let total = self.total.load(Ordering::Relaxed);
        if total > 0 {
            set_progress(&self.app, self.id, current as f32 / total as f32);
        }
    }

    /// 解压完成：进度拉满
    fn done(&self) {
        set_progress(&self.app, self.id, 1.0);
    }

    /// 条目名含非法字符时一律替换，安装流程不弹窗
    fn file_rename(&self, _name: &str) -> bool {
        true
    }
}
