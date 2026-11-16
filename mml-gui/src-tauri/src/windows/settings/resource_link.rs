//! 设置窗口：资源映射的存量处理

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use mml_link::IndexKind;
use mml_link::info::{ResourceInfoObj, ResourceType};
use mml_sys::open_helper;
use tauri::{AppHandle, Emitter};

use crate::dtos::{
    LinkResourceDto, LinkResourcePageDto, LinkResourceQueryDto, ResourceMapProgressDto,
    ResourceMapResultDto,
};
use crate::err_keys;
use crate::listens;
use crate::log_text::LogText;

/// 存量映射是否正在处理
static RUNNING: AtomicBool = AtomicBool::new(false);

/// 存量映射正在处理的守卫，命令退出或 panic 时自动复位
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

/// 存量映射是否正在处理
///
/// # 返回值
///
/// 正在处理时返回 `true`
pub(crate) fn running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// 批量资源映射进度事件
///
/// - `app`: 应用句柄
/// - `dto`: 当前进度
#[gui_macros::emit]
pub fn emit_resource_map_progress(app: &AppHandle, dto: ResourceMapProgressDto) {
    let _ = app.emit(listens::RESOURCE_MAP_PROGRESS, dto);
}

/// 把所有实例里带在线信息的模组 / 资源包搬进公共目录并补硬链接
///
/// 只处理实例里实际存在、带 SHA1 的文件；禁用的文件和缺失的文件跳过。
///
/// # 返回值
///
/// 见 ResourceMapResultDto；资源映射未开启或已有任务在跑时返回错误键
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_map_resources(app: AppHandle) -> Result<ResourceMapResultDto, String> {
    if !mml_link::is_enabled() {
        return Err(err_keys::RESOURCE_LINK_OFF.to_string());
    }
    // 游戏运行时模组文件可能正被读取，搬动它们既慢又容易失败
    if !mml_game::get_running_instances().is_empty() {
        return Err(err_keys::INSTANCE_RUNNING.to_string());
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(err_keys::RESOURCE_MAP_RUNNING.to_string());
    }
    // 守卫先建：统计或起线程前 panic / 命令 future 被丢掉也能复位
    let guard = RunningGuard;

    let games = mml_game::get_instances();
    let instances = games.len() as u32;
    let total: u32 = games
        .iter()
        .map(mml_game::game_link::count_instance_resources)
        .sum();
    mml_log::info(LogText::ResourceMapStarted { instances, total }.text());
    emit_resource_map_progress(
        &app,
        ResourceMapProgressDto {
            instances,
            total,
            done: 0,
        },
    );

    let progress_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        // 守卫随闭包进入阻塞线程：命令 future 被丢掉时仍会一直挡着退出，直到文件搬完
        let _guard = guard;
        let mut result = ResourceMapResultDto {
            instances,
            ..Default::default()
        };
        let mut done = 0u32;
        for game in &games {
            let count = mml_game::game_link::map_instance_resources_with(game, |info| {
                done += 1;
                mml_log::info(
                    LogText::ResourceMapItem {
                        name: info.name.clone(),
                        done,
                        total,
                    }
                    .text(),
                );
                emit_resource_map_progress(
                    &progress_app,
                    ResourceMapProgressDto {
                        instances,
                        total,
                        done,
                    },
                );
            });
            result.mapped += count.mapped;
            result.failed += count.failed;
            result.skipped += count.skipped;
            // 一个实例处理完落一次盘：被强杀时最多丢这一个实例的索引记录
            mml_link::flush_pending();
        }
        mml_log::info(
            LogText::ResourceMapFinished {
                mapped: result.mapped,
                failed: result.failed,
                skipped: result.skipped,
            }
            .text(),
        );
        result
    })
    .await;
    if result.is_err() {
        RUNNING.store(false, Ordering::SeqCst);
    }
    result.map_err(|err| err.to_string())
}

/// 存量映射是否正在处理，供前端决定关窗提示
///
/// # 返回值
///
/// 正在处理时返回 `true`
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_resource_map_running() -> bool {
    running()
}

/// 每页条数的默认值，单位条
const PAGE_SIZE_DEFAULT: u32 = 20;
/// 每页条数的上限，单位条
const PAGE_SIZE_MAX: u32 = 200;

/// 分页列出公共目录里已下载的资源
///
/// 读的是索引库里的记录；文件已被删掉的也列出来，标 `exists = false`。
/// 同一项目 ID 的多份文件排在一起，方便按项目核对。
///
/// - `query`: 搜索词与分页参数
///
/// # 返回值
///
/// 见 [`LinkResourcePageDto`]；索引没打开时为空页
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub async fn settings_list_link_resources(query: LinkResourceQueryDto) -> LinkResourcePageDto {
    tauri::async_runtime::spawn_blocking(move || list_link_resources(&query))
        .await
        .unwrap_or_default()
}

/// 按查询条件收集一页资源记录
///
/// - `query`: 搜索词与分页参数
///
/// # 返回值
///
/// 见 [`LinkResourcePageDto`]
fn list_link_resources(query: &LinkResourceQueryDto) -> LinkResourcePageDto {
    let page_size = if query.page_size == 0 {
        PAGE_SIZE_DEFAULT
    } else {
        query.page_size.min(PAGE_SIZE_MAX)
    };
    let keyword = query.keyword.trim().to_lowercase();
    let mut list = collect_link_resources();
    if !keyword.is_empty() {
        list.retain(|item| {
            item.pid.to_lowercase().contains(&keyword)
                || item.name.to_lowercase().contains(&keyword)
                || item.file.to_lowercase().contains(&keyword)
                || item.hash.to_lowercase().contains(&keyword)
        });
    }
    // 同一个项目的多个版本挨在一起，组内大的排前面
    list.sort_by(|a, b| {
        cmp_pid(&a.pid, &b.pid)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.name.cmp(&b.name))
    });
    let total_size = list
        .iter()
        .filter(|item| item.exists)
        .map(|item| item.size)
        .sum();
    let (page, pages, start, end) = page_bounds(list.len(), page_size as usize, query.page);
    LinkResourcePageDto {
        total: list.len() as u64,
        total_size,
        page,
        pages,
        page_size,
        items: list[start..end].to_vec(),
    }
}

/// 读出索引里的全部资源记录并补上磁盘上的大小
///
/// # 返回值
///
/// 未排序的记录；索引没打开时为空列表
fn collect_link_resources() -> Vec<LinkResourceDto> {
    let Some(store) = mml_link::index_store() else {
        return Vec::new();
    };
    let mut list = Vec::new();
    for (kind, resource, id) in [
        (IndexKind::Mod, ResourceType::Mod, "mod"),
        (IndexKind::ResourcePack, ResourceType::ResourcePack, "resourcepack"),
    ] {
        let dir = match resource {
            ResourceType::Mod => mml_link::get_mod_download_path(),
            ResourceType::ResourcePack => mml_link::get_resource_pack_download_path(),
        };
        // 先扫目录再逐条查：几千条记录逐个 stat 太慢
        let sizes = dir_file_sizes(&dir);
        let Ok(rows) = store.load(kind) else {
            continue;
        };
        for (pid, payload) in rows {
            let Ok(info) = serde_json::from_slice::<ResourceInfoObj>(&payload) else {
                continue;
            };
            let size = sizes.get(&info.file).copied();
            list.push(LinkResourceDto {
                kind: id.to_string(),
                pid,
                name: info.name,
                file: info.file,
                hash: info.hash,
                size: size.unwrap_or(0),
                exists: size.is_some(),
            });
        }
    }
    list
}

/// 列出一个目录下各文件的大小
///
/// - `dir`: 公共下载目录
///
/// # 返回值
///
/// 文件名 → 字节数；目录读不到时为空
fn dir_file_sizes(dir: &Path) -> HashMap<String, u64> {
    let mut sizes = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return sizes;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_file() {
            sizes.insert(entry.file_name().to_string_lossy().into_owned(), meta.len());
        }
    }
    sizes
}

/// 按条数分页
///
/// - `total`: 记录总数，单位条
/// - `page_size`: 每页条数，单位条
/// - `page`: 请求的页码，从 1 开始
///
/// # 返回值
///
/// `(实际页码, 总页数, 起始下标, 结束下标)`；请求超出末页时返回末页
fn page_bounds(total: usize, page_size: usize, page: u32) -> (u32, u32, usize, usize) {
    let page_size = page_size.max(1);
    let pages = total.div_ceil(page_size).max(1) as u32;
    let page = page.clamp(1, pages);
    let start = (page as usize - 1) * page_size;
    let end = (start + page_size).min(total);
    (page, pages, start, end)
}

/// 项目 ID 排序：两边都是数字时按数值比，否则按字符串比
fn cmp_pid(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => a.cmp(b),
    }
}

/// 在资源管理器里定位一份已下载资源
///
/// - `kind`: 资源类型，`mod` / `resourcepack`
/// - `file`: 公共目录下的文件名，取自 [`settings_list_link_resources`]
///
/// # 返回值
///
/// 类型不认识时返回错误键；文件不在时退而打开所在目录
#[gui_macros::ipc_group("settings")]
#[gui_macros::command]
pub fn settings_open_link_resource(kind: String, file: String) -> Result<(), String> {
    let dir = match kind.as_str() {
        "mod" => mml_link::get_mod_download_path(),
        "resourcepack" => mml_link::get_resource_pack_download_path(),
        _ => return Err(err_keys::LOCAL_FILE_UNKNOWN.to_string()),
    };
    let path = dir.join(&file);
    if path.is_file() {
        open_helper::open_file_with_explorer(&path);
    } else {
        open_helper::open_dir(&dir);
    }
    Ok(())
}
