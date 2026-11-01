use super::*;

/// 单个值的 facets 应输出 `[["data:value"]]` 形式
#[test]
fn build_facets_single_value() {
    let facets = vec![build_versions(vec![String::from("1.20.4")])];
    assert_eq!(build_facets(facets), r#"[["versions:1.20.4"]]"#);
}

/// 同一 data 下的多个值各自成组，对应 Modrinth 的 OR 语义
#[test]
fn build_facets_multiple_values() {
    let facets = vec![build_categories(vec![
        String::from("adventure"),
        String::from("optimization"),
    ])];
    assert_eq!(
        build_facets(facets),
        r#"[["categories:adventure"],["categories:optimization"]]"#
    );
}

/// 多个 data 的 facets 依次拼接
#[test]
fn build_facets_multiple_groups() {
    let facets = vec![
        build_project_type(vec![CLASS_MODPACK.to_string()]),
        build_versions(vec![String::from("1.21.6")]),
    ];
    assert_eq!(
        build_facets(facets),
        r#"[["project_type:modpack"],["versions:1.21.6"]]"#
    );
}

/// values 为空的组应被跳过
#[test]
fn build_facets_skips_empty_group() {
    let facets = vec![
        build_categories(Vec::new()),
        build_project_type(vec![CLASS_MOD.to_string()]),
    ];
    assert_eq!(build_facets(facets), r#"[["project_type:mod"]]"#);
}

/// ModrinthSortType 的排序参数名
#[test]
fn sort_type_index() {
    assert_eq!(ModrinthSortType::Relevance.to_string(), "relevance");
    assert_eq!(ModrinthSortType::Downloads.to_string(), "downloads");
    assert_eq!(ModrinthSortType::Follows.to_string(), "follows");
    assert_eq!(ModrinthSortType::Newest.to_string(), "newest");
    assert_eq!(ModrinthSortType::Updated.to_string(), "updated");
}

