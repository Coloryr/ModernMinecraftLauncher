use super::*;

#[test]
fn parses_select_options() {
    let html = "<select id='arch'><option value='any'>Any</option><option value='x64'>x64</option></select>";
    let list = select_options(html, "arch").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].value, "any");
    assert_eq!(list[1].value, "x64");
}

#[test]
fn keeps_option_class() {
    let html = "<select id='runtimeVersion'><option value='8' class='bx--select-option'>8</option></select>";
    let list = select_options(html, "runtimeVersion").unwrap();
    assert!(list[0].class.contains("bx--select-option"));
}

#[test]
fn missing_select_returns_none() {
    assert!(select_options("<select id='os'></select>", "arch").is_none());
}

#[test]
fn converts_js_literal_to_json() {
    let script = "let sourceDataJson = {\n  downloads: [\n    { name: 'a&b', version: 8, os: 'Linux', arch: 'x64', },\n  ],\n};\n";
    let json = js_object_to_json(script).unwrap();
    assert!(!json.contains("downloads:"));
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["downloads"][0]["name"], "a&b");
    assert_eq!(value["downloads"][0]["version"], 8);
}

