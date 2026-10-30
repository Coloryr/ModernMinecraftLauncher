//! 服务器分发包：清单、启动器配置和实例；导入完成后重启加载。

mod format;
mod import;

use super::archive;
use crate::{dtos::export_dto::ServerPackOptionsDto, err_keys, gui_config};
use mml_names::names;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use tauri::AppHandle;

pub(crate) use format::is_server_pack;
pub(crate) use import::{ImportedPack, import_pack};

/// 包内清单文件名，也是「这是不是一个服务器包」的判据
const MANIFEST: &str = "mml-server.json";
/// 包内内核配置，可选
const CONFIG: &str = "mml/config.json";
/// 包内界面配置，可选
const GUI_CONFIG: &str = "mml/gui_config.json";
/// 包内实例目录前缀，后面接实例目录名
const INSTANCES: &str = "mml/minecraft/instances/";
/// 可选携带的启动器文件名，仅 Windows 允许带
const LAUNCHER: &str = "MML.exe";
/// 可选携带的 Java 运行时目录前缀，后面接运行时名称
///
/// 导入端按 mml/ 前缀落到运行目录，正好是启动器认的 java/ 位置
const JAVA: &str = "mml/java";
/// 导入互斥：同时只跑一次导入，免得两份包互相覆盖
static IMPORTING: Mutex<()> = Mutex::new(());

/// 包内清单 `mml-server.json`
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Manifest {
    /// 格式标识，必须等于 `mml-server-pack`
    format: String,
    /// 清单版本，目前只认 `1`
    version: u32,
    /// 包内实例，至少一个
    instances: Vec<Instance>,
    /// 是否携带启动器 `MML.exe`
    launcher: bool,
}

/// 清单里的一条实例
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Instance {
    /// 实例标识，与包内 game.json 里的一致
    uuid: uuid::Uuid,
    /// 包内目录名，相对 [`INSTANCES`]
    directory: String,
}

/// 打一个服务器包：多个实例、可选启动器、可选配置与自定义界面文件
///
/// 实例锁定只留一个：包里不止一个实例、或锁的不是包里的那个时清空锁，
/// 否则玩家进不去包里其它实例。界面配置里的开关按实际带走的文件回填。
///
/// - `games`: 要打进去的实例
/// - `file`: 输出 zip
/// - `include_launcher`: 是否带上当前启动器可执行文件
/// - `options`: 生成选项，控制配置与自定义界面文件是否随包
/// - `gui`: 进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 写盘成功返回 `Ok(())`；实例为空、目录名非法、跨平台带启动器或写盘失败返回错误
pub(super) fn export_pack(
    games: &[mml_game::launcher::instance_setting_obj::InstanceSettingObj],
    file: &Path,
    include_launcher: bool,
    options: &ServerPackOptionsDto,
    gui: &mml_base::archives::BaseArchiveGui,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    if games.is_empty() {
        return Err(err_keys::SERVER_PACK_INVALID.into());
    }
    if include_launcher && !cfg!(windows) {
        return Err(err_keys::SERVER_PACK_PLATFORM.into());
    }
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
            let rel = path
                .strip_prefix(&root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if rel != names::GAME_FILE {
                files.push((format!("{prefix}{rel}"), path));
            }
        }
        instances.push(Instance {
            uuid: game.uuid,
            directory: game.dir.clone(),
        });
        data.push((
            format!("{prefix}{}", names::GAME_FILE),
            serde_json::to_vec_pretty(game).map_err(|e| e.to_string())?,
        ));
    }
    if include_launcher {
        files.push((
            LAUNCHER.into(),
            std::env::current_exe().map_err(|e| e.to_string())?,
        ));
    }
    // 内置 Java：选中的运行时各自照原样打进 java/<名称>/，导入端解到运行目录后登记进 Java 列表，
    // 玩家没有装过对应版本的 Java 也能直接启动。可以同时带多个（不同版本各一套）
    let mut bundled = std::collections::HashSet::new();
    for name in options.java.iter().filter(|name| !name.is_empty()) {
        let info =
            mml_jvms::get_java_info(name).ok_or_else(|| err_keys::SERVER_PACK_INVALID.to_string())?;
        // 运行时名称要当目录名进 zip：按包内条目的同一套规则清洗，清洗完不合法就不带
        let name = java_dir_name(name);
        if !format::safe_component(&name) || !bundled.insert(name.clone()) {
            return Err(err_keys::SERVER_PACK_INVALID.into());
        }
        // 可执行文件是 <根>/bin/java(或 javaw.exe)，退两级才是运行时根目录
        let exe = info.path.as_path();
        let root = exe
            .parent()
            .and_then(|bin| bin.parent())
            .ok_or_else(|| err_keys::SERVER_PACK_INVALID.to_string())?;
        for path in archive::files_under(root)? {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((format!("{JAVA}/{name}/{rel}"), path));
        }
    }

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
        && ui
            .client
            .lock_instance
            .parse::<uuid::Uuid>()
            .is_ok_and(|id| id == games[0].uuid);
    if !locked_in_pack {
        ui.client.lock_instance.clear();
    }
    // 导入后落到包里的第一个实例，别停在导出者当时选的那个
    if let Some(first) = games.first() {
        ui.main_window.selected_instance = first.uuid.to_string();
    }
    // 界面配置里的开关要跟实际带走的文件一致，否则玩家导入后会指向不存在的主题
    ui.client.custom_css_on = options.css && !css_packages.is_empty();
    ui.client
        .custom_css
        .retain(|item| options.css && crate::windows::custom_ui::css_package(&item.id).is_file());
    ui.client.custom_home = options.home && home_on;
    ui.client.custom_page_on = options.page && page_on;
    if page_on {
        files.push(("mml/custom_ui/page.zip".into(), page));
    }
    files.extend(css_packages);
    if home_on {
        files.push(("mml/home".into(), home));
    }
    // 背景图是渲染好的缓存文件；界面配置里的 bg_source 可能指向导出者本机路径，
    // 只带这份图就能在导入端显示
    if options.bg
        && let Some(dir) = gui_config::dir()
    {
        let bg = dir.join("bg_image");
        if bg.is_file() {
            files.push(("mml/bg_image".into(), bg));
        }
    }
    // 客户端设置锁也在界面配置里：不带上，玩家导入后就变成未锁定
    if options.gui_config
        && let Some(lock) = crate::client_lock::read_bytes()
    {
        data.push((format!("mml/{}", crate::client_lock::FILE_NAME), lock));
    }
    // 自定义程序图标：文件名在界面配置里，文件本身要一起带上，导入端才显示得出同一张图
    if options.gui_config
        && let Some(icon) = crate::windows::settings::icon::icon_path()
    {
        files.push((format!("mml/{}", crate::windows::settings::icon::ICON_FILE), icon));
    }

    let manifest = Manifest {
        format: "mml-server-pack".into(),
        version: 1,
        instances,
        launcher: include_launcher,
    };
    data.push((
        MANIFEST.into(),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    ));
    if options.config {
        data.push((
            CONFIG.into(),
            serde_json::to_vec_pretty(&*mml_config::read_config()).map_err(|e| e.to_string())?,
        ));
    }
    if options.gui_config {
        data.push((
            GUI_CONFIG.into(),
            serde_json::to_vec_pretty(&ui).map_err(|e| e.to_string())?,
        ));
    }
    archive::write_zip(file, &files, &data, gui, cancel)
}

/// 运行时名称当目录名用前先清洗：替换掉包内条目不允许的字符，去掉结尾的点与空格
///
/// - `name`: Java 显示名
///
/// # 返回值
///
/// 可直接当 zip 目录名用的字符串；全部字符都不合法时为空串
fn java_dir_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "\\/:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    cleaned.trim_end_matches(['.', ' ']).to_string()
}

/// 导入服务器包并重新载入启动器
///
/// - `app`: 应用句柄，重启经它发起
/// - `file`: 用户拖入的服务器包路径
///
/// # 返回值
///
/// 导入完成会重启进程，不会正常返回；校验、占用或覆盖失败返回错误文案
#[gui_macros::ipc_group("export")]
#[gui_macros::command]
pub async fn export_import_server_pack(app: AppHandle, file: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let _guard = IMPORTING
            .try_lock()
            .map_err(|_| err_keys::SERVER_PACK_BUSY.to_string())?;
        let mut launch_guard = crate::windows::main::launch::import_guard()?;
        let root = gui_config::dir().ok_or(err_keys::SERVER_PACK_INVALID)?;
        let imported = mml_config::config_save::replace_before_restart(|| {
            import_pack(Path::new(&file), &root)
        })?;
        let ImportedPack { config, gui, java } = imported;
        if let Some(config) = config {
            *mml_config::write_config() = config;
        }
        if let Some(gui) = gui {
            gui_config::set(gui);
        }
        // 包里自带的 Java 已经在运行目录的 java/ 下：登记进列表，导入完成重启后就能直接用。
        // 必须在配置替换之后做，否则写进的是被覆盖掉的那份配置
        for name in java {
            let dir = root.join(names::JAVA_DIR).join(&name);
            if let Some(exe) = mml_jvms::find_java_from_path(&dir) {
                mml_jvms::add_item(Some(name), exe.to_string_lossy().to_string());
            }
        }
        launch_guard.commit();
        crate::windows::window_restart_app(app);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::java_dir_name;

    /// 运行时名称要当包内目录名：非法字符换成下划线，结尾的点与空格去掉
    #[test]
    fn java_dir_name_cleans_path_chars() {
        assert_eq!(java_dir_name("OpenJDK-17.0.1-x86_64"), "OpenJDK-17.0.1-x86_64");
        assert_eq!(java_dir_name("Java 17 (x64)"), "Java 17 (x64)");
        assert_eq!(java_dir_name("jdk21. "), "jdk21");
        assert!(super::format::safe_component(&java_dir_name("a/b:c*?")));
    }
}
