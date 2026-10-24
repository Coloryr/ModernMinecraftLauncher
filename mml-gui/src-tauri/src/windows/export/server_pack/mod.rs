//! 服务器分发包：清单、启动器配置和实例；导入完成后重启加载。

mod format;
mod import;

use std::{path::Path, sync::Mutex};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use mml_names::names;
use crate::{err_keys, gui_config};
use super::archive;

pub(crate) use import::import_pack;
pub(crate) use format::is_server_pack;

const MANIFEST: &str = "mml-server.json";
const CONFIG: &str = "mml/config.json";
const GUI_CONFIG: &str = "mml/gui_config.json";
const INSTANCES: &str = "mml/minecraft/instances/";
const LAUNCHER: &str = "MML.exe";
static IMPORTING: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Manifest {
    format: String,
    version: u32,
    instances: Vec<Instance>,
    launcher: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Instance {
    uuid: uuid::Uuid,
    directory: String,
}

pub(super) fn export_pack(
    game: &mml_game::launcher::instance_setting_obj::InstanceSettingObj,
    file: &Path, include_launcher: bool, gui: &mml_base::archives::BaseArchiveGui,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    if !format::safe_component(&game.dir) { return Err(err_keys::SERVER_PACK_INVALID.into()); }
    if include_launcher && !cfg!(windows) { return Err(err_keys::SERVER_PACK_PLATFORM.into()); }
    let root = game.get_base_path();
    archive::check_destination(file, &root)?;
    let prefix = format!("{INSTANCES}{}/", game.dir);
    let mut files = Vec::new();
    for path in archive::files_under(&root)? {
        let rel = path.strip_prefix(&root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if rel != names::GAME_FILE { files.push((format!("{prefix}{rel}"), path)); }
    }
    if include_launcher { files.push((LAUNCHER.into(), std::env::current_exe().map_err(|e| e.to_string())?)); }
    let manifest = Manifest {
        format: "mml-server-pack".into(), version: 1,
        instances: vec![Instance { uuid: game.uuid, directory: game.dir.clone() }], launcher: include_launcher,
    };
    let data = vec![
        (MANIFEST.into(), serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?),
        (CONFIG.into(), serde_json::to_vec_pretty(&*mml_config::read_config()).map_err(|e| e.to_string())?),
        (GUI_CONFIG.into(), serde_json::to_vec_pretty(&gui_config::get()).map_err(|e| e.to_string())?),
        (format!("{prefix}{}", names::GAME_FILE), serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?),
    ];
    archive::write_zip(file, &files, &data, gui, cancel)
}

/// 导入服务器包并重新载入启动器。
/// - `app`: 应用句柄。
/// - `file`: 用户拖入的服务器包路径。
/// # 返回值
/// 校验或覆盖失败时返回错误。
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub async fn export_import_server_pack(app: AppHandle, file: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let _guard = IMPORTING.try_lock().map_err(|_| err_keys::SERVER_PACK_BUSY.to_string())?;
        let mut launch_guard = crate::windows::main::launch::import_guard()?;
        let root = gui_config::dir().ok_or(err_keys::SERVER_PACK_INVALID)?;
        let imported = mml_config::config_save::replace_before_restart(|| import_pack(Path::new(&file), &root))?;
        *mml_config::write_config() = imported.config;
        gui_config::set(imported.gui);
        launch_guard.commit();
        crate::windows::window_restart_app(app);
        Ok(())
    }).await.map_err(|e| e.to_string())?
}
