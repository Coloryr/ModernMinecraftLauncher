use super::*;
use crate::{ts::TsParam, Decl, GeneratedFile, Sig};

/// 造一条命令签名，无显式分组
fn sig(module: &str, name: &str, params: Vec<TsParam>, ret: &str) -> Sig {
    Sig {
        module: module.into(),
        name: name.into(),
        group: None,
        params,
        ret: ret.into(),
    }
}

/// 带显式分组的签名，模拟拆到子模块里的命令
fn sig_group(module: &str, name: &str, group: &str) -> Sig {
    Sig {
        group: Some(group.into()),
        ..sig(module, name, vec![], "void")
    }
}

/// 造一条类型声明
fn decl(name: &str, attrs: &str, body: &str, is_enum: bool) -> Decl {
    Decl {
        name: name.into(),
        attrs: attrs.into(),
        body: body.into(),
        is_enum,
    }
}

/// 把生成文件列表摊成 路径 -> 内容
fn type_map(files: &[GeneratedFile]) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|f| (f.rel_path.clone(), f.content.clone()))
        .collect()
}

/// 造一个事件，payload 是 Rust 类型原文
fn event(emit_name: &str, payload: Option<&str>) -> ScannedEvent {
    ScannedEvent {
        emit_name: emit_name.into(),
        payload: payload.map(|s| s.to_string()),
    }
}

/// 组键取模块最后一段，方法名剥掉组内公共前段
#[test]
fn test_group_and_prefix_strip() {
    let sigs = vec![
        sig(
            "windows::add_modpack",
            "add_modpack_search",
            vec![TsParam::new("source", "string", false)],
            "ModpackSearchDto",
        ),
        sig("windows::add_modpack", "add_modpack_start", vec![], "void"),
    ];
    let out = bindings_ts(&sigs, &BTreeSet::new());
    assert!(out.contains("    addModpack: {\n"), "{out}");
    assert!(
        out.contains("export function createCommands(invoke: Invoke) {\n  return {\n"),
        "{out}"
    );
    assert!(out.contains("      search: (source: string) => invoke<ModpackSearchDto>(\"add_modpack_search\", { source }),\n"), "{out}");
    // 无参数的命令不带对象实参
    assert!(
        out.contains("      start: () => invoke<void>(\"add_modpack_start\"),\n"),
        "{out}"
    );
    assert!(out.contains("export * from \"./types\";\n"), "{out}");
}

/// 显式分组：子模块的命令并回原来的组，组键与组内公共前段剥离都不受影响
#[test]
fn test_explicit_group_merges_submodule() {
    let sigs = vec![
        sig("windows::resource", "resource_list_saves", vec![], "void"),
        sig_group(
            "windows::resource::screenshots",
            "resource_delete_screenshot",
            "resource",
        ),
        sig_group(
            "windows::resource::screenshots",
            "resource_list_screenshots",
            "resource",
        ),
    ];
    let out = bindings_ts(&sigs, &BTreeSet::new());
    assert_eq!(out.matches("    resource: {\n").count(), 1, "{out}");
    let body = out.split("    resource: {").nth(1).unwrap();
    let order: Vec<&str> = [
        "clearScreenshots",
        "deleteScreenshot",
        "listSaves",
        "listScreenshots",
    ]
    .into_iter()
    .filter(|m| body.contains(&format!("      {m}:")))
    .collect();
    assert_eq!(
        order,
        vec!["deleteScreenshot", "listSaves", "listScreenshots"],
        "{out}"
    );
}

/// 单命令组：公共前缀就是整个名字，必须回退一段，否则方法名为空
#[test]
fn test_single_command_group() {
    let sigs = vec![sig("windows::collect", "collect_load", vec![], "void")];
    let out = bindings_ts(&sigs, &BTreeSet::new());
    assert!(
        out.contains("      load: () => invoke<void>(\"collect_load\"),\n"),
        "{out}"
    );
}

/// 可缺省参数带 ?
#[test]
fn test_optional_param() {
    let sigs = vec![sig(
        "windows::main",
        "main_open",
        vec![TsParam::new("page", "number | null", true)],
        "void",
    )];
    let out = bindings_ts(&sigs, &BTreeSet::new());
    assert!(
        out.contains(
            "      open: (page?: number | null) => invoke<void>(\"main_open\", { page }),\n"
        ),
        "{out}"
    );
}

/// 类型按来源文件拆开，跨文件引用自动补 import，barrel 全量再导出
#[test]
fn test_type_files_split_and_imports() {
    let groups = BTreeMap::from([
        (
            "dtos/a_dto.ts".to_string(),
            vec![decl(
                "ADto",
                "#[derive(Serialize)]",
                "\n    pub b: BDto,\n",
                false,
            )],
        ),
        (
            "dtos/b_dto.ts".to_string(),
            vec![decl(
                "BDto",
                "#[derive(Serialize)]",
                "\n    pub x: String,\n",
                false,
            )],
        ),
    ]);
    let files = type_files(&groups, &BTreeMap::new());
    let map = type_map(&files);

    let a = &map["types/dtos/a_dto.ts"];
    assert!(a.contains("import type { BDto } from \"./b_dto\";\n"), "{a}");
    assert!(a.contains("export type ADto = {\n  b: BDto,\n};\n\n"), "{a}");
    let b = &map["types/dtos/b_dto.ts"];
    assert!(!b.contains("import type"), "{b}");

    let index = &map["types/index.ts"];
    assert!(index.contains("export * from \"./dtos/a_dto\";\n"), "{index}");
    assert!(index.contains("export * from \"./dtos/b_dto\";\n"), "{index}");
}

/// 来自 src 根目录的类型文件相对引用上层目录
#[test]
fn test_type_files_parent_import() {
    let groups = BTreeMap::from([
        (
            "gui_config.ts".to_string(),
            vec![decl(
                "GuiConfigDto",
                "#[derive(Serialize)]",
                "\n    pub theme: Theme,\n",
                false,
            )],
        ),
        (
            "dtos/user_dto.ts".to_string(),
            vec![decl(
                "UserDto",
                "#[derive(Serialize)]",
                "\n    pub cfg: GuiConfigDto,\n",
                false,
            )],
        ),
    ]);
    let map = type_map(&type_files(&groups, &BTreeMap::new()));
    assert!(
        map["types/dtos/user_dto.ts"].contains("from \"../gui_config\""),
        "{}",
        map["types/dtos/user_dto.ts"]
    );
}

/// 枚举：serde 命名风格生效，文档注释与 #[default] 不干扰变体名
#[test]
fn test_enum_decl() {
    let groups = BTreeMap::from([(
        "gui_config.ts".to_string(),
        vec![decl(
            "HeadType",
            "#[derive(Serialize)]\n#[serde(rename_all = \"lowercase\")]",
            "\n    /// 默认\n    #[default]\n    Auto,\n    Head,\n",
            true,
        )],
    )]);
    let map = type_map(&type_files(&groups, &BTreeMap::new()));
    assert!(
        map["types/gui_config.ts"].contains("export type HeadType = \"auto\" | \"head\";\n\n"),
        "{}",
        map["types/gui_config.ts"]
    );
}

/// 枚举的 camelCase 只把变体首字母小写，与 serde 的变体规则一致
#[test]
fn test_enum_camel_case_variants() {
    let groups = BTreeMap::from([(
        "dtos/theme_dto.ts".to_string(),
        vec![decl(
            "ThemeKindDto",
            "#[derive(Serialize)]\n#[serde(rename_all = \"camelCase\")]",
            "\n    Home,\n    Css,\n    Page,\n",
            true,
        )],
    )]);
    let map = type_map(&type_files(&groups, &BTreeMap::new()));
    assert!(
        map["types/dtos/theme_dto.ts"]
            .contains("export type ThemeKindDto = \"home\" | \"css\" | \"page\";\n\n"),
        "{}",
        map["types/dtos/theme_dto.ts"]
    );
}

/// 无 serde 命名风格的 struct 键名保持 snake；camelCase 的转驼峰
#[test]
fn test_struct_rename_all() {
    let groups = BTreeMap::from([
        (
            "dtos/dir_dto.ts".to_string(),
            vec![decl(
                "DirEntry",
                "#[derive(Serialize)]",
                "\n    pub name: String,\n    pub is_dir: bool,\n",
                false,
            )],
        ),
        (
            "dtos/oauth_dto.ts".to_string(),
            vec![decl(
                "AccountOAuthStateDto",
                "#[derive(Serialize)]\n#[serde(rename_all = \"camelCase\")]",
                "\n    pub state: String,\n    pub message: Option<String>,\n",
                false,
            )],
        ),
    ]);
    let map = type_map(&type_files(&groups, &BTreeMap::new()));
    assert!(
        map["types/dtos/dir_dto.ts"].contains("  is_dir: boolean,\n"),
        "{}",
        map["types/dtos/dir_dto.ts"]
    );
    assert!(
        map["types/dtos/oauth_dto.ts"].contains("  state: string,\n  message: string | null,\n"),
        "{}",
        map["types/dtos/oauth_dto.ts"]
    );
}

/// 只 derive Deserialize 的入参 DTO：Option 字段可缺省，非 Option 字段仍必填
#[test]
fn test_input_only_dto() {
    let groups = BTreeMap::from([(
        "dtos/set_dto.ts".to_string(),
        vec![decl(
            "SetDto",
            "#[derive(Deserialize)]",
            "\n    pub name: String,\n    pub count: Option<u32>,\n",
            false,
        )],
    )]);
    let map = type_map(&type_files(&groups, &BTreeMap::new()));
    assert!(
        map["types/dtos/set_dto.ts"].contains("  name: string,\n  count?: number | null,\n"),
        "{}",
        map["types/dtos/set_dto.ts"]
    );
}

/// 外部 crate 类型走映射表
#[test]
fn test_external_type_in_struct() {
    let groups = BTreeMap::from([(
        "gui_config.ts".to_string(),
        vec![decl(
            "GuiConfigDto",
            "#[derive(Serialize)]\n#[serde(rename_all = \"camelCase\")]",
            "\n    pub lang: Lang,\n",
            false,
        )],
    )]);
    let ext = BTreeMap::from([("Lang".to_string(), "\"zh_cn\" | \"en_us\"".to_string())]);
    let map = type_map(&type_files(&groups, &ext));
    assert!(
        map["types/gui_config.ts"].contains("  lang: \"zh_cn\" | \"en_us\",\n"),
        "{}",
        map["types/gui_config.ts"]
    );
}

/// listens.ts：名字常量之外，每个事件生成一个带 payload 类型的 onXxx
#[test]
fn test_listens_events_functions() {
    let events = vec![
        event("emit_account_change", None),
        event("emit_account_oauth", Some("AccountOAuthDto")),
    ];
    let declared = BTreeSet::from(["AccountOAuthDto".to_string()]);
    let out = listens_ts(&events, &BTreeMap::new(), &declared);

    assert!(
        out.contains("import type { AccountOAuthDto } from \"./bindings\";\n"),
        "{out}"
    );
    assert!(out.contains("export const AccountChange = \"account-change\"\n"), "{out}");
    assert!(out.contains("  [AccountChange]: void;\n"), "{out}");
    assert!(out.contains("  [AccountOAuth]: AccountOAuthDto;\n"), "{out}");
    assert!(
        out.contains("    onAccountChange: (cb: () => void) => on(AccountChange, cb),\n"),
        "{out}"
    );
    assert!(
        out.contains("    onAccountOAuth: (cb: (payload: AccountOAuthDto) => void) => on(AccountOAuth, cb),\n"),
        "{out}"
    );
}

/// 事件 payload 是容器 / Option 时按 TS 类型展开
#[test]
fn test_listens_payload_containers() {
    let events = vec![
        event("emit_resource_mods_duplicate", Some("Vec<ModDuplicateDto>")),
        event("emit_load_done", Some("LoadState")),
    ];
    let declared = BTreeSet::from(["ModDuplicateDto".to_string(), "LoadState".to_string()]);
    let out = listens_ts(&events, &BTreeMap::new(), &declared);
    assert!(
        out.contains("    onResourceModsDuplicate: (cb: (payload: ModDuplicateDto[]) => void) => on(ResourceModsDuplicate, cb),\n"),
        "{out}"
    );
    assert!(
        out.contains("    onLoadDone: (cb: (payload: LoadState) => void) => on(LoadDone, cb),\n"),
        "{out}"
    );
}

