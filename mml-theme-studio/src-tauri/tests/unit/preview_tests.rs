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
    const SPEC: &str = include_str!("../../../../mml-gui/src-tauri/src/windows/common/registry/spec.rs");

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

