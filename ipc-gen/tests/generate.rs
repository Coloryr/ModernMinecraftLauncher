//! 端到端：临时目录里摆一个小型 crate，跑 generate() 断言产物

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use gui_ipc_gen::{GenConfig, Generated, generate};

/// 建一个空的临时 src 目录，同名目录先清掉
///
/// - name: 用例特征名，拼进目录名；各用例各用各的目录，避免互相踩
///
/// # 返回值
///
/// 建好的目录，落在 target/temp
fn temp_src(name: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!("gui_ipc_gen_it_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 在 src 下写文件，自动建父目录
fn write(src: &Path, rel: &str, text: &str) {
    let path = src.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// 造一份生成配置：源码根为 src，外部类型只映射 Lang
fn config(src: &Path) -> GenConfig {
    GenConfig {
        src_dir: src.to_path_buf(),
        external_types: BTreeMap::from([(
            "Lang".to_string(),
            "\"zh_cn\" | \"en_us\"".to_string(),
        )]),
        ..Default::default()
    }
}

/// 按路径取一个生成的类型文件；没有就 panic
fn type_file<'a>(out: &'a Generated, rel: &str) -> &'a str {
    out.type_files
        .iter()
        .find(|f| f.rel_path == rel)
        .map(|f| f.content.as_str())
        .unwrap_or_else(|| panic!("没有生成 {rel}"))
}

/// 一整套：命令分组、事件常量与函数、类型分文件、注册宏
#[test]
fn test_generate_full() {
    let src = temp_src("full");

    write(
        &src,
        "windows/add_modpack.rs",
        r#"
use tauri::AppHandle;

/// 搜索整合包
#[gui_macros::command]
pub async fn add_modpack_search(
    app: AppHandle,
    source: String,
    page: Option<u32>,
) -> Result<ModpackSearchDto, String> {
    todo!()
}

#[gui_macros::command]
pub fn add_modpack_start(app: AppHandle, project_id: String) -> Result<(), String> {
    todo!()
}
"#,
    );

    write(
        &src,
        "windows/collect.rs",
        r#"
#[gui_macros::command]
pub fn collect_load(app: AppHandle) {}

#[gui_macros::emit]
pub fn emit_add_pack_progress(app: &tauri::AppHandle, dto: PackProgressDto) {
    let _ = app.emit(listens::ADD_PACK_PROGRESS, dto);
}
"#,
    );

    write(
        &src,
        "dtos/add_dto.rs",
        r#"
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackSearchDto {
    pub project_id: String,
    pub icon_url: Option<String>,
}

/// 入参 DTO：只 derive Deserialize，字段可缺省
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackSearchInput {
    pub source: String,
    pub keyword: Option<String>,
}
"#,
    );

    write(
        &src,
        "dtos/pack_dto.rs",
        r#"
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackProgressDto {
    pub now: u32,
    pub total: u32,
}
"#,
    );

    write(
        &src,
        "gui_config.rs",
        r#"
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadType {
    /// 默认：按皮肤加载
    #[default]
    Auto,
    Head,
}

/// 非 IPC 类型（没 derive serde），不该出现在产物里
pub enum Internal {
    A,
}
"#,
    );

    let out = generate(&config(&src)).unwrap();
    let b = &out.bindings_ts;

    // 命令：按 .rs 分组，剥掉公共前段；类型不在 bindings.ts 里
    assert!(b.contains("    addModpack: {\n"), "{b}");
    assert!(
        b.contains("      search: (source: string, page: number | null) => invoke<ModpackSearchDto>(\"add_modpack_search\", { source, page }),\n"),
        "{b}"
    );
    assert!(
        b.contains("      start: (projectId: string) => invoke<void>(\"add_modpack_start\", { projectId }),\n"),
        "{b}"
    );
    assert!(b.contains("export * from \"./types\";\n"), "{b}");
    assert!(!b.contains("export type "), "{b}");

    // 类型：按来源 .rs 拆文件，camelCase 与 Option 规则生效
    let add = type_file(&out, "types/dtos/add_dto.ts");
    assert!(
        add.contains("export type ModpackSearchDto = {\n  projectId: string,\n  iconUrl: string | null,\n};\n\n"),
        "{add}"
    );
    assert!(
        add.contains("export type ModpackSearchInput = {\n  source: string,\n  keyword?: string | null,\n};\n\n"),
        "{add}"
    );
    let config_ts = type_file(&out, "types/gui_config.ts");
    assert!(
        config_ts.contains("export type HeadType = \"auto\" | \"head\";\n\n"),
        "{config_ts}"
    );
    assert!(!config_ts.contains("Internal"), "{config_ts}");

    // barrel 把所有类型文件再导出
    let index = type_file(&out, "types/index.ts");
    assert!(index.contains("export * from \"./dtos/add_dto\";\n"), "{index}");
    assert!(index.contains("export * from \"./gui_config\";\n"), "{index}");

    // 事件：常量 + 订阅函数；payload 类型进 EventPayloads
    let l = &out.listens_ts;
    assert!(l.contains("export const AddPackProgress = \"add-pack-progress\"\n"), "{l}");
    assert!(l.contains("import type { PackProgressDto } from \"./bindings\";\n"), "{l}");
    assert!(l.contains("  [AddPackProgress]: PackProgressDto;\n"), "{l}");
    assert!(
        l.contains("    onAddPackProgress: (cb: (payload: PackProgressDto) => void) => on(AddPackProgress, cb),\n"),
        "{l}"
    );

    // 注册宏：模块路径完整
    let rs = &out.invokes_gen_rs;
    assert!(rs.contains("            windows::add_modpack::add_modpack_search,\n"), "{rs}");
    assert!(rs.contains("            windows::collect::collect_load,\n"), "{rs}");
    assert!(rs.contains("    pub const ADD_PACK_PROGRESS: &str = \"add-pack-progress\";\n"), "{rs}");

    let _ = fs::remove_dir_all(&src);
}

/// 外部 crate 类型 Lang 走映射表，不留裸类型名
#[test]
fn test_generate_external_type() {
    let src = temp_src("ext");
    write(
        &src,
        "dtos/gui_config_dto.rs",
        r#"
use mml_names::Lang;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiConfigDto {
    pub lang: Lang,
}
"#,
    );

    let out = generate(&config(&src)).unwrap();
    assert!(
        type_file(&out, "types/dtos/gui_config_dto.ts").contains("  lang: \"zh_cn\" | \"en_us\",\n"),
        "{}",
        type_file(&out, "types/dtos/gui_config_dto.ts")
    );
    let _ = fs::remove_dir_all(&src);
}

/// 同一份源码生成两次结果必须完全一致，排序稳定
#[test]
fn test_generate_is_deterministic() {
    let src = temp_src("det");
    for (i, name) in ["bbb", "aaa", "ccc"].iter().enumerate() {
        write(
            &src,
            &format!("windows/{name}.rs"),
            &format!("#[gui_macros::command]\npub fn {name}_do_{i}(app: tauri::AppHandle) -> bool {{ true }}\n"),
        );
    }

    let cfg = config(&src);
    let a = generate(&cfg).unwrap();
    let b = generate(&cfg).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.type_files, b.type_files);

    // 完整路径按 (模块, 函数名) 排序，aaa 在前
    let ia = a.invokes_gen_rs.find("windows::aaa").unwrap();
    let ic = a.invokes_gen_rs.find("windows::ccc").unwrap();
    assert!(ia < ic, "{}", a.invokes_gen_rs);

    let _ = fs::remove_dir_all(&src);
}
