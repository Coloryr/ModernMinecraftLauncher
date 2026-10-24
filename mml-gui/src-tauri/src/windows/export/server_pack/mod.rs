//! 服务器分发包：清单、启动器配置和实例；导入完成后重启加载。

mod format;
mod import;

use std::{path::Path, sync::Mutex};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use mml_names::names;
use crate::{dtos::export_dto::ServerPackOptionsDto, err_keys, gui_config};
use super::archive;

pub(crate) use import::{ImportedPack, import_pack};
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
    games: &[mml_game::launcher::instance_setting_obj::InstanceSettingObj],
    file: &Path, include_launcher: bool, options: &ServerPackOptionsDto,
    gui: &mml_base::archives::BaseArchiveGui,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    if games.is_empty() { return Err(err_keys::SERVER_PACK_INVALID.into()); }
    if include_launcher && !cfg!(windows) { return Err(err_keys::SERVER_PACK_PLATFORM.into()); }
    let mut directories = std::collections::HashSet::new();
    let mut instances = Vec::with_capacity(games.len());
    let mut files = Vec::new();
    let mut data = Vec::new();
    for game in games {
        if !format::safe_component(&game.dir) || !directories.insert(game.dir.to_lowercase()) {
            return Err(err_keys::SERVER_PACK_INVALID.into());
        }
        let root = game.get_base_path();
        archive::check_destination(file, &root)?;
        let prefix = format!("{INSTANCES}{}/", game.dir);
        for path in archive::files_under(&root)? {
            let rel = path.strip_prefix(&root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            if rel != names::GAME_FILE { files.push((format!("{prefix}{rel}"), path)); }
        }
        instances.push(Instance { uuid: game.uuid, directory: game.dir.clone() });
        data.push((format!("{prefix}{}", names::GAME_FILE), serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?));
    }
    if include_launcher { files.push((LAUNCHER.into(), std::env::current_exe().map_err(|e| e.to_string())?)); }

    let page = crate::windows::custom_ui::page_file();
    let page_on = options.page && page.is_file();
    let home = crate::windows::custom_home::zip_path();
    let home_on = options.home && home.is_file();
    let mut ui = gui_config::get();
    let mut css_packages = Vec::new();
    if options.css {
        for item in &ui.client.custom_css {
            let package = crate::windows::custom_ui::css_package(&item.id);
            if package.is_file() {
                css_packages.push((format!("mml/custom_ui/css/{}.zip", item.id), package));
            }
        }
    }
    // 实例锁定只认一个实例：包里不止一个，或锁的不是包里的，就去掉。否则玩家进不去包里其它实例
    let locked_in_pack = games.len() == 1
        && ui.client.lock_instance.parse::<uuid::Uuid>().is_ok_and(|id| id == games[0].uuid);
    if !locked_in_pack { ui.client.lock_instance.clear(); }
    // 导入后落到包里的第一个实例，别停在导出者当时选的那个
    if let Some(first) = games.first() { ui.main_window.selected_instance = first.uuid.to_string(); }
    // 界面配置里的开关要跟实际带走的文件一致，否则玩家导入后会指向不存在的主题
    ui.client.custom_css_on = options.css && !css_packages.is_empty();
    ui.client.custom_css.retain(|item| {
        options.css && crate::windows::custom_ui::css_package(&item.id).is_file()
    });
    ui.client.custom_home = options.home && home_on;
    ui.client.custom_page_on = options.page && page_on;
    if page_on { files.push(("mml/custom_ui/page.zip".into(), page)); }
    files.extend(css_packages);
    if home_on { files.push(("mml/home".into(), home)); }
    // 客户端设置锁也在界面配置里：不带上，玩家导入后就变成未锁定
    if options.gui_config
        && let Some(lock) = crate::client_lock::read_bytes()
    {
        data.push((format!("mml/{}", crate::client_lock::FILE_NAME), lock));
    }

    let manifest = Manifest {
        format: "mml-server-pack".into(), version: 1, instances, launcher: include_launcher,
    };
    data.push((MANIFEST.into(), serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?));
    if options.config {
        data.push((CONFIG.into(), serde_json::to_vec_pretty(&*mml_config::read_config()).map_err(|e| e.to_string())?));
    }
    if options.gui_config {
        data.push((GUI_CONFIG.into(), serde_json::to_vec_pretty(&ui).map_err(|e| e.to_string())?));
    }
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
        let ImportedPack { config, gui } = imported;
        if let Some(config) = config { *mml_config::write_config() = config; }
        if let Some(gui) = gui { gui_config::set(gui); }
        launch_guard.commit();
        crate::windows::window_restart_app(app);
        Ok(())
    }).await.map_err(|e| e.to_string())?
}
