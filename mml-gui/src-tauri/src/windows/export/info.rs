//! 可分享的实例信息，不包含账户、环境变量和代理凭据。

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct InstanceReport {
    format: &'static str,
    name: String,
    uuid: uuid::Uuid,
    minecraft: String,
    loader: mml_game::loader::LoaderType,
    loader_version: Option<String>,
    custom_loader: bool,
    modpack: bool,
    project_id: Option<String>,
    file_id: Option<String>,
    mods: Vec<String>,
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
        let mods_dir = game.get_mods_path();
        let mut mods = if mods_dir.exists() {
            std::fs::read_dir(mods_dir).map_err(|e| e.to_string())?
                .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
        } else { Vec::new() };
        mods.sort();
        let report = InstanceReport {
            format: "mml-instance-info-v1", name: game.name, uuid: game.uuid,
            minecraft: game.version, loader: game.loader, loader_version: game.loader_version,
            custom_loader: game.custom_loader.is_some(), modpack: game.is_modpack,
            project_id: game.pid, file_id: game.fid, mods,
        };
        let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(file, bytes).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}
