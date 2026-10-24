//! 编辑器状态：项目、构建任务、预览进程与配置
//!
//! 前端只发动作，不持有跨界面数据；任务的取消、预览子进程和最近项目都在这里。

use crate::{
    config::{self, Config, Theme},
    files::{self, SaveInput, SourceFile},
    messages::{Locale, MessageKey, StudioError, render},
    preview::{self, PreviewState, WindowMinSize, WorkerEvent},
    process::{self, Environment, LogFn, RunOptions, environment, no_log, run},
    project::{self, CreateProjectInput, Project, Template},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::process::Stdio;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tokio::{
    process::Command,
    sync::{Mutex, mpsc, watch},
};
use tokio_util::sync::CancellationToken;

/// 同时只允许一个长任务
struct Task {
    /// 取消信号
    cancel: CancellationToken,
    /// 任务结束通知
    done: watch::Sender<bool>,
}

/// 前端启动 / 重载时需要的状态
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioState {
    /// 当前项目，没有则为空
    pub project: Option<Project>,
    /// 界面语言
    pub locale: Locale,
    /// 界面配色
    pub theme: Theme,
    /// 最近项目
    pub recent: Vec<String>,
    /// 外部工具环境
    pub environment: EnvironmentView,
    /// 各预览窗口的最小客户区尺寸，按窗口 kind 索引
    pub window_min_sizes: BTreeMap<String, WindowMinSize>,
}

/// 外部工具环境，探测失败时只带错误文案
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentView {
    /// 满足要求时是 node 版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_version: Option<String>,
    /// git --version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    /// 探测失败的原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 编辑器共享状态
pub struct Studio {
    app: AppHandle,
    config_path: PathBuf,
    config: Mutex<Config>,
    project: Mutex<Option<Project>>,
    task: Mutex<Option<Task>>,
    preview: Mutex<PreviewState>,
    cache: PathBuf,
    environment_cache: Mutex<Option<Environment>>,
    dirty: AtomicBool,
    busy: AtomicBool,
    allow_close: AtomicBool,
    asking_close: AtomicBool,
}

impl Studio {
    /// 建立共享状态
    ///
    /// - `app`: 应用句柄
    /// - `config_path`: studio.json 路径
    /// - `config`: 启动时读到的配置
    /// - `cache`: 下载与预览临时目录
    pub fn new(app: AppHandle, config_path: PathBuf, config: Config, cache: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            app,
            config_path,
            config: Mutex::new(config),
            project: Mutex::new(None),
            task: Mutex::new(None),
            preview: Mutex::new(PreviewState::default()),
            cache,
            environment_cache: Mutex::new(None),
            dirty: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            allow_close: AtomicBool::new(false),
            asking_close: AtomicBool::new(false),
        })
    }

    /// 是否有未保存修改
    pub fn dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// 是否有长任务在跑
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// 是否允许关闭窗口
    pub fn allow_close(&self) -> bool {
        self.allow_close.load(Ordering::SeqCst)
    }

    /// 是否已经在弹关闭确认
    pub fn asking_close(&self) -> bool {
        self.asking_close.load(Ordering::SeqCst)
    }

    /// 设置关闭确认状态
    pub fn set_asking_close(&self, value: bool) {
        self.asking_close.store(value, Ordering::SeqCst);
    }

    /// 放行关闭
    pub fn set_allow_close(&self, value: bool) {
        self.allow_close.store(value, Ordering::SeqCst);
    }

    /// 标记未保存修改
    pub fn set_dirty(&self, value: bool) {
        self.dirty.store(value, Ordering::SeqCst);
    }

    /// 当前语言
    pub async fn locale(&self) -> Locale {
        self.config.lock().await.locale
    }

    /// 写一行原始输出
    pub fn log_raw(&self, text: &str) {
        let _ = self.app.emit("studio-log", text);
    }

    /// 广播忙状态
    fn emit_busy(&self, value: bool) {
        let _ = self.app.emit("studio-busy", value);
    }

    /// 广播预览地址，空串表示已停止
    fn emit_preview(&self, url: &str) {
        let _ = self.app.emit("studio-preview", url);
    }

    /// 写一条 i18n 日志
    async fn message(&self, key: MessageKey, values: &[(&'static str, String)]) {
        let locale = self.locale().await;
        self.log_raw(&format!("{}\n", render(locale, key, values)));
    }

    /// 日志回调：给 project / process 用
    fn log_sink(&self) -> LogFn {
        let app = self.app.clone();
        Arc::new(move |text: &str| {
            let _ = app.emit("studio-log", text);
        })
    }

    /// 启动前的忙碌检查
    ///
    /// # 返回值
    ///
    /// 本次任务的取消令牌
    pub async fn begin_task(&self) -> Result<CancellationToken, StudioError> {
        let mut task = self.task.lock().await;
        if task.is_some() {
            return Err(StudioError::new(MessageKey::Busy));
        }
        let cancel = CancellationToken::new();
        let (done, _) = watch::channel(false);
        *task = Some(Task {
            cancel: cancel.clone(),
            done,
        });
        drop(task);
        self.busy.store(true, Ordering::SeqCst);
        self.emit_busy(true);
        Ok(cancel)
    }

    /// 任务结束，释放忙状态
    pub async fn end_task(&self) {
        let mut task = self.task.lock().await;
        if let Some(task) = task.take() {
            let _ = task.done.send(true);
        }
        self.busy.store(false, Ordering::SeqCst);
        self.emit_busy(false);
    }

    /// 取消当前任务并等它收尾
    pub async fn cancel(&self) {
        let (cancel, mut done) = {
            let task = self.task.lock().await;
            match task.as_ref() {
                Some(task) => (task.cancel.clone(), task.done.subscribe()),
                None => return,
            }
        };
        cancel.cancel();
        let _ = tokio::time::timeout(Duration::from_secs(30), done.wait_for(|value| *value)).await;
    }

    /// 当前项目
    pub async fn project(&self) -> Option<Project> {
        self.project.lock().await.clone()
    }

    /// 取当前项目，没有则报错
    async fn require_project(&self) -> Result<Project, StudioError> {
        self.project()
            .await
            .ok_or_else(|| StudioError::new(MessageKey::InvalidProject))
    }

    /// 切换项目并记入最近列表
    pub async fn select_project(&self, project: Project) -> Result<(), StudioError> {
        self.stop_preview().await;
        *self.project.lock().await = Some(project.clone());
        {
            let mut config = self.config.lock().await;
            config.recent.retain(|path| path != &project.root);
            config.recent.insert(0, project.root.clone());
            config.recent.truncate(8);
        }
        self.save_config().await
    }

    /// 落盘配置
    async fn save_config(&self) -> Result<(), StudioError> {
        let config = self.config.lock().await.clone();
        config::save(&self.config_path, &config).await
    }

    /// 启动状态
    pub async fn state(&self) -> StudioState {
        let config = self.config.lock().await.clone();
        let project = self.project().await;
        let environment = match self.environment().await {
            Ok(env) => EnvironmentView {
                node_version: Some(env.node_version),
                git: Some(env.git),
                error: None,
            },
            Err(error) => EnvironmentView {
                node_version: None,
                git: None,
                error: Some(error.text(config.locale)),
            },
        };
        StudioState {
            project,
            locale: config.locale,
            theme: config.theme,
            recent: config.recent,
            environment,
            window_min_sizes: preview::min_sizes(),
        }
    }

    /// 外部工具环境，探测一次后缓存
    async fn environment(&self) -> Result<Environment, StudioError> {
        if let Some(environment) = self.environment_cache.lock().await.clone() {
            return Ok(environment);
        }
        let environment = environment().await?;
        *self.environment_cache.lock().await = Some(environment.clone());
        Ok(environment)
    }

    /// 选择目录
    ///
    /// - `kind`: source / parent / project
    pub async fn pick(&self, kind: String) -> Result<Option<String>, StudioError> {
        let locale = self.locale().await;
        let key = match kind.as_str() {
            "source" => MessageKey::ChooseRepo,
            "parent" => MessageKey::ChooseParent,
            _ => MessageKey::ChooseProject,
        };
        let app = self.app.clone();
        let title = render(locale, key, &[]);
        let folder = tokio::task::spawn_blocking(move || {
            let mut dialog = app.dialog().file().set_title(title);
            if let Some(window) = app.get_webview_window("main") {
                dialog = dialog.set_parent(&window);
            }
            dialog.blocking_pick_folder()
        })
        .await
        .map_err(|error| StudioError::raw(error.to_string()))?;
        Ok(folder
            .and_then(|folder| folder.into_path().ok())
            .map(|path| files::clean_path(path).to_string_lossy().into_owned()))
    }

    /// 创建项目
    pub async fn create(
        &self,
        input: CreateProjectInput,
        cancel: &CancellationToken,
    ) -> Result<Project, StudioError> {
        self.message(MessageKey::Downloading, &[]).await;
        let project = project::create_project(input, &self.cache, cancel, &self.log_sink()).await?;
        self.select_project(project.clone()).await?;
        self.message(MessageKey::Created, &[("path", project.root.clone())])
            .await;
        Ok(project)
    }

    /// 打开项目
    pub async fn open(&self, root: &str) -> Result<Project, StudioError> {
        if self.busy() {
            return Err(StudioError::new(MessageKey::Busy));
        }
        let project = project::open_project(Path::new(root)).await?;
        self.select_project(project.clone()).await?;
        Ok(project)
    }

    /// 文件树
    pub async fn files(&self) -> Result<Vec<String>, StudioError> {
        let project = self.require_project().await?;
        files::list_files(Path::new(&project.root)).await
    }

    /// 读文件
    pub async fn read(&self, path: &str) -> Result<SourceFile, StudioError> {
        let project = self.require_project().await?;
        files::read_text(Path::new(&project.root), path).await
    }

    /// 保存文件
    pub async fn save(&self, input: SaveInput) -> Result<SourceFile, StudioError> {
        let project = self.require_project().await?;
        let path = input.path.clone();
        let file = files::save_text(Path::new(&project.root), input).await?;
        self.message(MessageKey::FileSaved, &[("path", path)]).await;
        Ok(file)
    }

    /// 安装依赖
    pub async fn install(&self, cancel: &CancellationToken) -> Result<(), StudioError> {
        self.stop_preview().await;
        let project = self.require_project().await?;
        project::install_project(&project, cancel, &self.log_sink()).await?;
        self.message(MessageKey::Installed, &[]).await;
        Ok(())
    }

    /// 构建 ZIP
    pub async fn build(&self, cancel: &CancellationToken) -> Result<Vec<String>, StudioError> {
        let project = self.require_project().await?;
        let artifacts = project::build_project(&project, cancel, &self.log_sink()).await?;
        for path in &artifacts {
            self.message(MessageKey::Built, &[("path", path.clone())])
                .await;
        }
        Ok(artifacts)
    }

    /// 读取项目 SDK 里的控件与变量清单
    pub async fn catalog(&self) -> Result<Value, StudioError> {
        let project = self.require_project().await?;
        if project.template == Template::Html {
            return Ok(json!({ "controls": [], "variables": [] }));
        }
        let environment = self.environment().await?;
        let worker = runtime_dir(&self.app).join("preview/catalog.mjs");
        let node = environment.node.to_string_lossy().into_owned();
        let args = vec![worker.to_string_lossy().into_owned(), project.root.clone()];
        let output = run(
            &node,
            &args,
            RunOptions {
                cwd: Some(Path::new(&project.root)),
                log: no_log(),
                cancel: None,
            },
        )
        .await
        .map_err(|_| StudioError::new(MessageKey::MissingSdk))?;
        serde_json::from_str(&output).map_err(|_| StudioError::new(MessageKey::MissingSdk))
    }

    /// 在文件管理器里打开项目目录
    pub async fn reveal(&self) -> Result<String, StudioError> {
        let project = self.require_project().await?;
        let path = Path::new(&project.root);
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("explorer").arg(path).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(path).spawn();
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        }
        Ok(String::new())
    }

    /// 切换界面语言
    pub async fn set_locale(&self, value: &str) -> Result<Locale, StudioError> {
        let locale =
            Locale::parse(value).ok_or_else(|| StudioError::new(MessageKey::InvalidProject))?;
        self.config.lock().await.locale = locale;
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.set_title(&render(locale, MessageKey::Title, &[]));
        }
        self.save_config().await?;
        Ok(locale)
    }

    /// 切换界面配色，同时改原生标题栏
    pub async fn set_theme(&self, value: &str) -> Result<Theme, StudioError> {
        let theme =
            Theme::parse(value).ok_or_else(|| StudioError::new(MessageKey::InvalidTheme))?;
        self.config.lock().await.theme = theme;
        if let Some(window) = self.app.get_webview_window("main") {
            // 个别平台不支持动态改原生标题栏主题，失败也不该拖垮配色切换
            let _ = window.set_theme(Some(theme.native()));
            let _ = window.set_background_color(Some(theme.color()));
        }
        self.save_config().await?;
        Ok(theme)
    }

    /// 启动预览
    pub async fn start_preview(
        self: &Arc<Self>,
        cancel: &CancellationToken,
    ) -> Result<String, StudioError> {
        self.stop_preview().await;
        self.message(MessageKey::Preparing, &[]).await;
        let environment = self.environment().await?;
        let project = self.require_project().await?;
        tokio::fs::create_dir_all(&self.cache).await?;
        let cache = self.cache.join(format!("preview-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&cache).await?;
        let worker = runtime_dir(&self.app).join("preview/worker.mjs");
        let locale = self.locale().await;

        let mut command = Command::new(&environment.node);
        command
            .arg(&worker)
            .arg(&project.root)
            .arg(project.template.as_str())
            .arg(&cache)
            .arg(locale.as_str())
            .current_dir(&project.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        process::configure(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| StudioError::raw(format!("node: {error}")))?;
        let pid = child
            .id()
            .ok_or_else(|| StudioError::raw("node: missing pid"))?;
        process::registry().add(pid);
        // 先登记 pid：worker 在报告就绪前后退出时，退出回调能认出这是当前这次预览
        self.preview.lock().await.pid = Some(pid);
        // tokio 的 wait 会顺手丢掉 stdin；这里自己拿着，父进程活着时管道就不算断，
        // worker 也不会把"启动成功"误判成"父进程没了"。
        let stdin = child.stdin.take();

        let (tx, mut rx) = mpsc::channel::<WorkerEvent>(16);
        let log = self.log_sink();
        if let Some(stdout) = child.stdout.take() {
            process::spawn_line_reader(stdout, log.clone(), tx.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            process::spawn_line_reader(stderr, log, tx.clone());
        }
        drop(tx);

        let monitor = Arc::clone(self);
        tokio::spawn(async move {
            let status = child.wait().await;
            drop(stdin);
            process::registry().remove(pid);
            monitor.on_preview_exit(pid, status).await;
        });

        let ready = async {
            tokio::select! {
                _ = cancel.cancelled() => Err(StudioError::new(MessageKey::Cancelled)),
                message = rx.recv() => match message {
                    Some(WorkerEvent::Ready(url)) => Ok(url),
                    Some(WorkerEvent::Failed(error)) => {
                        if !error.is_empty() {
                            self.log_raw(&format!("{error}\n"));
                        }
                        Err(StudioError::new(MessageKey::PreviewFailed))
                    }
                    None => Err(StudioError::new(MessageKey::PreviewFailed)),
                },
            }
        };
        let url = match tokio::time::timeout(Duration::from_secs(120), ready).await {
            Ok(Ok(url)) if !cancel.is_cancelled() => url,
            Ok(Ok(_)) => {
                process::kill_tree(pid).await;
                return Err(StudioError::new(MessageKey::Cancelled));
            }
            Ok(Err(error)) => {
                process::kill_tree(pid).await;
                return Err(error);
            }
            Err(_) => {
                process::kill_tree(pid).await;
                return Err(StudioError::new(MessageKey::PreviewFailed));
            }
        };

        {
            let mut preview = self.preview.lock().await;
            // 报告就绪与进程退出可能只差几毫秒，退出回调已经把 pid 清掉时不再对外报地址
            if preview.pid != Some(pid) {
                drop(preview);
                process::kill_tree(pid).await;
                return Err(StudioError::new(MessageKey::PreviewFailed));
            }
            preview.url = url.clone();
            // 持锁广播，避免退出回调的"空地址"被这次广播盖回去
            self.emit_preview(&url);
        }
        self.message(MessageKey::PreviewReady, &[("url", url.clone())])
            .await;
        Ok(url)
    }

    /// 停止预览
    pub async fn stop_preview(&self) {
        let pid = {
            let mut preview = self.preview.lock().await;
            preview.url.clear();
            preview.pid.take()
        };
        if let Some(pid) = pid {
            process::kill_tree(pid).await;
        }
        self.emit_preview("");
    }

    /// 预览 worker 退出后清掉状态
    async fn on_preview_exit(&self, pid: u32, _status: std::io::Result<std::process::ExitStatus>) {
        let mut preview = self.preview.lock().await;
        if preview.pid != Some(pid) {
            return;
        }
        preview.pid = None;
        preview.url.clear();
        drop(preview);
        self.emit_preview("");
    }

    /// 打开编辑器窗口的开发者工具；预览 iframe 也在同一套 DevTools 里
    pub fn open_devtools(&self) {
        if self.preview_url_blocking().is_empty() {
            return;
        }
        if let Some(window) = self.app.get_webview_window("main") {
            window.open_devtools();
        }
    }

    fn preview_url_blocking(&self) -> String {
        // 这里只用来判断有没有预览，拿不到锁时按"没有"处理
        self.preview
            .try_lock()
            .map(|preview| preview.url.clone())
            .unwrap_or_default()
    }

    /// 应用退出前同步回收预览与子进程
    pub fn shutdown_blocking(&self) {
        process::registry().kill_all();
    }
}

/// 预览脚本所在目录：开发时用仓库，分发时用资源目录
fn runtime_dir(app: &AppHandle) -> PathBuf {
    if let Ok(directory) = app.path().resource_dir()
        && directory.join("preview/worker.mjs").is_file()
    {
        return directory;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
