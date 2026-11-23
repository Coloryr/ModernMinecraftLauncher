//! 设置窗口「本地文件」标签：统计运行目录下各文件夹的占用，并清理缓存类目录
//!
//! 目录来源一律走各自的访问器，不在这里另拼路径：下载缓存 / 下载内容在 mml-downloader，
//! 图标缓存在 image_manager，实例目录在 mml-game，Java 在 mml-jvms。
//! 命令带分组标记，值填原始组名 `settings`，免得组键随文件名变掉，见 AGENTS.md §4。

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

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
/// 图片缓存与界面数据都是「缓存」目录的子目录，因此不再单独成行：它们作为缓存的明细出现，
/// 缓存的占用按明细求和，几行的数字加起来不会比磁盘实际占用大。
///
/// # 返回值
///
/// 四个文件夹的 id / 路径 / 可清理性
fn kinds() -> Vec<LocalFileKind> {
    vec![
        LocalFileKind {
            id: "cache",
            dir: mml_downloader::get_cache_path(),
            deletable: true,
        },
        LocalFileKind {
            id: "download",
            dir: mml_downloader::get_download_path(),
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
    if kind.id == "cache" {
        return cache_own_size(&kind.dir);
    }
    path_helper::get_folder_size(&kind.dir)
}

/// 缓存目录里除图片缓存与界面数据之外的占用，单位字节
///
/// - `dir`: 缓存目录
///
/// # 返回值
///
/// 扣掉两个子目录后的字节数
fn cache_own_size(dir: &Path) -> u64 {
    path_helper::get_folder_size(dir)
        .saturating_sub(path_helper::get_folder_size(dir.join(names::IMAGE_DIR)))
        .saturating_sub(path_helper::get_folder_size(dir.join(names::WEBVIEW_DIR)))
}

/// 采集全部文件夹的占用
///
/// # 返回值
///
/// 与 [`kinds`] 同序的 DTO 列表
fn collect() -> Vec<LocalFileEntryDto> {
    let list: Vec<LocalFileEntryDto> = kinds()
        .into_iter()
        .map(|kind| {
            let children = match kind.id {
                "instances" => instance_sizes(),
                "download" => download_sizes(),
                "cache" => cache_sizes(),
                "java" => java_sizes(),
                _ => Vec::new(),
            };
            // 带明细的行按明细求和：避免为一个总数再扫一遍目录，也保证明细之和等于总数
            let size = if matches!(kind.id, "instances" | "cache" | "java") {
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
        .collect();
    list
}

/// 实例里已算进 `download` 的那部分文件，按实例根目录的相对路径
///
/// 只认 `is_link` 的记录：跨盘回退成复制的那些数据仍然只在实例里，大小要算给实例。
/// 用映射记录代替逐文件取身份 —— 取身份要开一次文件句柄，十几万个文件走一遍就是几分钟，
/// 而记录里本来就写着哪些文件是硬链接
///
/// - `uuid`: 实例 uuid
/// - `base`: 实例根目录
///
/// # 返回值
///
/// 相对路径集合，统一用 `/` 分隔；没有映射记录时为空
fn linked_paths(uuid: uuid::Uuid, base: &Path) -> HashSet<String> {
    mml_link::get_instance_link_data(uuid, base)
        .into_iter()
        .filter(|item| item.is_link)
        .map(|item| item.path.replace('\\', "/"))
        .collect()
}

/// 统计目录大小，跳过已计入别处的文件
///
/// - `base`: 实例根目录，用来算相对路径
/// - `dir`: 当前遍历的目录
/// - `linked`: 已计入 `download` 的相对路径
///
/// # 返回值
///
/// 未重复文件的字节数
fn size_excluding(base: &Path, dir: &Path, linked: &HashSet<String>) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            total += size_excluding(base, &entry.path(), linked);
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .map(|item| item.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if linked.contains(&relative) {
            continue;
        }
        total += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
    }
    total
}

/// 各游戏实例的占用，按大小降序
///
/// # 返回值
///
/// 每个实例的 uuid / 名字 / 根目录与独占字节数；没有实例时为空列表
fn instance_sizes() -> Vec<LocalFileChildDto> {
    let mut list: Vec<LocalFileChildDto> = mml_game::get_instances()
        .into_iter()
        .map(|instance| {
            // 扫目录可能很慢：先把字段取出来放锁，别攥着实例读锁做磁盘遍历
            let (uuid, name, path) = {
                let instance = instance.read().unwrap();
                (
                    instance.uuid,
                    instance.name.clone(),
                    instance.get_base_path(),
                )
            };
            // 实例里的模组是 download 的硬链接，按映射记录把这部分排除，否则总计会算两遍
            let linked = linked_paths(uuid, &path);
            LocalFileChildDto {
                id: uuid.to_string(),
                kind: String::from("instance"),
                name,
                size: size_excluding(&path, &path, &linked),
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

/// 缓存这一行的明细：其余缓存、图片缓存、界面数据
///
/// 三者加起来就是缓存目录的完整占用（缓存本体扣掉了另外两个子目录）。
///
/// # 返回值
///
/// 三条明细，顺序固定
fn cache_sizes() -> Vec<LocalFileChildDto> {
    let cache = mml_downloader::get_cache_path();
    let image = crate::image_manager::icon_cache_dir();
    let webview = cache.join(names::WEBVIEW_DIR);
    [
        (String::from("cacheOther"), cache.clone(), cache_own_size(&cache)),
        (
            String::from("image"),
            image.clone(),
            path_helper::get_folder_size(&image),
        ),
        (
            String::from("webview"),
            webview.clone(),
            path_helper::get_folder_size(&webview),
        ),
    ]
    .into_iter()
    .map(|(kind, path, size)| LocalFileChildDto {
        id: kind.clone(),
        kind,
        name: String::new(),
        size,
        path: path.to_string_lossy().to_string(),
    })
    .collect()
}

/// Java 运行时目录下每个运行时的占用，按大小降序
///
/// 只统计启动器自己下载 / 解压出来的运行时目录；系统里已有的 Java 不计入。
///
/// # 返回值
///
/// 每个运行时一条明细；目录不存在时为空列表
fn java_sizes() -> Vec<LocalFileChildDto> {
    let dir = mml_jvms::get_java_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut list: Vec<LocalFileChildDto> = entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            LocalFileChildDto {
                id: name.clone(),
                kind: String::from("java"),
                name,
                size: path_helper::get_folder_size(&path),
                path: path.to_string_lossy().to_string(),
            }
        })
        .collect();
    list.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    list
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
/// 「缓存」这一行的图片缓存与界面数据都在它下面，所以先删目录内容，再走浏览数据接口清界面数据。
///
/// - `app`: 应用句柄，清理「缓存」时用来取任意一个 webview
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
    if kind.id == "cache" {
        tauri::async_runtime::spawn_blocking(move || clear(&kind))
            .await
            .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())??;
        return clear_webview_data(&app);
    }
    tauri::async_runtime::spawn_blocking(move || clear(&kind))
        .await
        .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())?
}

/// 清掉 WebView2 的用户数据，即「缓存 › 界面数据」那一条明细
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
    let keep = if kind.id == "cache" {
        // 界面数据删不得：WebView2 一直占着这个目录，清理「缓存」后另走浏览数据接口
        vec![kind.dir.join(names::WEBVIEW_DIR)]
    } else {
        Vec::new()
    };
    clear_contents(&kind.dir, &keep)
}

/// 清空目录里的内容，跳过 `keep` 里的路径
///
/// - `dir`: 目标目录
/// - `keep`: 要保留的绝对路径
///
/// # 返回值
///
/// 全部删掉或目录本来就不存在返回 `Ok(())`；有文件删不掉返回错误键，明细进日志
fn clear_contents(dir: &Path, keep: &[PathBuf]) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        // 目录不在就是已经干净了，没什么可删
        return Ok(());
    };

    let mut failed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if keep.contains(&path) {
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

/// 解析一条明细对应的目录
///
/// 只认本页登记的条目与它自己的子项，**不接受前端传路径**：缓存与下载内容按固定子目录解析，
/// Java 按运行时目录名匹配，游戏实例按 uuid 查实例根目录。
///
/// - `entry`: 行 id，见 [`settings_get_local_files`]
/// - `child`: 明细 id，见同行返回的 `children`
///
/// # 返回值
///
/// 明细目录；id 不认识或目录查不到时返回错误键
fn child_dir(entry: &str, child: &str) -> Result<PathBuf, String> {
    let cache = mml_downloader::get_cache_path();
    match (entry, child) {
        ("cache", "cacheOther") => Ok(cache),
        ("cache", "image") => Ok(crate::image_manager::icon_cache_dir()),
        ("cache", "webview") => Ok(cache.join(names::WEBVIEW_DIR)),
        ("download", "mods") => Ok(mml_downloader::get_download_path().join(names::GAME_MODS_DIR)),
        ("download", "resourcepacks") => {
            Ok(mml_downloader::get_download_path().join(names::GAME_RESOURCEPACKS_DIR))
        }
        ("java", name) => java_child_dir(name),
        ("instances", uuid) => instance_child_dir(uuid),
        _ => Err(err_keys::LOCAL_FILE_UNKNOWN.to_string()),
    }
}

/// 按运行时目录名找 Java 运行时的目录
///
/// 逐个比对 `java/` 下的子目录，而不是直接拼路径，免得名字里带分隔符时跳出目录。
///
/// - `name`: 运行时目录名
///
/// # 返回值
///
/// 运行时目录；找不到时返回错误键
fn java_child_dir(name: &str) -> Result<PathBuf, String> {
    let Ok(entries) = fs::read_dir(mml_jvms::get_java_dir()) else {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    };
    entries
        .flatten()
        .find(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_dir())
                .unwrap_or(false)
                && entry.file_name().to_string_lossy() == name
        })
        .map(|entry| entry.path())
        .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())
}

/// 按实例 uuid 取实例根目录
///
/// - `uuid`: 实例 uuid，取自明细 id
///
/// # 返回值
///
/// 实例根目录；uuid 非法或实例已不在时返回错误键
fn instance_child_dir(uuid: &str) -> Result<PathBuf, String> {
    let id = uuid::Uuid::parse_str(uuid).map_err(|_| err_keys::LOCAL_FILE_UNKNOWN.to_string())?;
    mml_game::get_instance(&id)
        .map(|instance| instance.read().unwrap().get_base_path())
        .ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())
}

/// 在资源管理器里打开一条明细的目录
///
/// - `entry`: 行 id，见 [`settings_get_local_files`]
/// - `child`: 明细 id
///
/// # 返回值
///
/// 成功返回 `Ok(())`；id 不认识、目录不存在且不该凭空建时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_open_local_child(entry: String, child: String) -> Result<(), String> {
    let dir = child_dir(&entry, &child)?;
    if !dir.exists() {
        // 缓存与下载内容下的子目录可能还没建出来；实例与 Java 不凭空造目录
        if matches!(entry.as_str(), "cache" | "download") {
            path_helper::create_dir_all(&dir).map_err(|_| err_keys::FILE_NOT_FOUND.to_string())?;
        } else {
            return Err(err_keys::FILE_NOT_FOUND.to_string());
        }
    }
    open_helper::open_dir(&dir);
    Ok(())
}

/// 删除一条明细
///
/// 缓存类清空目录内容（界面数据走浏览数据接口），Java 运行时连目录一起删；
/// 游戏实例不走这里 —— 删除实例要挪回收站、关窗口、广播变更，走主窗口的删除命令。
///
/// - `app`: 应用句柄，清界面数据时用来取任意一个 webview
/// - `entry`: 行 id，见 [`settings_get_local_files`]
/// - `child`: 明细 id
///
/// # 返回值
///
/// 删除成功返回 `Ok(())`；id 不认识、这一项不支持在这里删、或文件删不掉时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_clear_local_child(
    app: AppHandle,
    entry: String,
    child: String,
) -> Result<(), String> {
    let dir = child_dir(&entry, &child)?;
    match (entry.as_str(), child.as_str()) {
        // 界面数据由 WebView2 自己回收，删目录会把正在运行的 profile 拆坏
        ("cache", "webview") => clear_webview_data(&app),
        ("cache", "cacheOther") => {
            let keep = [dir.join(names::IMAGE_DIR), dir.join(names::WEBVIEW_DIR)];
            tauri::async_runtime::spawn_blocking(move || clear_contents(&dir, &keep))
                .await
                .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())?
        }
        ("cache", _) => tauri::async_runtime::spawn_blocking(move || clear_contents(&dir, &[]))
            .await
            .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())?,
        ("download", _) => {
            mml_downloader::cancel_all();
            tauri::async_runtime::spawn_blocking(move || clear_contents(&dir, &[]))
                .await
                .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())?
        }
        // Java 运行时是整个目录一起删
        ("java", _) => tauri::async_runtime::spawn_blocking(move || remove_dir(&dir))
            .await
            .map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())?,
        _ => Err(err_keys::LOCAL_FILE_UNKNOWN.to_string()),
    }
}

/// 连目录一起删掉
///
/// - `dir`: 目标目录
///
/// # 返回值
///
/// 目录不在也算成功；删不掉时返回错误键
fn remove_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(dir).map_err(|_| err_keys::LOCAL_FILE_CLEAR.to_string())
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
#[path = "../../../tests/unit/windows_settings_local_files_tests.rs"]
mod tests;
