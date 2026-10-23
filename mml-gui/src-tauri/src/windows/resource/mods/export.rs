//! 从资源窗口的扫描缓存导出模组清单。

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::{dtos::mod_export_dto::ModExportDto, err_keys, windows::instance_snapshot};
use super::item::{ModCtx, mod_item};
use super::super::model::window_model;

const FIELDS: &[&str] = &[
    "name", "modId", "version", "file", "author", "description", "url",
    "source", "projectId", "fileId", "sha1", "disable", "side", "loaders", "note",
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
    let rows = model.with_cache(|cache| {
        let online: HashMap<_, _> = cache.online.values()
            .map(|info| (info.sha1.clone(), info.clone())).collect();
        let ctx = ModCtx {
            notes: &settings.mod_name, sources: &settings.mod_source,
            online: &online, game_path: &path, loader: cache.loader,
        };
        cache.list.iter()
            .filter(|item| ids.is_empty() || ids.contains(item.uuid.to_string().as_str()))
            .filter_map(|item| mod_item(item, &ctx, false))
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
    }).ok_or_else(|| err_keys::MOD_EXPORT_SCAN.to_string())?
        .map_err(|e| e.to_string())?;
    if !ids.is_empty() && rows.len() != ids.len() {
        return Err(err_keys::MOD_EXPORT_SCAN.to_string());
    }
    let text = render(&rows, &config.fields, &config.format)?;
    let count = rows.len();
    tokio::task::spawn_blocking(move || {
        if op.is_cancelled() { return Err(err_keys::CANCELLED.to_string()); }
        std::fs::write(&config.file, text).map_err(|e| e.to_string())?;
        Ok(count)
    }).await.map_err(|e| e.to_string())?
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Array(items) => items.iter().map(text_of).collect::<Vec<_>>().join(", "),
        _ => value.to_string(),
    }
}

fn csv_cell(value: &str) -> String {
    // 表格软件会执行以这些符号起始的公式，文本字段必须保持为文本。
    let value = if value.trim_start().starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else { value.to_string() };
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn render(rows: &[Value], fields: &[String], format: &str) -> Result<String, String> {
    let unique: HashSet<_> = fields.iter().collect();
    if fields.is_empty() || unique.len() != fields.len()
        || fields.iter().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err(err_keys::EXPORT_FIELDS.to_string());
    }
    match format {
        "json" => {
            let rows: Vec<Map<String, Value>> = rows.iter().map(|row| fields.iter()
                .map(|key| (key.clone(), row[key].clone())).collect()).collect();
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())
        }
        "csv" => {
            let mut lines = vec![fields.iter().map(|s| csv_cell(s)).collect::<Vec<_>>().join(",")];
            lines.extend(rows.iter().map(|row| fields.iter()
                .map(|key| csv_cell(&text_of(&row[key]))).collect::<Vec<_>>().join(",")));
            Ok(lines.join("\r\n") + "\r\n")
        }
        "txt" => Ok(rows.iter().map(|row| fields.iter()
            .map(|key| format!("{key}: {}", text_of(&row[key])))
            .collect::<Vec<_>>().join("\n")).collect::<Vec<_>>().join("\n\n")),
        _ => Err(err_keys::EXPORT_PACK_TYPE.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selected_fields_and_csv_escaping() {
        let rows = vec![json!({"name": "中文,\"a\"\nline", "version": "=1+1", "file": "private.jar"})];
        let fields = vec!["name".into(), "version".into()];
        let csv = render(&rows, &fields, "csv").unwrap();
        assert!(csv.contains("\"中文,\"\"a\"\"\nline\""));
        assert!(csv.contains("\"'=1+1\""));
        let json: Value = serde_json::from_str(&render(&rows, &fields, "json").unwrap()).unwrap();
        assert!(json[0].get("file").is_none());
        assert_eq!(json[0]["version"], "=1+1");
        assert!(render(&rows, &["icon".into()], "txt").is_err());
    }
}
