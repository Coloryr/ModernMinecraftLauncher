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

/// 预览可选的窗口及启动器里的最小客户区尺寸
///
/// 数值与 mml-gui/src-tauri/src/windows/registry/spec.rs 保持一致；
/// 单测会直接读那份源码校验，改了启动器尺寸这里会失败。
pub fn min_size_of_kind(kind: &str) -> WindowMinSize {
    match kind {
        "main" => WindowMinSize {
            width: 920.0,
            height: 600.0,
        },
        "settings" => WindowMinSize {
            width: 850.0,
            height: 600.0,
        },
        "account" => WindowMinSize {
            width: 800.0,
            height: 650.0,
        },
        "add" => WindowMinSize {
            width: 700.0,
            height: 585.0,
        },
        "resource" => WindowMinSize {
            width: 800.0,
            height: 510.0,
        },
        "download" => WindowMinSize {
            width: 670.0,
            height: 470.0,
        },
        "log" => WindowMinSize {
            width: 800.0,
            height: 480.0,
        },
        // 启动器的通用窗口下限
        _ => WindowMinSize {
            width: 640.0,
            height: 480.0,
        },
    }
}

/// 预览可选窗口的最小尺寸表
pub fn min_sizes() -> BTreeMap<String, WindowMinSize> {
    [
        "main", "settings", "account", "add", "resource", "download", "log",
    ]
    .into_iter()
    .map(|kind| (kind.to_string(), min_size_of_kind(kind)))
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
mod tests {
    use super::*;

    /// worker 协议行只有带前缀的才解析
    #[test]
    fn parses_worker_lines() {
        assert!(matches!(
            parse_worker_line(r#"@@mml-studio@@{"type":"ready","url":"http://127.0.0.1:1/"}"#),
            Some(WorkerEvent::Ready(url)) if url == "http://127.0.0.1:1/"
        ));
        assert!(matches!(
            parse_worker_line(r#"@@mml-studio@@{"type":"failed","error":"boom"}"#),
            Some(WorkerEvent::Failed(error)) if error == "boom"
        ));
        assert!(parse_worker_line("vite v8 ready").is_none());
    }

    /// 预览最小尺寸必须和启动器窗口注册表一致
    #[test]
    fn min_sizes_match_launcher_registry() {
        const SPEC: &str = include_str!("../../../mml-gui/src-tauri/src/windows/registry/spec.rs");

        fn value(source: &str, name: &str) -> f64 {
            let marker = format!("const {name}: f64 = ");
            let start = source
                .find(&marker)
                .unwrap_or_else(|| panic!("launcher registry is missing {name}"))
                + marker.len();
            let end = source[start..].find(';').unwrap();
            source[start..start + end].trim().parse().unwrap()
        }

        for (kind, width, height) in [
            ("main", "MAIN_MIN_WIDTH", "MAIN_MIN_HEIGHT"),
            ("settings", "SETTINGS_MIN_WIDTH", "SETTINGS_MIN_HEIGHT"),
            ("account", "ACCOUNT_MIN_WIDTH", "ACCOUNT_MIN_HEIGHT"),
            ("add", "ADD_MIN_WIDTH", "ADD_MIN_HEIGHT"),
            ("resource", "RESOURCE_MIN_WIDTH", "RESOURCE_MIN_HEIGHT"),
            ("download", "DOWNLOAD_MIN_WIDTH", "DOWNLOAD_MIN_HEIGHT"),
            ("log", "LOG_MIN_WIDTH", "LOG_MIN_HEIGHT"),
        ] {
            let size = min_size_of_kind(kind);
            assert_eq!(size.width, value(SPEC, width), "{kind} width");
            assert_eq!(size.height, value(SPEC, height), "{kind} height");
        }
    }
}
