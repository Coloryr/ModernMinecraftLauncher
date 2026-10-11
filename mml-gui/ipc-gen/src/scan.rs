//! 源码扫描与语法解析
//!
//! 全部是纯函数：输入源码文本，输出结构化数据，不碰文件系统与环境变量
//! `walk_rs` 除外，它只负责遍历目录。

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::Decl;

/// 递归收集目录下的 .rs 文件
///
/// - `dir`: 起始目录，读不了就当作空目录，不报错
/// - `out`: 收集结果，追加写入，调用方自己先清空
pub fn walk_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// 文件路径 -> Rust 模块路径，如 src/windows/account.rs -> "windows::account"；
/// mod.rs / lib.rs / main.rs -> 所在目录即模块
///
/// - `src`: 源码根目录
/// - `file`: 要换算的 .rs 文件
///
/// # 返回值
///
/// `::` 分隔的模块路径；文件不在 `src` 下时返回空串
pub fn module_path(src: &Path, file: &Path) -> String {
    let rel = match file.strip_prefix(src) {
        Ok(rel) => rel,
        Err(_) => return String::new(),
    };
    let mut comps: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    comps.pop();
    let file_name = rel
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let stem = file_name.trim_end_matches(".rs");
    // mod.rs / src 根下的 lib.rs、main.rs 不追加自身名，其余文件名即模块名
    let is_special = stem == "mod" || ((stem == "lib" || stem == "main") && comps.is_empty());
    if !is_special {
        comps.push(stem.to_string());
    }
    comps.join("::")
}

/// 找 marker 之后的第一个函数名，跳过中间的文档 / 属性，标准格式下足够可靠
///
/// - `text`: 源码全文
/// - `marker`: 标记字面量，即命令属性或事件属性
///
/// # 返回值
///
/// 每个 marker 之后的第一个函数名，按出现顺序
pub fn fns_after(text: &str, marker: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        if let Some(f) = after.find("fn ") {
            let ident: String = after[f + 3..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !ident.is_empty() {
                names.push(ident);
            }
        }
        rest = after;
    }
    names
}

/// 从 `from` 起，最早的 `pub struct` / `pub enum` 声明起始位置
///
/// - `text`: 源码全文
/// - `from`: 起始字节位置
///
/// # 返回值
///
/// 声明起始位置；后面没有声明时返回 `None`
fn next_decl(text: &str, from: usize) -> Option<usize> {
    let a = text[from..].find("\npub struct ").map(|p| p + from + 1);
    let b = text[from..].find("\npub enum ").map(|p| p + from + 1);
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

/// 扫描 `pub struct` / `pub enum` 声明
///
/// - `text`: 源码全文
///
/// # 返回值
///
/// 每个声明一项，字段含义见 [`Decl`]
pub fn scan_types(text: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(pos) = next_decl(text, search) {
        let is_enum = text[pos..].starts_with("pub enum ");
        let head = if is_enum { "pub enum " } else { "pub struct " };
        let name_start = pos + head.len();
        let name: String = text[name_start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            search = name_start;
            continue;
        }
        // 属性块：从声明往回逐行收集 `#[...]` / `///`，遇到空行或 `}` 等其它内容即停
        let mut attr_lines: Vec<&str> = Vec::new();
        for line in text[..pos].lines().rev() {
            let t = line.trim();
            if t.starts_with("#[") || t.starts_with("///") {
                attr_lines.push(line);
            } else {
                break;
            }
        }
        attr_lines.reverse();
        let attrs = attr_lines.join("\n");

        // 正文：花括号配平
        let Some(open) = text[name_start..].find('{') else {
            search = name_start;
            continue;
        };
        let body_start = name_start + open + 1;
        let mut depth = 1usize;
        let mut i = body_start;
        let bytes = text.as_bytes();
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        out.push(Decl {
            name,
            attrs,
            body: text[body_start..i.saturating_sub(1)].to_string(),
            is_enum,
        });
        search = i;
    }
    out
}

/// 从属性块里取 serde 命名风格的值，如 camelCase
///
/// - `attrs`: [`scan_types`] 返回的属性原文
///
/// # 返回值
///
/// 命名的风格名，如 `camelCase`；没有该属性时返回 `None`
pub fn rename_all_of(attrs: &str) -> Option<String> {
    let p = attrs.find("rename_all")?;
    let rest = &attrs[p..];
    let q1 = rest.find('"')? + 1;
    let q2 = rest[q1..].find('"')? + q1;
    Some(rest[q1..q2].to_string())
}

/// 拆顶层逗号，不切泛型里的逗号
///
/// - `s`: 待拆分的原文，如参数表、泛型参数表
///
/// # 返回值
///
/// 各段，已 trim，空段丢弃
pub fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '<' | '(' | '[' => {
                depth += 1;
                cur.push(c);
            }
            '>' | ')' | ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// 取命令属性块里的显式分组标记，标记的值就是原始组名
///
/// **为什么需要它**：`bindings.ts` 的命令组键默认取"来源 .rs 模块最后一段"，于是把命令
/// 拆进子模块 `windows::resource::mods` 会让组键从 `resource` 变成 `mods`，前端所有
/// `commands.resource.*` 调用点一起断。加上这个标记，命令可以按分类拆文件、组键不变。
///
/// 与 `commands_in` 一样是**裸文本**扫描：从命令属性往上逐行看，允许中间夹着别的属性 /
/// 文档注释 / 空行，遇到别的代码就停；只有 `#[` 开头的行才算，注释里写出来无效。
///
/// - `head`: 命令属性**之前**的那段源码
///
/// # 返回值
///
/// 分组名；没写标记时返回 `None`，由调用方按来源模块分组
fn group_of(head: &str) -> Option<String> {
    const KEY: &str = "ipc_group(";
    for line in head.lines().rev() {
        let t = line.trim();
        if t.starts_with("#[") {
            if let Some(p) = t.find(KEY) {
                let after = &t[p + KEY.len()..];
                let q1 = after.find('"')? + 1;
                let q2 = after[q1..].find('"')? + q1;
                let name = after[q1..q2].trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            continue;
        }
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        break;
    }
    None
}

/// 扫描到的一条命令函数
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedCommand {
    /// 命令名，即 Rust 函数名
    pub name: String,
    /// 参数原文，可跨多行；Tauri 注入的参数仍留在里面，由 [`crate::ts::ts_params`] 过滤
    pub params: String,
    /// 返回类型原文；没写返回类型时是空串
    pub ret: String,
    /// 显式分组标记的值；`None` 表示按来源模块分组
    pub group: Option<String>,
}

/// 取命令属性后面的函数
///
/// 参数/返回按括号与尖括号配平截取，能处理跨多行的签名。
/// 显式分组来自属性块里的分组标记，没有则为 `None`，按来源模块分组。
///
/// - `text`: 源码全文
///
/// # 返回值
///
/// 每个命令一项，各字段含义见 [`ScannedCommand`]
pub fn commands_in(text: &str) -> Vec<ScannedCommand> {
    const MARK: &str = "#[tauri::command]";
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(MARK) {
        // 分组标记写在命令属性**之前**，所以在 MARK 之前那段文本里找
        let group = group_of(&rest[..pos]);
        let after = &rest[pos + MARK.len()..];
        let Some(fpos) = after.find("fn ") else {
            rest = after;
            continue;
        };
        let after_fn = &after[fpos + 3..];
        let name: String = after_fn
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let Some(lp) = after_fn.find('(') else {
            rest = after_fn;
            continue;
        };

        // 参数：括号配平
        let mut depth = 0i32;
        let mut end = None;
        for (i, c) in after_fn[lp..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(lp + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(rp) = end else {
            rest = after_fn;
            continue;
        };
        let params = after_fn[lp + 1..rp].to_string();

        // 返回类型：若紧跟 `->`，取到函数体的 `{`，尖括号与圆括号配平
        let tail = &after_fn[rp + 1..];
        let ret = match tail.find("->") {
            Some(a) if tail[..a].trim().is_empty() => {
                let ty_start = a + 2;
                let mut d = 0i32;
                let mut cut = tail.len();
                for (i, c) in tail[ty_start..].char_indices() {
                    match c {
                        '<' | '(' | '[' => d += 1,
                        '>' | ')' | ']' => d -= 1,
                        '{' if d == 0 => {
                            cut = ty_start + i;
                            break;
                        }
                        _ => {}
                    }
                }
                tail[ty_start..cut].trim().to_string()
            }
            _ => String::new(),
        };

        out.push(ScannedCommand {
            name,
            params,
            ret,
            group,
        });
        rest = tail;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 命令签名跨多行也能解析，注入参数留在参数原文里，由 ts.rs 过滤
    #[test]
    fn test_commands_in_multiline() {
        let src = r#"
/// 文档注释
#[tauri::command]
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
        let src =
            "#[tauri::command]\npub fn add_set_close_guard(window: WebviewWindow, enabled: bool) {";
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
#[tauri::command]
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
        let src = "#[tauri::command]\npub fn a() -> bool { true }";
        assert_eq!(commands_in(src)[0].group, None);

        let src2 = "// 说明：本来该写 #[ipc_group(\"x\")]，但这只是注释\n#[tauri::command]\npub fn b() -> bool { true }";
        assert_eq!(commands_in(src2)[0].group, None);
    }

    /// 多个命令都要收进来
    #[test]
    fn test_commands_in_multiple() {
        let src = "#[tauri::command]\npub fn a() -> bool { true }\n\n#[tauri::command]\npub fn b() -> i64 { 0 }";
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
}
