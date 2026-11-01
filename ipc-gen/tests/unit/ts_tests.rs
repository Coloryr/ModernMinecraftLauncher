use super::*;

/// 测试用的外部类型映射，只有 `Lang` 一条
///
/// # 返回值
///
/// 只含 `Lang` 的映射表
fn ext() -> BTreeMap<String, String> {
    BTreeMap::from([("Lang".to_string(), "\"zh_cn\" | \"en_us\"".to_string())])
}

/// 造一个字段的期望值
///
/// - `ty`: TS 类型
/// - `optional`: 是否可缺省
fn f(ty: &str, optional: bool) -> TsField {
    TsField {
        ty: ty.into(),
        optional,
    }
}

/// 造一个参数的期望值
///
/// - `name`: TS 参数名
/// - `ty`: TS 类型
/// - `optional`: 是否可缺省
fn p(name: &str, ty: &str, optional: bool) -> TsParam {
    TsParam::new(name, ty, optional)
}

/// 基础类型与容器映射
#[test]
fn test_ts_type_basics() {
    let e = ext();
    assert_eq!(ts_type("String", &e), "string");
    assert_eq!(ts_type("bool", &e), "boolean");
    assert_eq!(ts_type("i32", &e), "number");
    assert_eq!(ts_type("usize", &e), "number");
    assert_eq!(ts_type("Option<String>", &e), "string | null");
    assert_eq!(ts_type("Vec<String>", &e), "string[]");
    assert_eq!(
        ts_type("HashMap<String, Vec<String>>", &e),
        "Record<string, string[]>"
    );
    // 自定义类型原样返回，它由本 crate 生成
    assert_eq!(ts_type("NewsItem", &e), "NewsItem");
}

/// 引用类型按值处理，`Vec<&'static str>` 是本仓真实用到的写法
#[test]
fn test_ts_type_references() {
    let e = ext();
    assert_eq!(ts_type("&str", &e), "string");
    assert_eq!(ts_type("Vec<&'static str>", &e), "string[]");
}

/// 没有返回类型 / 单元类型都是 void
#[test]
fn test_ts_type_void() {
    let e = ext();
    assert_eq!(ts_type("", &e), "void");
    assert_eq!(ts_type("()", &e), "void");
}

/// 外部 crate 的类型走映射表
#[test]
fn test_ts_type_external() {
    assert_eq!(ts_type("Lang", &ext()), "\"zh_cn\" | \"en_us\"");
    // 表里没有的仍按本 crate 类型处理
    assert_eq!(ts_type("Lang", &BTreeMap::new()), "Lang");
}

/// 含联合的内层要加括号，否则 `A | null[]` 是错的
#[test]
fn test_ts_type_vec_of_option() {
    assert_eq!(ts_type("Vec<Option<String>>", &ext()), "(string | null)[]");
}

/// 三态补丁：Option<Option<T>> 一定可缺省
#[test]
fn test_ts_field_tri_state() {
    assert_eq!(
        ts_field("Option<Option<String>>", false, &ext()),
        f("string | null", true)
    );
    assert_eq!(
        ts_field("Option<Option<String>>", true, &ext()),
        f("string | null", true)
    );
}

/// 只 derive Deserialize 的入参 DTO，其 Option 字段可缺省；输出 DTO 必填
#[test]
fn test_ts_field_input_only() {
    assert_eq!(
        ts_field("Option<String>", true, &ext()),
        f("string | null", true)
    );
    assert_eq!(
        ts_field("Option<String>", false, &ext()),
        f("string | null", false)
    );
    assert_eq!(ts_field("String", false, &ext()), f("string", false));
}

/// 注入参数按类型名剔除，与位置无关；三种顺序都要过
#[test]
fn test_ts_params_drops_injected() {
    let e = ext();
    let cases = [
        "app: AppHandle, name: String",
        "window: WebviewWindow, app: AppHandle, id: String",
        "app: AppHandle, window: WebviewWindow, uuid: String",
    ];
    for case in cases {
        let ps = ts_params(case, &e);
        assert!(
            !ps.iter().any(|q| q.name == "app" || q.name == "window"),
            "{case} -> {ps:?}"
        );
    }
    assert_eq!(
        ts_params("window: WebviewWindow, app: AppHandle, id: String", &e),
        vec![p("id", "string", false)]
    );
}

/// 参数名转 camelCase，`r#type` 去掉原始标识符前缀
#[test]
fn test_ts_params_names_and_types() {
    let e = ext();
    assert_eq!(
        ts_params("project_id: String, page: Option<u32>, r#type: String", &e),
        vec![
            p("projectId", "string", false),
            p("page", "number | null", false),
            p("type", "string", false),
        ]
    );
}

/// Result 剥掉外层，只留 Ok 类型
#[test]
fn test_result_ok() {
    assert_eq!(result_ok("Result<Vec<NewsItem>, String>"), "Vec<NewsItem>");
    assert_eq!(result_ok("Result<(), String>"), "()");
    assert_eq!(result_ok("bool"), "bool");
    assert_eq!(result_ok(""), "");
}

/// 命名转换，含 Oauth -> OAuth 修正
#[test]
fn test_name_helpers() {
    assert_eq!(camel("add_modpack_search"), "addModpackSearch");
    assert_eq!(camel("account_cancel_oauth"), "accountCancelOAuth");
    assert_eq!(pascal("account-cancel-oauth"), "AccountCancelOAuth");
    assert_eq!(pascal("add-pack-progress"), "AddPackProgress");
    assert_eq!(screaming("account-change"), "ACCOUNT_CHANGE");
}

