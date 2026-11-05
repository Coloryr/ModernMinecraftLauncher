//! Java 下载窗口：Java 运行时下载的规格 + IPC 命令
//!
//! 搜索源 Adoptium / Zulu / OpenJ9 / Foojay 选定后，经 java_download_get_options
//! 取该源的四个下拉框候选，即发行类型 / 主版本 / 系统 / 架构，筛选拟下载的 Java，
//! 选好后经 java_download_start 触发下载，完成后经 java-change 事件
//! 通知各窗口刷新 Java 列表，与手动添加是同一条刷新链路。
//!
//! 列表项的 id 由下载地址派生（见 list::uuid_of），换列表也不变：
//! 正在安装的判断、重复点击的拦截与解压目录都用它，不再依赖显示名。
//!
//! 安装任务支持取消，结果分完成 / 取消 / 失败三种；安装过程不挂在窗口上，
//! 窗口关掉也照跑，结束时会按需发系统通知。

use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use mml_base::{
    archives::{BaseArchiveGui, IBaseArchiveGui},
    file_item::FileItemObj,
};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::dtos::{JavaDownloadOptionsDto, JavaInstallStageDto, JavaInstallTaskDto, JavaTypes};
use crate::err_keys;
use crate::log_text::NotifyText;
use crate::notify;

// 带命令的子模块必须 pub(crate)：生成的 tauri_commands! 从 crate 根引用它
pub(crate) mod list;
pub(crate) mod task;

// 集成测试 tests/java_download_test.rs 按原路径引用它，故再导出
pub use self::list::java_download_get_list;

/// Java 包列表缓存，键是列表项 id、值是待下载项目；列表查询成功后整体替换，开始下载时取出
pub(super) static JAVA_ITEMS: LazyLock<RwLock<HashMap<Uuid, FileItemObj>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 一次安装的结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaInstallResult {
    /// 下载、解压、识别都成功
    Done,
    /// 用户取消
    Cancelled,
    /// 失败，内容是 i18n 键或错误原文
    Failed(String),
}

/// 安装进度回调事件
pub enum JavaInstallEvent {
    /// 换阶段
    Stage(JavaInstallStageDto),
    /// 当前阶段进度，0.0 到 1.0
    Progress(f32),
}

/// 获取全部搜索源：Adoptium / Zulu / OpenJ9 / Foojay
///
/// # 返回值
///
/// 全部搜索源的枚举值，供前端下拉框使用；不会失败
#[gui_macros::command]
pub fn java_download_get_types() -> Vec<JavaTypes> {
    JavaTypes::all()
}

/// 获取指定搜索源的 Java 下载可选项，即四个下拉框的候选列表
///
/// 各源候选来源不同：
/// - Foojay：/parameters 接口，四组候选齐全
/// - Adoptium：主版本动态拉取，类型 / 系统 / 架构为其支持的固定集合
/// - Zulu：从包列表提取，只取 GA 版本，bundle_type 区分 jdk / jre，主版本取 java_version 首段
/// - OpenJ9：页面抓取结果自带 arch / os / 主版本候选
///
/// - source: 搜索源
///
/// # 返回值
///
/// 该源的四个下拉框候选，即发行类型 / 主版本 / 系统 / 架构；网络失败时返回 Err
#[gui_macros::command]
pub async fn java_download_get_options(
    source: JavaTypes,
) -> Result<JavaDownloadOptionsDto, String> {
    match source {
        JavaTypes::Foojay => {
            let obj = mml_net::foojay_api::get_options()
                .await
                .map_err(|e| e.to_string())?;
            Ok(JavaDownloadOptionsDto {
                types: obj.types,
                majors: obj
                    .majors
                    .iter()
                    .filter_map(|item| item.parse().ok())
                    .collect(),
                systems: obj.systems,
                archs: obj.archs,
            })
        }
        JavaTypes::Adoptium => {
            let versions = mml_net::adoptium_api::get_java_version()
                .await
                .map_err(|e| e.to_string())?;
            let mut majors: Vec<u32> = versions
                .iter()
                .filter_map(|item| item.parse().ok())
                .collect();
            majors.sort_unstable_by(|a, b| b.cmp(a));
            Ok(JavaDownloadOptionsDto {
                types: ["jdk", "jre"].iter().map(|s| s.to_string()).collect(),
                majors,
                // 与 adoptium_api::get_os 取值一致
                systems: ["windows", "linux", "alpine-linux", "mac", "aix", "solaris"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                // Adoptium 官方支持的架构集合
                archs: [
                    "x64", "x32", "aarch64", "arm", "ppc64le", "ppc64", "s390x", "riscv64",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            })
        }
        JavaTypes::Zulu => {
            let list = mml_net::zulu_api::get_java_list()
                .await
                .map_err(|e| e.to_string())?;
            // 只从 GA 版本取候选：未发布的主版本（如 28 EA）不该成为默认值
            let ga: Vec<_> = list
                .iter()
                .filter(|item| item.release_status == "ga")
                .collect();

            let mut types: Vec<String> = ga
                .iter()
                .map(|item| item.bundle_type.clone())
                .filter(|item| !item.is_empty())
                .collect();
            types.sort();
            types.dedup();

            let mut majors: Vec<u32> = ga
                .iter()
                .filter_map(|item| item.java_version.first().map(|v| *v as u32))
                .collect();
            majors.sort_unstable();
            majors.dedup();
            majors.reverse();

            let mut systems: Vec<String> = ga
                .iter()
                .map(|item| item.os.clone())
                .filter(|item| !item.is_empty())
                .collect();
            systems.sort();
            systems.dedup();

            let mut archs: Vec<String> = ga
                .iter()
                .map(|item| zulu_arch(&item.arch, &item.hw_bitness))
                .collect();
            archs.sort();
            archs.dedup();

            Ok(JavaDownloadOptionsDto {
                types,
                majors,
                systems,
                archs,
            })
        }
        JavaTypes::OpenJ9 => {
            // openj9_api 内部持有 rquickjs 运行时，future 非 Send，
            // 不能直接跨 Tauri 的异步命令 await；丢到阻塞线程上用
            // block_on 单线程驱动；阻塞线程不是异步 worker，block_on 合法
            let obj = tauri::async_runtime::spawn_blocking(move || {
                tauri::async_runtime::block_on(mml_net::openj9_api::get_java_list())
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
            let mut majors: Vec<u32> = obj
                .main_version
                .iter()
                .filter_map(|item| item.parse().ok())
                .collect();
            majors.sort_unstable_by(|a, b| b.cmp(a));
            // openj9 列表自带空串占位项，即原页面的 any 选项，这里过滤掉
            Ok(JavaDownloadOptionsDto {
                types: ["jdk", "jre"].iter().map(|s| s.to_string()).collect(),
                majors,
                systems: obj.os,
                archs: obj.arch,
            })
        }
    }
}

/// Zulu 的 arch + hw_bitness 组合转架构名，与 ArchEnum 命名一致
///
/// java_download_start 里做反向映射即可还原回 arch + bitness 过滤条件。
///
/// - arch: Zulu 列表里的架构名，如 x86 / arm / ppc
/// - bitness: 位宽字符串，如 64 / 32
///
/// # 返回值
///
/// 统一后的架构名；64 位的三个特例映射成 x86_64 / aarch64 / ppc64，其余原样返回
fn zulu_arch(arch: &str, bitness: &str) -> String {
    match (arch, bitness) {
        ("x86", "64") => "x86_64".to_string(),
        ("arm", "64") => "aarch64".to_string(),
        ("ppc", "64") => "ppc64".to_string(),
        (arch, _) => arch.to_string(),
    }
}

/// 取列表缓存里的下载项
///
/// - uuid: 列表项 id
///
/// # 返回值
///
/// 找到时返回克隆；id 非法或缓存里没有（列表刷新过）时返回 None
fn cached_item(uuid: &str) -> Option<FileItemObj> {
    let id = Uuid::parse_str(uuid).ok()?;
    JAVA_ITEMS.read().unwrap().get(&id).cloned()
}

/// 开始安装：登记任务后立刻返回，下载 / 解压 / 注册在后台跑，进度走 java-install-status
///
/// - app: 应用句柄，广播进度与失败原因用
/// - uuid: 列表项 id，取自 java_download_get_list 返回的 uuid 字段
///
/// # 返回值
///
/// 任务登记成功返回 Ok(())；uuid 非法或缓存里没有该项时返回 Err，内容为 i18n 键
#[gui_macros::command]
pub async fn java_download_start(app: AppHandle, uuid: String) -> Result<(), String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::JAVA_ITEM_NOT_FOUND.to_string());
    };

    let Some(item) = cached_item(&uuid) else {
        return Err(err_keys::JAVA_ITEM_NOT_FOUND.to_string());
    };

    // 同一个包已经在装：直接当作成功，别重复登记
    if task::is_running(id) {
        return Ok(());
    }

    let cancel = task::begin(&app, id, &item.name);
    let name = item.name.clone();
    let task_app = app.clone();
    let finish_app = app.clone();
    let notify_app = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = install_package(id, item, cancel, move |event| match event {
            JavaInstallEvent::Stage(stage) => task::set_stage(&task_app, id, stage),
            JavaInstallEvent::Progress(progress) => task::set_progress(&task_app, id, progress),
        })
        .await;

        // 启动器不在前台时才弹系统通知：界面上本来就有进度与结果
        match &result {
            JavaInstallResult::Done => {
                notify::notify_if_unfocused(&notify_app, NotifyText::JavaDone { name });
            }
            JavaInstallResult::Failed(_) => {
                notify::notify_if_unfocused(&notify_app, NotifyText::JavaFailed { name });
            }
            JavaInstallResult::Cancelled => {}
        }
        task::finish(&finish_app, id, result);
    });

    Ok(())
}

/// 取消一条正在安装的任务
///
/// - uuid: 列表项 id
///
/// # 返回值
///
/// 任务存在且仍在进行时返回 true
#[gui_macros::command]
pub fn java_download_cancel(uuid: String) -> bool {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return false;
    };
    task::cancel(id)
}

/// 当前安装任务列表
///
/// # 返回值
///
/// 按开始顺序排列的任务；窗口打开时先拉一次，之后靠事件更新
#[gui_macros::command]
pub fn java_download_status() -> Vec<JavaInstallTaskDto> {
    task::list()
}

/// 清掉已经结束的安装任务
///
/// - app: 应用句柄
///
/// # 返回值
///
/// 清理后的任务列表
#[gui_macros::command]
pub fn java_download_clear(app: AppHandle) -> Vec<JavaInstallTaskDto> {
    task::clear_finished(&app);
    task::list()
}

/// 下载并注册缓存里的一个 Java 包，不经过任务表
///
/// 与 java_download_start 走同一套安装流程；任务表只负责把进度转成事件，
/// 集成测试与将来的无界面调用可以直接用这里。
///
/// - uuid: 列表项 id
/// - cancel: 取消令牌
/// - report: 阶段与进度回调
///
/// # 返回值
///
/// 见 [JavaInstallResult]
pub async fn install_cached(
    uuid: &str,
    cancel: CancellationToken,
    report: impl Fn(JavaInstallEvent) + Send + Sync + 'static,
) -> JavaInstallResult {
    let Ok(id) = Uuid::parse_str(uuid) else {
        return JavaInstallResult::Failed(err_keys::JAVA_ITEM_NOT_FOUND.to_string());
    };
    let Some(item) = cached_item(uuid) else {
        return JavaInstallResult::Failed(err_keys::JAVA_ITEM_NOT_FOUND.to_string());
    };
    install_package(id, item, cancel, report).await
}

/// 安装主体：下载 zip → 解压 → 识别注册
///
/// - id: 稳定包 id，解压目录按它命名
/// - item: 下载信息
/// - cancel: 取消令牌，下载与解压都会看它
/// - report: 阶段与进度回调
///
/// # 返回值
///
/// 见 [JavaInstallResult]
async fn install_package<F>(
    id: Uuid,
    item: FileItemObj,
    cancel: CancellationToken,
    report: F,
) -> JavaInstallResult
where
    F: Fn(JavaInstallEvent) + Send + Sync + 'static,
{
    let report = Arc::new(report);

    // 下载只负责把 zip 拿到本地，解压单独作为下一段
    report(JavaInstallEvent::Stage(JavaInstallStageDto::Download));
    let Some(handle) =
        mml_downloader::start_download_task_handle(vec![item.clone()], cancel.clone())
    else {
        return JavaInstallResult::Failed(err_keys::DOWNLOAD_FAILED.to_string());
    };

    while !handle.is_finished() {
        if cancel.is_cancelled() {
            break;
        }
        let status = handle.status();
        if status.all_bytes > 0 {
            report(JavaInstallEvent::Progress(
                status.now_bytes as f32 / status.all_bytes as f32,
            ));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    match handle.wait().await {
        mml_downloader::TaskOutcome::Done => {}
        mml_downloader::TaskOutcome::Cancelled => return JavaInstallResult::Cancelled,
        mml_downloader::TaskOutcome::Failed => {
            return JavaInstallResult::Failed(err_keys::DOWNLOAD_FAILED.to_string());
        }
    }
    if cancel.is_cancelled() {
        return JavaInstallResult::Cancelled;
    }

    // 同一个包重下会落到同一个目录：先清掉旧的一份，免得残留文件与新包混在一起
    let dir_name = install_dir_name(&item, id);
    let dir = mml_jvms::get_java_dir().join(&dir_name);
    if dir.exists() && std::fs::remove_dir_all(&dir).is_err() {
        return JavaInstallResult::Failed(err_keys::JAVA_DIR_CLEAN.to_string());
    }

    report(JavaInstallEvent::Stage(JavaInstallStageDto::Extract));
    let gui: BaseArchiveGui = Some(Arc::new(ExtractGui::new(
        report.clone(),
        cancel.clone(),
    )));
    let dir = match mml_jvms::extract_java(Some(dir_name), &item.file, gui).await {
        Ok(dir) => dir,
        Err(err) => {
            return if cancel.is_cancelled() {
                JavaInstallResult::Cancelled
            } else {
                JavaInstallResult::Failed(err.to_string())
            };
        }
    };
    if cancel.is_cancelled() {
        return JavaInstallResult::Cancelled;
    }

    report(JavaInstallEvent::Stage(JavaInstallStageDto::Probe));
    match mml_jvms::register_java_dir(dir).await {
        Ok(Some(_)) => JavaInstallResult::Done,
        // 压缩包能解开但里面没有可运行的 Java（常见于给别的系统 / 架构下的包）
        Ok(None) => JavaInstallResult::Failed(err_keys::JAVA_NOT_REGISTERED.to_string()),
        Err(err) => JavaInstallResult::Failed(err.to_string()),
    }
}

/// 解压目录名：显示名截短后接稳定 id，同一个包重下仍是同一目录
///
/// - item: 下载信息，取它的显示名做可读前缀
/// - id: 稳定包 id
///
/// # 返回值
///
/// 形如 Temurin-21.0.13-9-jdk-windows-x64-1a2b3c4d
fn install_dir_name(item: &FileItemObj, id: Uuid) -> String {
    let mut slug = String::new();
    for ch in item.name.chars() {
        if slug.len() >= 60 {
            break;
        }
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            slug.push(ch);
        } else if ch.is_whitespace() || ch == '+' || ch == '(' || ch == ')' || ch == ',' {
            slug.push('-');
        }
    }
    // 连续的非法字符会拼出一串 "-"，压成单个
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() {
        "java".to_string()
    } else {
        slug
    };
    let id = id.simple().to_string();
    format!("{slug}-{}", &id[..8])
}

/// 解压进度回调，把条目推进折算成任务进度，并支持中途取消
struct ExtractGui<F> {
    /// 进度回调
    report: Arc<F>,
    /// 取消令牌
    cancel: CancellationToken,
    /// 解压总条目数，start 时下发
    total: AtomicUsize,
}

impl<F> ExtractGui<F>
where
    F: Fn(JavaInstallEvent) + Send + Sync,
{
    /// 建一个解压进度回调
    ///
    /// - report: 进度回调
    /// - cancel: 取消令牌
    ///
    /// # 返回值
    ///
    /// 给 mml_jvms::extract_java 用的回调
    fn new(report: Arc<F>, cancel: CancellationToken) -> Self {
        Self {
            report,
            cancel,
            total: AtomicUsize::new(0),
        }
    }
}

impl<F> IBaseArchiveGui for ExtractGui<F>
where
    F: Fn(JavaInstallEvent) + Send + Sync,
{
    /// 记下总条目数
    fn start(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        (self.report)(JavaInstallEvent::Progress(0.0));
    }

    /// 按已完成条目推进进度
    fn update(&self, _filename: Option<String>, current: usize) {
        let total = self.total.load(Ordering::Relaxed);
        if total > 0 {
            (self.report)(JavaInstallEvent::Progress(
                current as f32 / total as f32,
            ));
        }
    }

    /// 解压完成：进度拉满
    fn done(&self) {
        (self.report)(JavaInstallEvent::Progress(1.0));
    }

    /// 用户取消：让解压循环尽快收手
    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// 条目名含非法字符时一律替换，安装流程不弹窗
    fn file_rename(&self, _name: &str) -> bool {
        true
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use mml_base::file_item::{FileHash, LaterRun};
    use std::path::PathBuf;

    #[test]
    fn install_dir_name_is_stable_and_sanitized() {
        let item = FileItemObj {
            name: "Temurin 21 (jre, windows amd64, zip)".to_string(),
            file: PathBuf::new(),
            url: "https://example.com/x.zip".to_string(),
            hash: FileHash::None,
            later: LaterRun::None,
        };
        let id = Uuid::parse_str("00000000-0000-0000-0000-00000000000e").unwrap();
        let name = install_dir_name(&item, id);
        assert!(
            name.starts_with("Temurin-21-jre-windows-amd64-zip-"),
            "{name}"
        );
        assert!(!name.contains(' '), "{name}");
        assert_eq!(name, install_dir_name(&item, id));
    }
}
