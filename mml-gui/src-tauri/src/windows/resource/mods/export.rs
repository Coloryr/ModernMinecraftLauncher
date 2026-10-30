//! 从资源窗口的扫描缓存导出模组清单。

use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::Path,
};

use serde_json::{Map, Value};

use super::super::model::window_model;
use super::item::{ModCtx, mod_item};
use crate::{dtos::mod_export_dto::ModExportDto, err_keys, windows::instance_snapshot};

/// 允许导出的字段名，与窗口 model 里那份 DTO 的键一致
const FIELDS: &[&str] = &[
    "name",
    "modId",
    "version",
    "file",
    "author",
    "description",
    "url",
    "source",
    "projectId",
    "fileId",
    "sha1",
    "disable",
    "side",
    "loaders",
    "note",
];

/// 导出已扫描的模组，字段顺序与用户选择一致。
///
/// - `uuid`: 实例标识。
/// - `config`: 文件、格式、字段与模组范围。
///
/// # 返回值
/// 已写出的条目数；未扫描、选项无效或写入失败时返回错误。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_export_mods(uuid: String, config: ModExportDto) -> Result<usize, String> {
    let game = instance_snapshot(&uuid)?;
    let model = window_model(&uuid)?;
    let op = model.begin();
    let settings = crate::gui_setting::load(&game).mods;
    let path = game.get_game_path();
    let ids: HashSet<_> = config.ids.iter().map(String::as_str).collect();
    let rows = model
        .with_cache(|cache| {
            let online: HashMap<_, _> = cache
                .online
                .values()
                .map(|info| (info.sha1.clone(), info.clone()))
                .collect();
            let ctx = ModCtx {
                notes: &settings.mod_name,
                sources: &settings.mod_source,
                online: &online,
                game_path: &path,
                loader: cache.loader,
            };
            cache
                .list
                .iter()
                .filter(|item| ids.is_empty() || ids.contains(item.uuid.to_string().as_str()))
                .filter_map(|item| mod_item(item, &ctx, false))
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()
        })
        .ok_or_else(|| err_keys::MOD_EXPORT_SCAN.to_string())?
        .map_err(|e| e.to_string())?;
    if !ids.is_empty() && rows.len() != ids.len() {
        return Err(err_keys::MOD_EXPORT_SCAN.to_string());
    }
    let text = render(&rows, &config.fields, &config.format)?;
    let count = rows.len();
    tokio::task::spawn_blocking(move || {
        if op.is_cancelled() {
            return Err(err_keys::CANCELLED.to_string());
        }
        crate::windows::export_file::write(Path::new(&config.file), |output| {
            output
                .write_all(text.as_bytes())
                .map_err(|e| e.to_string())?;
            if op.is_cancelled() {
                return Err(err_keys::CANCELLED.to_string());
            }
            Ok(())
        })?;
        Ok(count)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 把一段文本写到用户选定的文件，供依赖检测结果导出到文件
///
/// - `file`: 用户选择的输出路径
/// - `content`: 要写入的文本
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`；路径无效或写入失败返回错误文案
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_export_text(file: String, content: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        crate::windows::export_file::write(Path::new(&file), |output| {
            output
                .write_all(content.as_bytes())
                .map_err(|e| e.to_string())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 把字段值转成展示文本：数组按逗号拼接，null 为空串
fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Array(items) => items.iter().map(text_of).collect::<Vec<_>>().join(", "),
        _ => value.to_string(),
    }
}

/// 按 CSV 规则加引号转义；以公式符号开头的加单引号，防表格软件当成公式执行
fn csv_cell(value: &str) -> String {
    // 表格软件会执行以这些符号起始的公式，文本字段必须保持为文本。
    let value = if value
        .trim_start()
        .starts_with(['=', '+', '-', '@', '\t', '\r'])
    {
        format!("'{value}")
    } else {
        value.to_string()
    };
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// 按选定字段与格式渲染整份导出内容
///
/// - `rows`: 已转成 JSON 的模组条目，每项是字段名 → 值
/// - `fields`: 用户选定的字段名，顺序即输出顺序
/// - `format`: 导出格式，取值 `json` / `csv` / `txt`
///
/// # 返回值
///
/// 渲染好的文本；字段为空、重复、不认识，或格式不认识时返回错误串
fn render(rows: &[Value], fields: &[String], format: &str) -> Result<String, String> {
    let unique: HashSet<_> = fields.iter().collect();
    if fields.is_empty()
        || unique.len() != fields.len()
        || fields.iter().any(|key| !FIELDS.contains(&key.as_str()))
    {
        return Err(err_keys::EXPORT_FIELDS.to_string());
    }
    match format {
        "json" => {
            let rows: Vec<Map<String, Value>> = rows
                .iter()
                .map(|row| {
                    fields
                        .iter()
                        .map(|key| (key.clone(), row[key].clone()))
                        .collect()
                })
                .collect();
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())
        }
        "csv" => {
            let mut lines = vec![
                fields
                    .iter()
                    .map(|s| csv_cell(s))
                    .collect::<Vec<_>>()
                    .join(","),
            ];
            lines.extend(rows.iter().map(|row| {
                fields
                    .iter()
                    .map(|key| csv_cell(&text_of(&row[key])))
                    .collect::<Vec<_>>()
                    .join(",")
            }));
            Ok(lines.join("\r\n") + "\r\n")
        }
        "txt" => Ok(rows
            .iter()
            .map(|row| {
                fields
                    .iter()
                    .map(|key| format!("{key}: {}", text_of(&row[key])))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n")),
        _ => Err(err_keys::EXPORT_PACK_TYPE.to_string()),
    }
}

#[cfg(test)]
mod tests {
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
}
