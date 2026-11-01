use super::*;
use serde_json::json;

#[test]
fn selected_fields_and_csv_escaping() {
    let rows =
        vec![json!({"name": "中文,\"a\"\nline", "version": "=1+1", "file": "private.jar"})];
    let fields = vec!["name".into(), "version".into()];
    let csv = render(&rows, &fields, "csv").unwrap();
    assert!(csv.contains("\"中文,\"\"a\"\"\nline\""));
    assert!(csv.contains("\"'=1+1\""));
    let json: Value = serde_json::from_str(&render(&rows, &fields, "json").unwrap()).unwrap();
    assert!(json[0].get("file").is_none());
    assert_eq!(json[0]["version"], "=1+1");
    assert!(render(&rows, &["icon".into()], "txt").is_err());
}

#[test]
fn json_keeps_value_types_and_only_requested_fields() {
    let rows = [
        json!({"name":"中文", "disable":false, "loaders":["fabric","quilt"], "note":null, "icon":"private"}),
    ];
    let fields = ["name", "disable", "loaders", "note"].map(String::from);
    let result: Value = serde_json::from_str(&render(&rows, &fields, "json").unwrap()).unwrap();
    assert_eq!(
        result,
        json!([{"name":"中文", "disable":false, "loaders":["fabric","quilt"], "note":null}])
    );
}

#[test]
fn text_and_csv_follow_field_order_and_preserve_multiline_text() {
    let rows = [
        json!({"name":"中文,\"引号\"\r\n下一行", "disable":false, "loaders":["fabric","quilt"], "note":null}),
    ];
    let fields = ["disable", "loaders", "note", "name"].map(String::from);
    assert_eq!(
        render(&rows, &fields, "txt").unwrap(),
        "disable: false\nloaders: fabric, quilt\nnote: \nname: 中文,\"引号\"\r\n下一行"
    );
    assert_eq!(
        render(&rows, &fields, "csv").unwrap(),
        "\"disable\",\"loaders\",\"note\",\"name\"\r\n\"false\",\"fabric, quilt\",\"\",\"中文,\"\"引号\"\"\r\n下一行\"\r\n"
    );
}

#[test]
fn empty_lists_are_valid_in_each_format() {
    let fields = ["name".to_string()];
    assert_eq!(render(&[], &fields, "json").unwrap(), "[]");
    assert_eq!(render(&[], &fields, "csv").unwrap(), "\"name\"\r\n");
    assert_eq!(render(&[], &fields, "txt").unwrap(), "");
}

#[test]
fn invalid_field_selection_or_format_is_rejected() {
    for fields in [
        vec![],
        vec!["name".into(), "name".into()],
        vec!["token".into()],
    ] {
        for format in ["txt", "csv", "json"] {
            assert_eq!(
                render(&[], &fields, format).unwrap_err(),
                err_keys::EXPORT_FIELDS
            );
        }
    }
    assert_eq!(
        render(&[], &["name".into()], "exe").unwrap_err(),
        err_keys::EXPORT_PACK_TYPE
    );
}

#[test]
fn spreadsheet_formula_prefixes_are_escaped_without_changing_json() {
    for text in ["=1+1", "+1", "-1", "@SUM(A1)", " \t=1", "\r\n+1"] {
        let rows = [json!({"name":text})];
        let fields = ["name".to_string()];
        assert_eq!(
            render(&rows, &fields, "csv").unwrap(),
            format!("\"name\"\r\n\"'{text}\"\r\n")
        );
        let json: Value =
            serde_json::from_str(&render(&rows, &fields, "json").unwrap()).unwrap();
        assert_eq!(json[0]["name"], text);
    }
}

