//! 设置窗口的 Java 运行时：扫描 / 添加 / 删除 / 导入压缩包 / 探测
//!
//! 从 `settings/mod.rs` 拆出来的。Java 列表由 `mml-jvms` 持有并即时持久化，
//! 这里只做命令与进度事件。命令带分组标记，值填原始组名
//! `settings`，免得组键随文件名变掉，见 AGENTS.md §4。

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mml_base::archives::IBaseArchiveGui;
use mml_jvms::java_helper;
use tauri::{AppHandle, Emitter};

use crate::dtos::{JavaImportProgressDto, JavaInfoDto};
use crate::listens;

/// 扫描系统已安装的 Java 并持久化到配置
///
/// # 返回值
///
/// 扫描后的完整 Java 列表
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub fn settings_scan_java() -> Vec<JavaInfoDto> {
    mml_jvms::scan_java();
    java_list()
}

/// 手动添加一个 Java
///
/// - `name`: 显示名；`None` 时按探测结果取名
/// - `path`: Java 可执行文件路径
///
/// # 返回值
///
/// 新增的 Java 项；路径无效返回 `err.javaInvalid`
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub fn settings_add_java(name: Option<String>, path: String) -> Result<JavaInfoDto, String> {
    if let Some(added) = mml_jvms::add_item(name, path) {
        Ok(JavaInfoDto {
            name: added.name.clone(),
            path: added.path.to_string_lossy().to_string(),
            version: added.version.clone(),
            major: added.major_version.max(0),
            java_type: added.java_type.clone(),
            arch: added.arch.to_string(),
        })
    } else {
        Err("err.javaInvalid".to_string())
    }
}

/// 移除一个 Java
///
/// - `name`: 要移除项的显示名，也是列表里的唯一键
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub fn settings_remove_java(name: String) {
    mml_jvms::remove(&name);
}

/// 移除全部 Java
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub fn settings_remove_all_java() {
    mml_jvms::remove_all();
}

/// Java 压缩包导入进度事件，解包与识别阶段推进时发
///
/// - `app`: 应用句柄，事件发往所有窗口
/// - `dto`: 当前阶段、进度与文件名
#[gui_macros::emit]
pub(super) fn emit_settings_java_progress(app: &AppHandle, dto: JavaImportProgressDto) {
    let _ = app.emit(listens::SETTINGS_JAVA_PROGRESS, dto);
}

/// 解压进度回调 → `settings-java-progress` 事件桥接
///
/// 给 `mml_jvms::unzip_java` 的 `BaseArchiveGui` 参数用：
/// `start` 下发的 total 记在内部，`update` 携带它一起转发成进度事件，
/// 当前文件名放进 `subText`；命令返回即导入完成，`done` 无需再发。
pub(super) struct JavaArchiveGui {
    /// 应用句柄，进度事件发往所有窗口
    app: AppHandle,
    /// `start` 下发的总数，`update` 时回读
    total: AtomicUsize,
}

/// 从压缩包解包导入 Java，支持 zip / 7z / tar.gz 等
///
/// 解包与识别过程中经 `settings-java-progress` 事件上报进度。
///
/// - `app`: 应用句柄，用于上报进度事件
/// - `name`: 显示名；`None` 时按探测结果取名
/// - `archive_path`: 本地压缩包路径
///
/// # 返回值
///
/// 导入成功的 Java 项；包里没有可用的 Java 时返回 `None`；解包失败返回错误信息
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub async fn settings_import_java(
    app: AppHandle,
    name: Option<String>,
    archive_path: String,
) -> Result<Option<JavaInfoDto>, String> {
    let data = mml_jvms::unzip_java(
        name,
        Path::new(&archive_path),
        Some(JavaArchiveGui::new(&app)),
    )
    .await
    .map_err(|err| err.to_string())?;
    if let Some(data) = data {
        Ok(Some(JavaInfoDto {
            name: data.name.clone(),
            path: data.path.to_string_lossy().to_string(),
            version: data.version.clone(),
            major: data.major_version,
            java_type: data.java_type.clone(),
            arch: data.arch.to_string(),
        }))
    } else {
        Ok(None)
    }
}

/// 扫描指定文件夹里的 Java 并注册，含子目录
///
/// - `path`: 要扫描的目录路径
///
/// # 返回值
///
/// 注册成功的 Java 项；目录里没找到 Java 时返回 `None`
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub async fn settings_scan_java_dir(path: String) -> Result<Option<JavaInfoDto>, String> {
    let data = mml_jvms::find_java_from_path(Path::new(&path));

    let Some(data) = data else {
        return Ok(None);
    };

    let data = tauri::async_runtime::spawn(async move { java_helper::test_java(Path::new(&data)) })
        .await
        .map_err(|err| err.to_string())?;

    if let Some(info) = data {
        let info = mml_jvms::add_info_item(info);
        Ok(Some(JavaInfoDto {
            name: info.name.clone(),
            path: info.path.to_string_lossy().to_string(),
            version: info.version.clone(),
            major: info.major_version,
            java_type: info.java_type.clone(),
            arch: info.arch.to_string(),
        }))
    } else {
        Ok(None)
    }
}

/// 探测路径指向的 Java，只取信息不注册
///
/// - `path`: Java 可执行文件路径
///
/// # 返回值
///
/// 识别到的显示名；探测不出 Java 时返回 `None`
#[gui_macros::ipc_group("settings")]
#[tauri::command]
pub async fn settings_detect_java(path: String) -> Result<Option<String>, String> {
    let data = tauri::async_runtime::spawn(async move { java_helper::test_java(Path::new(&path)) })
        .await
        .map_err(|err| err.to_string())?;

    if let Some(data) = data {
        Ok(Some(data.name))
    } else {
        Ok(None)
    }
}

/// mml-jvms 内存列表 → 前端 DTO
///
/// # 返回值
///
/// 当前全部 Java 的 DTO，与 windows::main 的 java_list 同构
pub(super) fn java_list() -> Vec<JavaInfoDto> {
    mml_jvms::get_all_java()
        .iter()
        .map(|j| JavaInfoDto {
            name: j.name.clone(),
            path: j.path.to_string_lossy().to_string(),
            version: j.version.clone(),
            major: j.major_version.max(0),
            java_type: j.java_type.clone(),
            arch: j.arch.to_string(),
        })
        .collect()
}

impl JavaArchiveGui {
    /// 按应用句柄建一个进度桥接器，总数从 0 起
    ///
    /// - `app`: 应用句柄，进度事件发往所有窗口
    ///
    /// # 返回值
    ///
    /// 共享的桥接器实例，交给 `mml_jvms::unzip_java` 用
    fn new(app: &AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app: app.clone(),
            total: AtomicUsize::new(0),
        })
    }
}

impl IBaseArchiveGui for JavaArchiveGui {
    /// 解包开始：记下总条目数并发出 `extract` 阶段的首个进度事件
    ///
    /// - `total`: 压缩包内待解包的条目总数
    fn start(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        emit_settings_java_progress(
            &self.app,
            JavaImportProgressDto {
                state: "extract".to_string(),
                now: 0,
                total: total as u32,
                sub_text: None,
            },
        );
    }

    /// 解包推进：转发当前进度
    ///
    /// - `filename`: 当前正在解包的条目名；没有名字时为 `None`
    /// - `current`: 已完成的条目数
    fn update(&self, filename: Option<String>, current: usize) {
        emit_settings_java_progress(
            &self.app,
            JavaImportProgressDto {
                state: "extract".to_string(),
                now: current as u32,
                total: self.total.load(Ordering::Relaxed) as u32,
                sub_text: filename,
            },
        );
    }

    /// 解包结束：命令返回即导入完成，这里不额外发事件
    fn done(&self) {}

    /// 导入场景无需询问，非法字符文件名直接同意替换
    ///
    /// - `_name`: 待替换的文件名，本实现不关心
    ///
    /// # 返回值
    ///
    /// 恒为 `true`，即总是同意替换
    fn file_rename(&self, _name: &str) -> bool {
        true
    }
}
