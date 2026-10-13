//! 主窗口：新闻 / 游戏事件模型 + 实例数据存储 + IPC 命令，即窗口按钮调用的方法
//!
//! 数据从 Rust 侧获取：实例 / Java / 版本存储在 `MainWindowModel`，
//! 持久化到应用数据目录的 `main_data.json`；前端通过 IPC 调用本模块命令。
//!
//! **分组不在这里**：组名归属与两种顺序都由内核分组表
//! `mml_game::game_group` 的 `group_save.json` 持有，本模块只转发。

mod args;
mod catalog;
// 带命令的子模块必须 `pub(crate)`：生成的 `tauri_commands!` 从 crate 根引用它们
pub(crate) mod instance;
pub(crate) mod launch;
pub(crate) mod query;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::dtos::main_dto::LoadState;
use crate::dtos::{ErrorEvent, InstanceArgsDto, InstanceChangeEvent, InstanceInfoDto};
use crate::err_keys;
use crate::{listens, windows};

// block.rs 也用它，故按原路径再导出，供 use super::main::core_instance 取到
pub(crate) use self::args::core_instance;

/// 核心加载完成事件：ok 为加载成功，error 为失败信息，前端据此显示错误页
///
/// - `app`: 应用句柄，事件的发送方
/// - `data`: 加载失败原因；`None` 表示加载成功
#[gui_macros::emit]
pub fn emit_load_done(app: &AppHandle, data: Option<String>) {
    let _ = app.emit(
        listens::LOAD_DONE,
        LoadState {
            ok: data.is_none(),
            error: data,
        },
    );
}

/// 主窗口数据存储：实例 / 启动参数 / 运行状态 / 日志
pub struct MainWindowModel {
    /// 实例列表，遗留数据的兜底存储；核心实例以 mml-game 为准
    pub instances: Vec<InstanceInfoDto>,
    /// 遗留实例的启动参数，键为实例 uuid
    pub args: HashMap<String, InstanceArgsDto>,
    /// 运行中的实例 uuid
    pub running: HashSet<String>,
}

impl MainWindowModel {
    /// 创建空模型，实例 / 参数 / 运行态都为空
    ///
    /// # 返回值
    ///
    /// 尚未载入任何实例的模型
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            args: HashMap::new(),
            running: HashSet::new(),
        }
    }
}

impl Default for MainWindowModel {
    /// `Default` 实现：创建空模型，等价于 `new`
    fn default() -> Self {
        Self::new()
    }
}

// ================= IPC 命令 =================

/// 取主窗口模型，模型跟随主窗口生命周期，开窗创建、关窗销毁，见 `windows`
///
/// 这些命令只会由主窗口 webview 调用；模型缺失属异常情形，即主窗口未创建，
/// 返回 Err / 空列表由调用方降级，不 panic。
///
/// - `window`: 调用方窗口，模型按窗口标签取
///
/// # 返回值
///
/// 取不到模型时返回 `err.modelMissing`
pub(super) fn model(window: &WebviewWindow) -> Result<Arc<Mutex<MainWindowModel>>, String> {
    windows::window_model(window).ok_or_else(|| err_keys::MODEL_MISSING.to_string())
}

/// 实例数据变更事件
///
/// - `app`: 应用句柄，事件的发送方
/// - `type`: 变更种类，取值 `add` / `edit` / `remove` / `group`
#[gui_macros::emit]
pub fn emit_instance_change(app: &AppHandle, r#type: &str) {
    let _ = app.emit(
        listens::INSTANCE_CHANGE,
        InstanceChangeEvent {
            r#type: r#type.into(),
        },
    );
}

/// Java 列表变更事件，由 mml_jvms 回调触发：添加 / 删除 / 配置加载完成
///
/// - `app`: 应用句柄，事件的发送方
#[gui_macros::emit]
pub fn emit_java_change(app: &AppHandle) {
    let _ = app.emit(listens::JAVA_CHANGE, ());
}

/// 启动失败事件，预留：接入 mml-core 启动链后使用
///
/// - `app`: 应用句柄，事件的发送方
/// - `event`: 失败详情，含错误文案与实例 uuid；启动前的错误没有 uuid
#[allow(dead_code)]
#[gui_macros::emit]
fn emit_launch_error(app: &AppHandle, event: ErrorEvent) {
    let _ = app.emit(listens::LAUNCH_ERROR, event);
}
