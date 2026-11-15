//! 预览 worker 的消息协议、运行时状态与预览窗口最小尺寸
//!
//! 预览页面通过 iframe 嵌在编辑器前端里（见 src/App.vue），
//! Rust 只负责启动 / 停止 Node 预览服务，不再创建独立窗口。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 预览 worker 在 stdout 上的协议前缀
const WORKER_MARKER: &str = "@@mml-studio@@";

/// 窗口最小客户区尺寸，单位逻辑像素
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowMinSize {
    /// 最小宽度
    pub width: f64,
    /// 最小高度
    pub height: f64,
}

impl WindowMinSize {
    /// 构造一条最小尺寸
    const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// 预览可选的窗口，顺序与启动器前端的窗口注册表一致
///
/// 启动器新增窗口后这里要跟着补，单测会对着启动器注册表校验集合，
/// 漏掉或多出一条都会失败。
pub const WINDOW_KINDS: &[&str] = &[
    "lobby",
    "main",
    "settings",
    "stats",
    "help",
    "resource",
    "account",
    "add",
    "add_modpack",
    "add_resource",
    "collect",
    "download",
    "java_download",
    "block",
    "log",
    "export",
    "config",
    "server_pack",
    "server_instance",
];

/// 预览可选的窗口及启动器里的最小客户区尺寸
///
/// 数值与 mml-gui/src-tauri/src/windows/common/registry/spec.rs 保持一致；
/// 单测会直接读那份源码校验，改了启动器尺寸这里会失败。
pub fn min_size_of_kind(kind: &str) -> WindowMinSize {
    match kind {
        "lobby" => WindowMinSize::new(780.0, 600.0),
        "main" => WindowMinSize::new(920.0, 600.0),
        "settings" => WindowMinSize::new(850.0, 600.0),
        "stats" => WindowMinSize::new(780.0, 600.0),
        "help" => WindowMinSize::new(640.0, 480.0),
        "resource" => WindowMinSize::new(800.0, 510.0),
        "account" => WindowMinSize::new(800.0, 650.0),
        "add" => WindowMinSize::new(700.0, 585.0),
        "add_modpack" => WindowMinSize::new(920.0, 600.0),
        "add_resource" => WindowMinSize::new(800.0, 510.0),
        "collect" => WindowMinSize::new(640.0, 480.0),
        "download" => WindowMinSize::new(670.0, 470.0),
        "java_download" => WindowMinSize::new(520.0, 420.0),
        "block" => WindowMinSize::new(770.0, 560.0),
        "log" => WindowMinSize::new(800.0, 480.0),
        "export" => WindowMinSize::new(800.0, 640.0),
        "config" => WindowMinSize::new(820.0, 560.0),
        "server_pack" => WindowMinSize::new(640.0, 420.0),
        "server_instance" => WindowMinSize::new(860.0, 500.0),
        // 启动器的通用窗口下限
        _ => WindowMinSize::new(640.0, 480.0),
    }
}

/// 预览可选窗口的最小尺寸表
pub fn min_sizes() -> BTreeMap<String, WindowMinSize> {
    WINDOW_KINDS
        .iter()
        .map(|kind| ((*kind).to_string(), min_size_of_kind(kind)))
        .collect()
}

/// 预览 worker 发回的消息
pub enum WorkerEvent {
    /// 服务已就绪，参数为访问地址
    Ready(String),
    /// 启动失败
    Failed(String),
}

/// 解析 worker 的一行输出
///
/// - `line`: stdout / stderr 上的一行
///
/// # 返回值
///
/// 不是协议行时返回 None，由调用方当普通日志转发
pub fn parse_worker_line(line: &str) -> Option<WorkerEvent> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(rename = "type")]
        kind: String,
        url: Option<String>,
        error: Option<String>,
    }
    let payload = line.strip_prefix(WORKER_MARKER)?;
    let message: Raw = serde_json::from_str(payload).ok()?;
    match message.kind.as_str() {
        "ready" => Some(WorkerEvent::Ready(message.url.unwrap_or_default())),
        "failed" => Some(WorkerEvent::Failed(message.error.unwrap_or_default())),
        _ => None,
    }
}

/// 预览运行时状态
#[derive(Default)]
pub struct PreviewState {
    /// 当前预览地址；空串表示没有预览
    pub url: String,
    /// worker 进程 pid
    pub pid: Option<u32>,
}

#[cfg(test)]
#[path = "../tests/unit/preview_tests.rs"]
mod tests;
