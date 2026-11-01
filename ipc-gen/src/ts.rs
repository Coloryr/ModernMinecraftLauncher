//! Rust 类型 → TypeScript 类型映射，以及命名转换

use std::collections::BTreeMap;

use crate::scan::split_top_level;

/// Tauri 注入的参数：不进前端签名，按类型名识别，与参数位置无关
const INJECTED: [&str; 4] = ["AppHandle", "WebviewWindow", "Window", "State"];

/// 一个字段转成 TS 的结果
#[derive(Debug, Clone, PartialEq)]
pub struct TsField {
    /// TS 类型
    pub ty: String,
    /// 是否可缺省，对应 TS 的 `?`
    pub optional: bool,
}

/// 转成 TS 后的一个函数参数
#[derive(Debug, Clone, PartialEq)]
pub struct TsParam {
    /// TS 参数名，已转小驼峰，原始标识符前缀 `r#` 已去掉
    pub name: String,
    /// TS 类型
    pub ty: String,
    /// 是否可缺省，对应 TS 签名的 `?`
    pub optional: bool,
}

impl TsParam {
    /// 造一个参数，供测试与常量表用
    ///
    /// - `name`: TS 参数名
    /// - `ty`: TS 类型
    /// - `optional`: 是否可缺省
    pub fn new(name: &str, ty: &str, optional: bool) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
            optional,
        }
    }
}

/// snake_case / kebab-case -> SCREAMING_SNAKE_CASE，如 account-change -> ACCOUNT_CHANGE
///
/// - `name`: 事件名或标识符
///
/// # 返回值
///
/// 全大写、`-` 换成 `_` 的名字
pub fn screaming(name: &str) -> String {
    name.replace('-', "_").to_ascii_uppercase()
}

/// 取出 `Name<T>` 里的 `T`
///
/// - `ty`: 待解析的类型原文
/// - `name`: 要匹配的泛型名，如 `Option` / `Vec` / `Result`
///
/// # 返回值
///
/// 内层类型；不是该泛型或写法不规范时返回 `None`
fn generic_inner(ty: &str, name: &str) -> Option<String> {
    let t = ty.trim();
    let rest = t.strip_prefix(name)?;
    let rest = rest.strip_prefix('<')?;
    let rest = rest.strip_suffix('>')?;
    Some(rest.to_string())
}

/// Rust 类型 -> TS 类型
///
/// - `ty`: Rust 类型原文，可带 `&` / `'static` 引用
/// - `ext`: 外部 crate 的类型映射，本 crate 里定义不了的，如 `mml-names` 的 `Lang`；
///   不在表里的类型名按「本 crate 生成的类型」处理，直接用原名
///
/// # 返回值
///
/// 对应的 TS 类型；认不出的类型名原样返回
pub fn ts_type(ty: &str, ext: &BTreeMap<String, String>) -> String {
    let t = ty.trim();
    // 引用类型如 `&str`、`&'static str` 按值处理
    let t = t
        .trim_start_matches('&')
        .trim_start_matches("'static")
        .trim();

    if let Some(inner) = generic_inner(t, "Option") {
        return format!("{} | null", ts_type(&inner, ext));
    }
    if let Some(inner) = generic_inner(t, "Vec") {
        let inner = ts_type(&inner, ext);
        return if inner.contains('|') || inner.contains(' ') {
            format!("({inner})[]")
        } else {
            format!("{inner}[]")
        };
    }
    if let Some(inner) = generic_inner(t, "HashMap").or_else(|| generic_inner(t, "BTreeMap")) {
        let kv = split_top_level(&inner);
        if kv.len() == 2 {
            return format!("Record<{}, {}>", ts_type(&kv[0], ext), ts_type(&kv[1], ext));
        }
    }

    // 外部 crate 的类型：由调用方给映射
    if let Some(mapped) = ext.get(t) {
        return mapped.clone();
    }

    match t {
        "String" | "str" => "string".into(),
        "bool" => "boolean".into(),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" | "f32"
        | "f64" => "number".into(),
        // 空串 = Rust 侧写了 `fn f()`，即无返回类型，等价于 `()`
        "" | "()" => "void".into(),
        // 本 crate 生成的类型：直接用名字
        other => other.to_string(),
    }
}

/// 字段类型转成 TS 表示
///
/// - `ty`: 字段的 Rust 类型原文
/// - `input_only`: 只 derive `Deserialize` 的入参 DTO。serde 允许 `Option` 字段整键缺失，
///   所以 `Option<T>` 标成可缺省；输出 DTO 的键一定会被序列化出来，保持必填
/// - `ext`: 外部 crate 的类型映射，透传给 [`ts_type`]
///
/// # 返回值
///
/// 见 [`TsField`]；`Option<Option<T>>` 是三态补丁，不传 = 不改 / null = 清空 /
/// 有值 = 更新，可缺省且可为 null
pub fn ts_field(ty: &str, input_only: bool, ext: &BTreeMap<String, String>) -> TsField {
    if let Some(inner) = generic_inner(ty.trim(), "Option") {
        if let Some(inner2) = generic_inner(inner.trim(), "Option") {
            return TsField {
                ty: format!("{} | null", ts_type(&inner2, ext)),
                optional: true,
            };
        }
        return TsField {
            ty: format!("{} | null", ts_type(&inner, ext)),
            optional: input_only,
        };
    }
    TsField {
        ty: ts_type(ty, ext),
        optional: false,
    }
}

/// `ab_cd` / `ab-cd` -> `abCd`，缩写修正 Oauth -> OAuth，与现有命名保持一致
///
/// - `name`: snake_case 或 kebab-case 名字
///
/// # 返回值
///
/// 小驼峰名字
pub fn camel(name: &str) -> String {
    let mut parts = name.split(['_', '-']).filter(|s| !s.is_empty());
    let first = parts.next().unwrap_or("").to_string();
    let rest: String = parts
        .map(|s| {
            let mut c = s.chars();
            format!("{}{}", c.next().unwrap().to_ascii_uppercase(), c.as_str())
        })
        .collect();
    format!("{first}{rest}").replace("Oauth", "OAuth")
}

/// 参数原文转成 TS 参数表，并丢掉 Tauri 注入的参数
///
/// - `params`: 函数签名括号内的参数原文，可跨多行
/// - `ext`: 外部 crate 的类型映射，透传给 [`ts_field`]
///
/// # 返回值
///
/// 前端签名要用的参数表，每项见 [`TsParam`]；Tauri 注入的参数见 [`INJECTED`]，已被剔除
pub fn ts_params(params: &str, ext: &BTreeMap<String, String>) -> Vec<TsParam> {
    split_top_level(params)
        .into_iter()
        .filter_map(|p| {
            let (name, ty) = p.split_once(':')?;
            let name = name.trim().trim_start_matches("mut ").trim();
            let ty = ty.trim();
            // Tauri 注入参数不进前端签名
            if INJECTED.contains(&ty) {
                return None;
            }
            let field = ts_field(ty, false, ext);
            Some(TsParam {
                name: camel(name.trim_start_matches("r#")),
                ty: field.ty,
                optional: field.optional,
            })
        })
        .collect()
}

/// `Result<T, E>` -> `T`；其它原样，命令返回值都是 Promise，错误通道另算
///
/// - `ty`: 返回类型原文
///
/// # 返回值
///
/// `Ok` 侧的类型；不是 `Result` 就原样返回
pub fn result_ok(ty: &str) -> String {
    if let Some(inner) = generic_inner(ty, "Result") {
        let parts = split_top_level(&inner);
        if let Some(first) = parts.first() {
            return first.clone();
        }
    }
    ty.to_string()
}

/// snake_case / kebab-case -> PascalCase，缩写词修正 Oauth -> OAuth
///
/// - `name`: snake_case 或 kebab-case 名字
///
/// # 返回值
///
/// 大驼峰名字，事件名常量等用
pub fn pascal(name: &str) -> String {
    name.split(['_', '-'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            format!(
                "{}{}",
                chars.next().unwrap().to_ascii_uppercase(),
                chars.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("")
        .replace("Oauth", "OAuth")
}

#[cfg(test)]
#[path = "../tests/unit/ts_tests.rs"]
mod tests;
