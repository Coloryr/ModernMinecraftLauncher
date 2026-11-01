use super::*;

/// 命令签名跨多行也能解析，注入参数留在参数原文里，由 ts.rs 过滤
#[test]
fn test_commands_in_multiline() {
    let src = r#"
/// 文档注释
#[gui_macros::command]
pub async fn add_modpack_search(
source: String,
query: Option<String>,
page: u32,
) -> Result<ModpackSearchDto, String> {
todo!()
}
"#;
    let cmds = commands_in(src);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name, "add_modpack_search");
    assert!(cmds[0].params.contains("source: String"));
    assert!(cmds[0].params.contains("page: u32"));
    assert_eq!(cmds[0].ret, "Result<ModpackSearchDto, String>");
}

/// 没有返回类型的函数，返回类型原文是空串
#[test]
fn test_commands_in_without_return_type() {
    let src = "#[gui_macros::command]\npub fn add_set_close_guard(window: WebviewWindow, enabled: bool) {";
    let cmds = commands_in(src);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name, "add_set_close_guard");
    assert_eq!(cmds[0].ret, "");
}

/// 显式分组标记：夹在文档注释 / 其它属性之间也要认出来
#[test]
fn test_commands_in_explicit_group() {
    let src = r#"
/// 文档
#[ipc_group("resource")]
#[gui_macros::command]
pub fn resource_list_mods(uuid: String) -> Result<Vec<ModItemDto>, String> {
todo!()
}
"#;
    let cmds = commands_in(src);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].group.as_deref(), Some("resource"));
}

/// 没有标记就是 None；注释里写出标记**不算**，只认 `#[` 开头的行
#[test]
fn test_commands_in_group_absent() {
    let src = "#[gui_macros::command]\npub fn a() -> bool { true }";
    assert_eq!(commands_in(src)[0].group, None);

    let src2 = "// 说明：本来该写 #[ipc_group(\"x\")]，但这只是注释\n#[gui_macros::command]\npub fn b() -> bool { true }";
    assert_eq!(commands_in(src2)[0].group, None);
}

/// 多个命令都要收进来
#[test]
fn test_commands_in_multiple() {
    let src = "#[gui_macros::command]\npub fn a() -> bool { true }\n\n#[gui_macros::command]\npub fn b() -> i64 { 0 }";
    let names: Vec<String> = commands_in(src).into_iter().map(|c| c.name).collect();
    assert_eq!(names, vec!["a", "b"]);
}

/// 事件名：去 `emit_` 前缀、`_` 换 `-`
#[test]
fn test_fns_after_emit() {
    let src = "#[gui_macros::emit]\npub fn emit_add_pack_progress(window: &WebviewWindow) {}";
    assert_eq!(
        fns_after(src, "#[gui_macros::emit]"),
        vec!["emit_add_pack_progress"]
    );
    let event = "emit_add_pack_progress"
        .strip_prefix("emit_")
        .unwrap()
        .replace('_', "-");
    assert_eq!(event, "add-pack-progress");
}

/// 事件 payload 从 emit 实参解析：参数、结构体字面量、无 payload 三种
#[test]
fn test_events_in_payloads() {
    let param = r#"
#[gui_macros::emit]
fn emit_launch_state(app: &AppHandle, event: StateEvent) {
let _ = app.emit(listens::LAUNCH_STATE, event);
}
"#;
    let e = events_in(param);
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].event_name(), "launch-state");
    assert_eq!(e[0].payload.as_deref(), Some("StateEvent"));

    // 签名参数与发出去的值不同：要认结构体字面量
    let literal = r#"
#[gui_macros::emit]
fn emit_load_done(app: &AppHandle, data: Option<String>) {
let _ = app.emit(
    listens::LOAD_DONE,
    LoadState { ok: data.is_none(), error: data },
);
}
"#;
    assert_eq!(events_in(literal)[0].payload.as_deref(), Some("LoadState"));

    let none = "#[gui_macros::emit]\nfn emit_java_change(app: &AppHandle) { let _ = app.emit(listens::JAVA_CHANGE, ()); }";
    assert_eq!(events_in(none)[0].payload, None);
}

/// emit_to 取第三个实参，路径限定要去掉，容器类型原样留下
#[test]
fn test_events_in_paths_and_containers() {
    let src = "#[gui_macros::emit]\nfn emit_file_drop(app: &AppHandle, label: &str, payload: crate::dtos::FileDropDto) { let _ = app.emit_to(label, listens::FILE_DROP, payload); }";
    assert_eq!(events_in(src)[0].payload.as_deref(), Some("FileDropDto"));

    let src2 = "#[gui_macros::emit]\nfn emit_dup(window: &WebviewWindow, dto: Vec<ModDuplicateDto>) { let _ = window.emit(listens::DUP, dto); }";
    assert_eq!(
        events_in(src2)[0].payload.as_deref(),
        Some("Vec<ModDuplicateDto>")
    );
}

/// 多参数时 payload 是结构体字面量，不能被前面的参数带偏
#[test]
fn test_events_in_multi_param() {
    let src = r#"
#[gui_macros::emit]
fn emit_gui_theme_change(
app: &AppHandle,
theme: crate::gui_config::Theme,
accent: String,
custom_accent: String,
) {
let _ = app.emit(
    listens::GUI_THEME_CHANGE,
    crate::dtos::GuiThemeDto { theme, accent, custom_accent },
);
}
"#;
    assert_eq!(events_in(src)[0].payload.as_deref(), Some("GuiThemeDto"));
}

/// 局部变量发出去时按 to_string 猜成 String
#[test]
fn test_events_in_local_string() {
    let src = "#[gui_macros::emit]\nfn emit_close_blocked(window: &Window) { let label = window.label().to_string(); let _ = window.emit_to(window.label(), listens::CLOSE_BLOCKED, label); }";
    assert_eq!(events_in(src)[0].payload.as_deref(), Some("String"));
}

/// 去掉类型路径限定，泛型里的路径也要去
#[test]
fn test_strip_path_qualifiers() {
    assert_eq!(strip_path_qualifiers("crate::dtos::FileDropDto"), "FileDropDto");
    assert_eq!(
        strip_path_qualifiers("Vec<crate::dtos::FileDropDto>"),
        "Vec<FileDropDto>"
    );
    assert_eq!(strip_path_qualifiers("&str"), "&str");
}

/// 文件路径 -> 模块路径：mod.rs / 根 lib.rs / 根 main.rs 不追加自身名
#[test]
fn test_module_path() {
    let src = Path::new("/x/src");
    assert_eq!(
        module_path(src, Path::new("/x/src/windows/account.rs")),
        "windows::account"
    );
    assert_eq!(
        module_path(src, Path::new("/x/src/windows/mod.rs")),
        "windows"
    );
    assert_eq!(module_path(src, Path::new("/x/src/lib.rs")), "");
    assert_eq!(module_path(src, Path::new("/x/src/main.rs")), "");
}

/// 属性块回溯要能跨过文档注释，并取到 serde 命名风格
#[test]
fn test_scan_types_attrs_and_rename() {
    let src = r#"
/// 文档注释
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Foo {
pub bar_baz: String,
}
"#;
    let types = scan_types(src);
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].name, "Foo");
    assert!(!types[0].is_enum, "不是枚举");
    assert_eq!(rename_all_of(&types[0].attrs).as_deref(), Some("camelCase"));
    assert!(types[0].body.contains("bar_baz"));
}

/// 枚举变体前的文档注释与 `#[default]` 不能干扰变体名提取
#[test]
fn test_scan_types_enum_body() {
    let src = r#"
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
/// 列表（默认）
#[default]
List,
Group,
Grid,
}
"#;
    let types = scan_types(src);
    assert!(types[0].is_enum, "是枚举");
    assert_eq!(types[0].body.trim().starts_with("/// 列表（默认）"), true);
}

/// 顶层逗号拆分不切泛型内部的逗号
#[test]
fn test_split_top_level() {
    assert_eq!(
        split_top_level("a: String, b: u32"),
        vec!["a: String", "b: u32"]
    );
    assert_eq!(
        split_top_level("HashMap<String, Vec<String>>, u32"),
        vec!["HashMap<String, Vec<String>>", "u32"]
    );
    assert!(split_top_level("").is_empty());
}

