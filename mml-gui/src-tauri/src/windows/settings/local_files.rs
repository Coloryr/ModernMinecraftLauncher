//! 设置窗口「本地文件」标签：统计运行目录下各文件夹的占用，并清理缓存类目录
//!
//! 目录来源一律走各自的访问器，不在这里另拼路径：下载缓存 / 下载内容在 mml-downloader，
//! 图标缓存在 image_manager，实例目录在 mml-game，Java 在 mml-jvms。
//! 命令带分组标记，值填原始组名 `settings`，免得组键随文件名变掉，见 AGENTS.md §4。

use std::fs;
use std::path::PathBuf;

use mml_names::names;
use mml_sys::{open_helper, path_helper};
use tauri::{AppHandle, Manager};

use crate::dtos::{LocalFileChildDto, LocalFileEntryDto};
use crate::err_keys;
use crate::log_text::LogText;

/// 一项本地文件夹
struct LocalFileKind {
    /// 稳定 id，前端按它取界面文案、发起清理 / 打开
    id: &'static str,
    /// 目录绝对路径
    dir: PathBuf,
    /// 是否允许清理；游戏实例与 Java 只统计不动
    deletable: bool,
}

/// 本标签列出的文件夹，顺序即界面顺序
///
/// `cache` 这一项是**互斥**的：它不含 `image` 与 `webview` 两个子目录 —— 那两项各自单独成行，
/// 否则几行的数字加起来会比磁盘实际占用大一截，清理时也会删掉别的行承诺保留的东西。
///
/// # 返回值
///
/// 六个文件夹的 id / 路径 / 可清理性
fn kinds() -> Vec<LocalFileKind> {
    let cache = mml_downloader::get_cache_path();
    vec![
        LocalFileKind {
            id: "cache",
            dir: cache.clone(),
            deletable: true,
        },
        LocalFileKind {
            id: "image",
            dir: crate::image_manager::icon_cache_dir(),
            deletable: true,
        },
        LocalFileKind {
            id: "download",
            dir: mml_downloader::get_download_path(),
            deletable: true,
        },
        LocalFileKind {
            id: "webview",
            dir: cache.join(names::WEBVIEW_DIR),
            // 目录本身删不得：WebView2 运行时一直占着它，走浏览数据接口清，见 clear_webview_data
            deletable: true,
        },
        LocalFileKind {
            id: "instances",
            dir: mml_game::launcher_path::instance_path::get_instance_dir(),
            deletable: false,
        },
        LocalFileKind {
            id: "java",
            dir: mml_jvms::get_java_dir(),
            deletable: false,
        },
    ]
}

/// 某一项的占用，单位字节
///
/// - `kind`: 目标文件夹
///
/// # 返回值
///
/// 目录总字节数；`cache` 已扣掉图片缓存与界面数据
fn size_of(kind: &LocalFileKind) -> u64 {
    let total = path_helper::get_folder_size(&kind.dir);
    if kind.id != "cache" {
        return total;
    }
    total
        .saturating_sub(path_helper::get_folder_size(
            kind.dir.join(names::IMAGE_DIR),
        ))
        .saturating_sub(path_helper::get_folder_size(
            kind.dir.join(names::WEBVIEW_DIR),
        ))
}

/// 采集全部文件夹的占用
///
/// # 返回值
///
/// 与 [`kinds`] 同序的 DTO 列表
fn collect() -> Vec<LocalFileEntryDto> {
    kinds()
        .into_iter()
        .map(|kind| {
            let children = match kind.id {
                "instances" => instance_sizes(),
                "download" => download_sizes(),
                _ => Vec::new(),
            };
            // 实例行按明细求和，避免为了一个总数再整棵实例目录扫第二遍
            let size = if kind.id == "instances" {
                children.iter().map(|item| item.size).sum()
            } else {
                size_of(&kind)
            };
            LocalFileEntryDto {
                size,
                path: kind.dir.to_string_lossy().to_string(),
                id: kind.id.to_string(),
                deletable: kind.deletable,
                children,
            }
        })
        .collect()
}

/// 各游戏实例的占用，按大小降序
///
/// # 返回值
///
/// 每个实例的 uuid / 名字 / 根目录与字节数；没有实例时为空列表
fn instance_sizes() -> Vec<LocalFileChildDto> {
    let mut list: Vec<LocalFileChildDto> = mml_game::get_instances()
        .into_iter()
        .map(|instance| {
            // 扫目录可能很慢：先把字段取出来放锁，别攥着实例读锁做磁盘遍历
            let (uuid, name, path) = {
                let instance = instance.read().unwrap();
                (
                    instance.uuid.to_string(),
                    instance.name.clone(),
                    instance.get_base_path(),
                )
            };
            LocalFileChildDto {
                id: uuid,
                kind: String::from("instance"),
                name,
                size: path_helper::get_folder_size(&path),
                path: path.to_string_lossy().to_string(),
            }
        })
        .collect();
    list.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    list
}

/// 下载内容里的模组与资源包占用
///
/// # 返回值
///
/// mods / resourcepacks 两个子目录的大小；目录不存在时为 0
fn download_sizes() -> Vec<LocalFileChildDto> {
    let dir = mml_downloader::get_download_path();
    [
        (String::from("mods"), names::GAME_MODS_DIR),
        (String::from("resourcepacks"), names::GAME_RESOURCEPACKS_DIR),
    ]
    .into_iter()
    .map(|(kind, name)| {
        let path = dir.join(name);
        LocalFileChildDto {
            id: kind.clone(),
            kind,
            name: String::new(),
            size: path_helper::get_folder_size(&path),
            path: path.to_string_lossy().to_string(),
        }
    })
    .collect()
}

/// 读取各文件夹占用，界面打开本标签时调用一次
///
/// 统计要遍历整棵树，实例目录可能有几万个文件，所以放到阻塞线程池里跑，别卡住 IPC。
///
/// # 返回值
///
/// 各文件夹的路径与占用；统计任务异常时返回空列表
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_get_local_files() -> Vec<LocalFileEntryDto> {
    tauri::async_runtime::spawn_blocking(collect)
        .await
        .unwrap_or_default()
}

/// 清理一项缓存目录，只删目录里的东西、目录本身留着
///
/// - `app`: 应用句柄，清理「界面数据」时用来取任意一个 webview
/// - `id`: 文件夹 id，取自 [`settings_get_local_files`] 的返回值
///
/// # 返回值
///
/// 清理成功返回 `Ok(())`；id 不认识、这一项不允许清理、或部分文件删不掉时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_clear_local_file(app: AppHandle, id: String) -> Result<(), String> {
    let Some(kind) = kinds().into_iter().find(|kind| kind.id == id) else {
        return Err(err_keys::LOCAL_FILE_UNKNOWN.to_string());
    };
    if !kind.deletable {
        return Err(err_keys::LOCAL_FILE_LOCKED.to_string());
    }
    if kind.id == "webview" {
        return clear_webview_data(&app);
    }
    tauri::async_runtime::spawn_blocking(move || clear(&kind))
        .await
        .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())?
}

/// 清掉 WebView2 的用户数据，即「界面数据」那一行
///
/// 浏览器数据由 WebView2 自己回收：直接删目录只会删掉没被占用的那部分，
/// 还把正在运行的 profile 拆成半套。
///
/// - `app`: 应用句柄，用来取任意一个 webview
///
/// # 返回值
///
/// 成功返回 `Ok(())`；没有窗口或运行时太旧不支持时返回错误键
fn clear_webview_data(app: &AppHandle) -> Result<(), String> {
    let Some(webview) = app.webview_windows().into_values().next() else {
        return Err(err_keys::LOCAL_FILE_CLEAR.to_string());
    };

    webview
        .clear_all_browsing_data()
        .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())
}

/// 删掉目录里的内容，跳过该行承诺保留的子目录
///
/// - `kind`: 目标文件夹
///
/// # 返回值
///
/// 全部删掉或目录本来就不存在返回 `Ok(())`；有文件删不掉返回错误键，明细进日志
fn clear(kind: &LocalFileKind) -> Result<(), String> {
    // 下载缓存与下载内容可能正被写入：先取消任务，否则文件句柄占着，删一半留一半。
    // 图片缓存与下载器无关，别顺手打断用户正在下的东西
    if kind.id == "cache" || kind.id == "download" {
        mml_downloader::cancel_all();
    }
    let keep_image = kind.dir.join(names::IMAGE_DIR);
    let keep_webview = kind.dir.join(names::WEBVIEW_DIR);

    let Ok(entries) = fs::read_dir(&kind.dir) else {
        // 目录不在就是已经干净了，没什么可删
        return Ok(());
    };

    let mut failed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        // 图片缓存与界面数据各自有归属：清理「缓存」时不能顺手动它们
        if kind.id == "cache" && (path == keep_image || path == keep_webview) {
            continue;
        }
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(err) = result {
            failed = true;
            mml_log::error(
                LogText::LocalFileClearFailed {
                    path: path.display().to_string(),
                    error: err.to_string(),
                }
                .text(),
            );
        }
    }

    if failed {
        Err(err_keys::LOCAL_FILE_CLEAR.to_string())
    } else {
        Ok(())
    }
}

/// 在资源管理器里打开一项的目录，不存在就先建出来
///
/// - `id`: 文件夹 id，取自 [`settings_get_local_files`] 的返回值
///
/// # 返回值
///
/// 成功返回 `Ok(())`；id 不认识或目录建不出来时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_open_local_file(id: String) -> Result<(), String> {
    let Some(kind) = kinds().into_iter().find(|kind| kind.id == id) else {
        return Err(err_keys::LOCAL_FILE_UNKNOWN.to_string());
    };
    if !kind.dir.exists() {
        path_helper::create_dir_all(&kind.dir).map_err(|_| err_keys::FILE_NOT_FOUND.to_string())?;
    }
    open_helper::open_dir(&kind.dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 清理只删内容：目录本身、图片缓存与界面数据都留着
    #[test]
    fn clear_removes_children_but_keeps_reserved_dirs() {
        let dir = mml_testutil::temp_dir().join(format!(
            "local_files_clear_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let cache = LocalFileKind {
            id: "cache",
            dir: dir.clone(),
            deletable: true,
        };
        fs::create_dir_all(dir.join(names::IMAGE_DIR)).unwrap();
        fs::create_dir_all(dir.join(names::WEBVIEW_DIR)).unwrap();
        fs::create_dir_all(dir.join(names::TEMP_DIR)).unwrap();
        fs::write(dir.join("a.zip"), b"x").unwrap();
        fs::write(dir.join(names::IMAGE_DIR).join("i.png"), b"x").unwrap();
        fs::write(dir.join(names::WEBVIEW_DIR).join("db"), b"x").unwrap();
        fs::write(dir.join(names::TEMP_DIR).join("t"), b"x").unwrap();

        clear(&cache).unwrap();

        assert!(dir.exists());
        assert!(dir.join(names::IMAGE_DIR).join("i.png").exists());
        assert!(dir.join(names::WEBVIEW_DIR).join("db").exists());
        assert!(!dir.join("a.zip").exists());
        assert!(!dir.join(names::TEMP_DIR).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// 「缓存」的占用不含图片缓存与界面数据，否则几行数字加起来会重复计算
    #[test]
    fn cache_size_excludes_reserved_children() {
        let dir = mml_testutil::temp_dir().join(format!(
            "local_files_size_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let cache = LocalFileKind {
            id: "cache",
            dir: dir.clone(),
            deletable: true,
        };
        fs::create_dir_all(dir.join(names::IMAGE_DIR)).unwrap();
        fs::create_dir_all(dir.join(names::WEBVIEW_DIR)).unwrap();
        fs::write(dir.join("a.zip"), vec![0u8; 300]).unwrap();
        fs::write(dir.join(names::IMAGE_DIR).join("i.png"), vec![0u8; 100]).unwrap();
        fs::write(dir.join(names::WEBVIEW_DIR).join("db"), vec![0u8; 200]).unwrap();

        assert_eq!(size_of(&cache), 300);

        fs::remove_dir_all(&dir).unwrap();
    }
}
