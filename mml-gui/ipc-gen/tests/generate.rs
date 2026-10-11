//! 端到端：临时目录里摆一个小型 crate，跑 `generate()` 断言三份产物

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use gui_ipc_gen::{GenConfig, generate};

/// 建一个空的临时 src 目录，同名目录先清掉
///
/// - `name`: 用例特征名，拼进目录名；各用例各用各的目录，避免互相踩
///
/// # 返回值
///
/// 建好的目录，落在 `target/temp`
fn temp_src(name: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!("gui_ipc_gen_it_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 在 src 下写文件，自动建父目录
///
/// - `src`: 临时 src 根目录
/// - `rel`: 相对路径，如 `windows/add_modpack.rs`
/// - `text`: 文件内容
fn write(src: &Path, rel: &str, text: &str) {
    let path = src.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// 造一份生成配置：源码根为 `src`，外部类型只映射 `Lang`
///
/// - `src`: 临时 src 根目录
///
/// # 返回值
///
/// 测试用配置，其余字段取默认值
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

/// 一整套：命令分组、事件常量、类型、注册宏
#[test]
fn test_generate_full() {
    let src = temp_src("full");

    write(
        &src,
        "windows/add_modpack.rs",
        r#"
use tauri::AppHandle;

/// 搜索整合包
#[tauri::command]
pub async fn add_modpack_search(
    app: AppHandle,
    source: String,
    page: Option<u32>,
) -> Result<ModpackSearchDto, String> {
    todo!()
}

#[tauri::command]
pub fn add_modpack_start(app: AppHandle, project_id: String) -> Result<(), String> {
    todo!()
}
"#,
    );

    write(
        &src,
        "windows/collect.rs",
        r#"
#[tauri::command]
pub fn collect_load(app: AppHandle) {}

#[gui_macros::emit]
pub fn emit_add_pack_progress(window: &tauri::WebviewWindow) {}
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

    // 命令：按 .rs 分组，剥掉公共前段
    assert!(b.contains("  addModpack: {\n"), "{b}");
    // 命令参数的 Option<T> 是 `T | null` 且不可缺省，前端要显式传 null
    assert!(
        b.contains("    search: (source: string, page: number | null) => invoke<ModpackSearchDto>(\"add_modpack_search\", { source, page }),\n"),
        "{b}"
    );
    assert!(
        b.contains("    start: (projectId: string) => invoke<void>(\"add_modpack_start\", { projectId }),\n"),
        "{b}"
    );
    // 单命令组：公共前缀回退一段
    assert!(
        b.contains("  collect: {\n    load: () => invoke<void>(\"collect_load\"),\n  },\n"),
        "{b}"
    );

    // 类型：camelCase 生效，Option 字段必填但可 null
    assert!(
        b.contains("export type ModpackSearchDto = {\n  projectId: string,\n  iconUrl: string | null,\n};\n\n"),
        "{b}"
    );
    // 入参 DTO：`Option` 字段可缺省，非 Option 字段仍必填
    assert!(
        b.contains("export type ModpackSearchInput = {\n  source: string,\n  keyword?: string | null,\n};\n\n"),
        "{b}"
    );
    // 枚举：serde 命名风格生效，文档注释与 #[default] 不干扰；非 serde 类型不收
    assert!(b.contains("export type HeadType = \"auto\" | \"head\";\n\n"), "{b}");
    assert!(!b.contains("Internal"), "{b}");

    // 事件常量
    assert!(
        out.listens_ts
            .contains("export const AddPackProgress: string = \"add-pack-progress\"\n"),
        "{}",
        out.listens_ts
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
        out.bindings_ts.contains("  lang: \"zh_cn\" | \"en_us\",\n"),
        "{}",
        out.bindings_ts
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
            &format!("#[tauri::command]\npub fn {name}_do_{i}(app: tauri::AppHandle) -> bool {{ true }}\n"),
        );
    }

    let cfg = config(&src);
    let a = generate(&cfg).unwrap();
    let b = generate(&cfg).unwrap();
    assert_eq!(a.bindings_ts, b.bindings_ts);
    assert_eq!(a.invokes_gen_rs, b.invokes_gen_rs);

    // 完整路径按 (模块, 函数名) 排序，aaa 在前
    let ia = a.invokes_gen_rs.find("windows::aaa").unwrap();
    let ic = a.invokes_gen_rs.find("windows::ccc").unwrap();
    assert!(ia < ic, "{}", a.invokes_gen_rs);

    let _ = fs::remove_dir_all(&src);
}
