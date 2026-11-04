//! Java 下载窗口：Java 运行时下载的规格 + IPC 命令
//!
//! 搜索源 Adoptium / Zulu / OpenJ9 / Foojay 选定后，经 `java_download_get_options`
//! 取该源的四个下拉框候选，即发行类型 / 主版本 / 系统 / 架构，筛选拟下载的 Java，
//! 选好后经 `java_download_start` 触发下载，完成后经 `java-change` 事件
//! 通知各窗口刷新 Java 列表，与手动添加是同一条刷新链路。

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, RwLock},
    time::Duration,
};

use mml_base::file_item::FileItemObj;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::dtos::{JavaDownloadOptionsDto, JavaInstallStageDto, JavaInstallTaskDto, JavaTypes};
use crate::err_keys;

// 带命令的子模块必须 `pub(crate)`：生成的 `tauri_commands!` 从 crate 根引用它
pub(crate) mod list;
pub(crate) mod task;

// 集成测试 tests/java_download_test.rs 按原路径引用它，故再导出
pub use self::list::java_download_get_list;

/// Java 包列表缓存，键是列表项 uuid、值是待下载项目；列表查询填充，开始下载时取出
pub(super) static JAVA_ITEMS: LazyLock<RwLock<HashMap<Uuid, FileItemObj>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

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
/// - Foojay：`/parameters` 接口，四组候选齐全
/// - Adoptium：主版本动态拉取，类型 / 系统 / 架构为其支持的固定集合
/// - Zulu：从包列表提取，`bundle_type` 区分 jdk / jre，主版本取 `java_version` 首段
/// - OpenJ9：页面抓取结果自带 arch / os / 主版本候选
///
/// - `source`: 搜索源
///
/// # 返回值
///
/// 该源的四个下拉框候选，即发行类型 / 主版本 / 系统 / 架构；网络失败时返回 `Err`
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

            let mut types: Vec<String> = list
                .iter()
                .map(|item| item.bundle_type.clone())
                .filter(|item| !item.is_empty())
                .collect();
            types.sort();
            types.dedup();

            let mut majors: Vec<u32> = list
                .iter()
                .filter_map(|item| item.java_version.first().map(|v| *v as u32))
                .collect();
            majors.sort_unstable();
            majors.dedup();
            majors.reverse();

            let mut systems: Vec<String> = list
                .iter()
                .map(|item| item.os.clone())
                .filter(|item| !item.is_empty())
                .collect();
            systems.sort();
            systems.dedup();

            let mut archs: Vec<String> = list
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
            // openj9 列表自带 "" 占位项，即原页面的 any 选项，这里过滤掉
            Ok(JavaDownloadOptionsDto {
                types: ["jdk", "jre"].iter().map(|s| s.to_string()).collect(),
                majors,
                systems: obj.os,
                archs: obj.arch,
            })
        }
    }
}

/// Zulu 的 `arch` + `hw_bitness` 组合转架构名，与 ArchEnum 命名一致
///
/// `java_download_start` 里做反向映射即可还原回 arch + bitness 过滤条件。
///
/// - `arch`: Zulu 列表里的架构名，如 `x86` / `arm` / `ppc`
/// - `bitness`: 位宽字符串，如 `64` / `32`
///
/// # 返回值
///
/// 统一后的架构名；64 位的三个特例映射成 `x86_64` / `aarch64` / `ppc64`，其余原样返回
fn zulu_arch(arch: &str, bitness: &str) -> String {
    match (arch, bitness) {
        ("x86", "64") => "x86_64".to_string(),
        ("arm", "64") => "aarch64".to_string(),
        ("ppc", "64") => "ppc64".to_string(),
        (arch, _) => arch.to_string(),
    }
}

/// 开始安装：登记任务后立刻返回，下载 / 解压 / 注册在后台跑，进度走 `java-install-status`
///
/// - `app`: 应用句柄，广播进度与失败原因用
/// - `uuid`: 列表项 uuid，取自 `java_download_get_list` 返回的 `uuid` 字段
///
/// # 返回值
///
/// 任务登记成功返回 `Ok(())`；uuid 非法或缓存里没有该项时返回 `Err`，内容为 i18n 键
#[gui_macros::command]
pub async fn java_download_start(app: AppHandle, uuid: String) -> Result<(), String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::JAVA_ITEM_NOT_FOUND.to_string());
    };

    let item = {
        let locker = JAVA_ITEMS.read().unwrap();
        locker.get(&id).cloned()
    };
    let Some(item) = item else {
        return Err(err_keys::JAVA_ITEM_NOT_FOUND.to_string());
    };

    // 同一个包已经在装：直接当作成功，别重复登记
    if task::is_running_name(&item.name) {
        return Ok(());
    }

    task::begin(&app, id, &item.name);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = install(&handle, id, item).await;
        task::finish(&handle, id, result.err());
    });
    Ok(())
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
/// - `app`: 应用句柄
///
/// # 返回值
///
/// 清理后的任务列表
#[gui_macros::command]
pub fn java_download_clear(app: AppHandle) -> Vec<JavaInstallTaskDto> {
    task::clear_finished(&app);
    task::list()
}

/// 安装主体：下载 zip → 解压 → 识别注册
///
/// - `app`: 应用句柄
/// - `id`: 列表项 uuid
/// - `item`: 下载信息
///
/// # 返回值
///
/// 成功返回 `Ok(())`；任一步失败时返回 i18n 键或原始说明
async fn install(app: &AppHandle, id: Uuid, item: FileItemObj) -> Result<(), String> {
    // 下载只负责把 zip 拿到本地，解压单独作为下一段
    task::set_stage(app, id, JavaInstallStageDto::Download);
    let Some(handle) = mml_downloader::start_download_task_handle(
        vec![item.clone()],
        CancellationToken::new(),
    ) else {
        return Err(err_keys::DOWNLOAD_FAILED.to_string());
    };
    while !handle.is_finished() {
        let status = handle.status();
        if status.all_bytes > 0 {
            task::set_progress(

                app,
                id,
                status.now_bytes as f32 / status.all_bytes as f32,
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if !handle.wait().await.is_done() {
        return Err(err_keys::DOWNLOAD_FAILED.to_string());
    }

    task::set_stage(app, id, JavaInstallStageDto::Extract);
    let gui = Arc::new(task::ExtractGui::new(app, id));
    let dir = mml_jvms::extract_java(Some(item.name.clone()), &item.file, Some(gui))
        .await
        .map_err(|err| err.to_string())?;

    task::set_stage(app, id, JavaInstallStageDto::Probe);
    mml_jvms::register_java_dir(dir)
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}
