//! 可分享的实例信息，不包含账户、环境变量和代理凭据。

use serde::Serialize;
use std::{fs, io::{ErrorKind, Write}, path::Path};
use mml_game::{launcher::instance_setting_obj::InstanceSettingObj, loader::LoaderType};
use mml_names::names;

/// 可分享报告只收版本与模组文件名，不复制实例启动配置。
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct InstanceReport {
    /// 报告格式标识。
    format: &'static str,
    /// 实例显示名。
    name: String,
    /// 实例标识。
    uuid: uuid::Uuid,
    /// Minecraft 版本。
    minecraft: String,
    /// 当前加载器。
    loader: mml_game::loader::LoaderType,
    /// 加载器版本，原版可为空。
    loader_version: Option<String>,
    /// 当前是否使用自定义加载器。
    custom_loader: bool,
    /// 是否来自整合包。
    modpack: bool,
    /// 整合包项目号，可为空。
    project_id: Option<String>,
    /// 整合包文件号，可为空。
    file_id: Option<String>,
    /// 顶层模组文件名，含禁用模组，按名字排序。
    mods: Vec<String>,
}

/// 按模组扫描的后缀口径列出普通文件，忽略目录和链接。
fn report(game: &InstanceSettingObj, mods_dir: &Path) -> Result<InstanceReport, String> {
    let entries = match fs::read_dir(mods_dir) {
        Ok(entries) => Some(entries),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let mut mods = Vec::new();
    for entry in entries.into_iter().flatten() {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() { continue; }
        if entry.path().extension().is_some_and(|ext|
            [names::JAR_EXT, names::DISABLE_EXT, names::DISABLED_EXT].iter()
                .any(|suffix| ext.eq_ignore_ascii_case(suffix))) {
            mods.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    mods.sort();
    Ok(InstanceReport {
        format: "mml-instance-info-v1", name: game.name.clone(), uuid: game.uuid,
        minecraft: game.version.clone(), loader: game.loader, loader_version: game.loader_version.clone(),
        custom_loader: game.loader == LoaderType::Custom, modpack: game.is_modpack,
        project_id: game.pid.clone(), file_id: game.fid.clone(), mods,
    })
}

/// 保存可分享的实例信息。
/// - `uuid`: 实例标识。
/// - `file`: 用户选择的 JSON 保存路径。
/// # 返回值
/// 写入结果。
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub async fn export_generate_info(uuid: String, file: String) -> Result<(), String> {
    let game = super::super::instance_snapshot(&uuid)?;
    tokio::task::spawn_blocking(move || {
        let report = report(&game, &game.get_mods_path())?;
        let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
        crate::windows::export_file::write(Path::new(&file), |output|
            output.write_all(&bytes).map_err(|e| e.to_string()))
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn report_filters_non_mods_and_sorts_enabled_and_disabled_files() {
        let dir = crate::test_support::temp_dir("instance-report-mods");
        for name in ["z.jar", "中文.JAR", "a.jar.disable", "b.DISABLED", "readme.txt", "report.json"] {
            fs::write(dir.join(name), "sample").unwrap();
        }
        fs::create_dir(dir.join("directory.jar")).unwrap();
        let report = report(&InstanceSettingObj::default(), &dir).unwrap();
        assert_eq!(report.mods, ["a.jar.disable", "b.DISABLED", "z.jar", "中文.JAR"]);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn report_has_no_private_launch_settings_and_uses_active_loader() {
        let dir = crate::test_support::temp_dir("instance-report-private");
        let mut game: InstanceSettingObj = serde_json::from_value(json!({
            "Name": "中文实例", "Version": "1.21.1", "JvmLocal": "private-java-path",
            "ServerUrl": "private-server-url", "CustomLoader": {},
            "AdvanceJvm": {"ClassPath": "private-class-path"}
        })).unwrap();
        let value: Value = serde_json::to_value(report(&game, &dir.join("missing")).unwrap()).unwrap();
        assert_eq!(value["Name"], "中文实例");
        assert_eq!(value["Mods"], json!([]));
        assert_eq!(value["CustomLoader"], false);
        assert!(!value.to_string().contains("private-"));
        assert_eq!(value.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>(),
            ["CustomLoader", "FileId", "Format", "Loader", "LoaderVersion", "Minecraft", "Modpack", "Mods", "Name", "ProjectId", "Uuid"]);
        game.loader = LoaderType::Custom;
        game.custom_loader = None;
        assert!(report(&game, &dir).unwrap().custom_loader);
        crate::test_support::cleanup(&[&dir]);
    }

    #[test]
    fn report_propagates_unreadable_mod_directory() {
        let dir = crate::test_support::temp_dir("instance-report-invalid");
        let file = dir.join("mods");
        fs::write(&file, "not a directory").unwrap();
        assert!(report(&InstanceSettingObj::default(), &file).is_err());
        crate::test_support::cleanup(&[&dir]);
    }
}
