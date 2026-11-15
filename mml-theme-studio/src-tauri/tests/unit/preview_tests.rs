use super::*;
use std::collections::BTreeSet;

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

/// 取 `const NAME: f64 = 1.0;` 的数值
fn constant(source: &str, name: &str) -> f64 {
    let marker = format!("const {name}: f64 = ");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("launcher registry is missing {name}"))
        + marker.len();
    let end = source[start..].find(';').unwrap();
    source[start..start + end].trim().parse().unwrap()
}

/// 取一条 WindowEntry 里的字段，读到行尾或逗号为止
fn field<'a>(entry: &'a str, name: &str) -> &'a str {
    let start = entry
        .find(name)
        .unwrap_or_else(|| panic!("window entry is missing {name}"))
        + name.len();
    entry[start..].split(['\n', ',']).next().unwrap().trim()
}

/// 启动器注册表里的 kind 与最小尺寸
fn launcher_windows(source: &str) -> Vec<(&str, f64, f64)> {
    let body = &source[source.find("static WINDOWS_INFO").unwrap()..];
    body.split("WindowEntry {")
        .skip(1)
        .map(|entry| {
            let label = field(entry, "label: ").trim_matches('"');
            let kind = label
                .strip_prefix("mml-")
                .unwrap_or_else(|| panic!("unexpected window label: {label}"));
            (
                kind,
                constant(source, field(entry, "min_width: ")),
                constant(source, field(entry, "min_height: ")),
            )
        })
        .collect()
}

/// 预览窗口清单与最小尺寸必须和启动器窗口注册表一致
#[test]
fn matches_launcher_registry() {
    const SPEC: &str = include_str!("../../../../mml-gui/src-tauri/src/windows/common/registry/spec.rs");
    let windows = launcher_windows(SPEC);

    let expected: BTreeSet<&str> = windows.iter().map(|(kind, _, _)| *kind).collect();
    let actual: BTreeSet<&str> = WINDOW_KINDS.iter().copied().collect();
    assert_eq!(actual, expected, "预览窗口清单与启动器注册表不一致");

    for (kind, width, height) in windows {
        let size = min_size_of_kind(kind);
        assert_eq!((size.width, size.height), (width, height), "{kind} 最小尺寸");
    }

    let sizes = min_sizes();
    assert_eq!(sizes.len(), WINDOW_KINDS.len());
    for kind in WINDOW_KINDS {
        assert!(sizes.contains_key(*kind), "{kind} 缺少最小尺寸");
    }
}

