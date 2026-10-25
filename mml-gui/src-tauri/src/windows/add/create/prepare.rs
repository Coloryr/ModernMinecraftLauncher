//! 待建任务的准备
//!
//! 建实例前先把要装的东西理成一份清单：目录 / 压缩包先扫一遍，
//! 认得出整合包就按整合包走，否则按模组文件走。

use std::sync::{Arc, Mutex};

use tauri::WebviewWindow;
use tokio_util::sync::CancellationToken;

use mml_game::add_game::PackType;

use crate::dtos::{DetectedPackDto, DirEntry, FolderInstanceDto};

use crate::windows::add::model::{AddWindowModel, model};

/// 一次安装任务的前置上下文：窗口模型句柄与这次任务的取消令牌
pub(crate) struct PreparedTask {
    /// 本窗口模型句柄，与窗口注册表里那份是同一个；持有它即保证安装期间模型不被释放
    pub(crate) model: Arc<Mutex<AddWindowModel>>,
    /// 本次任务的取消令牌，已通过 `set_cancel` 写进模型
    pub(crate) token: CancellationToken,
}

/// 任务前置检查：确认窗口模型存在，即窗口已开，并返回模型与新任务的取消令牌
///
/// HTTP 客户端由 `mml_core::load` 保证就绪，这里不再重复检查；原先的注释说检查了，
/// 实际只查了模型。
///
/// - `window`: 调用方窗口，用于取本窗口模型
///
/// # 返回值
///
/// 见 [`PreparedTask`]；模型缺失时返回 `err.modelMissing`
pub(crate) fn prepare(window: &WebviewWindow) -> Result<PreparedTask, String> {
    let store = model(window)?;
    let token = CancellationToken::new();
    store.lock().unwrap().set_cancel(token.clone());
    Ok(PreparedTask {
        model: store,
        token,
    })
}

/// 该压缩包类型是否为整合包，即内核里走整合包安装 worker 的那两种 manifest
///
/// - `pack`: 压缩包类型
///
/// # 返回值
///
/// CurseForge 与 Modrinth 返回 `true`，其余类型为 `false`
pub(crate) fn is_modpack(pack: &PackType) -> bool {
    matches!(pack, PackType::CurseForge | PackType::Modrinth)
}

/// 取压缩包文件名并去掉路径与扩展名：本地包没有项目元数据，任务显示名只能取它
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 文件名主干；取不到时原样返回传入的路径
pub(crate) fn pack_file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|item| item.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// 列出目录的直接内容，目录优先，再按名称排序
///
/// 添加实例窗口选择文件夹时调用，用于预览文件夹内容树
///
/// - `path`: 目录路径
///
/// # 返回值
///
/// 直接子项列表，只含名字与是否目录；目录打不开返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub fn add_list_dir(path: String) -> Result<Vec<DirEntry>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(DirEntry { name, is_dir });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// 扫描文件夹里可导入的实例，覆盖官方启动器 `versions/*` 与 MMC `instances/*`
///
/// 选到 `.minecraft` 这类**装着若干实例**的目录时用：一个目录一个实例，
/// 由前端列出来让用户勾选要导入哪些，而不是把整个 `.minecraft` 当成一个实例。
/// 扫描规则见 `mml_game::scan_game_from_path`。
///
/// **必须异步**：扫描是纯磁盘活，要读并解析各候选目录里的 json，modded 实例的
/// `config/` 下可能几百个；同步命令会占着主线程，窗口直接卡死 ——
/// 放 `spawn_blocking` 里跑，界面照常响应。
///
/// - `path`: 待扫描的目录路径
///
/// # 返回值
///
/// 扫到的实例列表，每项含实例名与目录路径；扫描线程出错时返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_scan_folder(path: String) -> Result<Vec<FolderInstanceDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mml_game::scan_game::scan_game_from_path(&path)
            .into_iter()
            .map(|item| FolderInstanceDto {
                name: item
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: item.display().to_string(),
            })
            .collect::<Vec<FolderInstanceDto>>()
    })
    .await
    .map_err(|e| e.to_string())
}

/// 列出压缩包内的条目路径，目录以 `/` 结尾，添加实例窗口预览内容树用
///
/// **必须异步**：要打开压缩包并读出全部条目，整合包里几千个文件时会明显耗时，
/// 同步命令占主线程会把窗口冻住。
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 包内全部条目的名字；打不开压缩包返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_list_archive(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let archive = mml_base::archives::BaseArchive::open(&path).map_err(|e| e.to_string())?;
        Ok(archive
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<String>>())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 检测压缩包的整合包类型与推荐实例名，选择压缩包后自动填表用
///
/// **必须异步**：要打开压缩包扫条目、再解析包内 manifest，同 `add_list_archive`。
///
/// - `path`: 压缩包路径
///
/// # 返回值
///
/// 探测结果；无法识别或读包失败返回 `Err`
#[gui_macros::ipc_group("add")]
#[gui_macros::command]
pub async fn add_detect_archive(path: String) -> Result<DetectedPackDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let pack = mml_game::add_game::detect_pack(&path).map_err(|e| e.to_string())?;
        Ok(DetectedPackDto {
            pack_type: pack.pack_type.id().to_string(),
            name: pack.name,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
