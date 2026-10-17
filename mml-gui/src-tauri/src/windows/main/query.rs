//! 主窗口的查询类命令：实例 / 分组 / 参数 / Java / 内存 / 版本 / 新闻 / MOTD
//!
//! 从 `main/mod.rs` 拆出来的。命令带分组标记，值填原始组名 main，把组键钉回
//! `main` —— 否则组键会跟着文件名变成 `query`，前端 `commands.main.*` 全断，见 AGENTS.md §4。

use tauri::WebviewWindow;
use uuid::Uuid;

use mml_game::launcher::LogEncoding;

use crate::dtos::main_dto::{LoadState, NewsItem};
use crate::dtos::{
    GroupDto, InstanceArgsDto, InstanceInfoDto, InstanceLangDto, JavaInfoDto, MotdDto,
    SystemMemoryDto, VersionInfoDto,
};
use crate::image_manager;

use super::args::{apply_args_to_core, args_from_core, core_instance};
use super::catalog::{VERSIONS_CACHE, fetch_versions, parse_motd_addr};
use super::model;

/// 获取实例列表，对接 mml-game::get_instances 并合并运行状态
///
/// # 返回值
///
/// 全部核心实例的 DTO 列表，分组归属与组内次序取自内核分组表
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_get_instances() -> Vec<InstanceInfoDto> {
    mml_game::get_instances()
        .into_iter()
        .map(|inst| {
            let inst = inst.read().unwrap();
            InstanceInfoDto {
                uuid: inst.uuid.to_string(),
                name: inst.name.clone(),
                // 分组归属与组内次序都在 mml-game 的分组表 group_save.json 里：
                // 实例配置 game.json 与 gui_setting.json 都没有这两个字段了
                group: mml_game::get_instance_group(&inst.uuid).map(|g| g.to_string()),
                version: inst.version.clone(),
                version_type: Some(inst.game_type.id().to_string()),
                loader: String::from(inst.loader.to_string()),
                loader_version: inst.loader_version.clone(),
                dir: inst.dir.clone(),
                running: mml_game::is_running(&inst.uuid),
                modpack_type: inst.is_modpack.then(|| inst.modpack_type.to_string()),
                pid: inst.pid.clone(),
                fid: inst.fid.clone(),
                server_url: inst.server_url.clone(),
                lang: None,
                log_encoding: Some(
                    if matches!(inst.encoding, LogEncoding::GBK) {
                        "gbk"
                    } else {
                        "utf8"
                    }
                    .to_string(),
                ),
                source: None,
                // 组内次序 = 分组表里的 `order`，键为实例 uuid；不在任何分组里时排到最后
                order: mml_game::get_instance_order(&inst.uuid).unwrap_or(i32::MAX),
            }
        })
        .collect()
}

/// 获取分组列表，含空分组
///
/// 返回 uuid + 组名：分组以 uuid 为身份、组名只是显示数据，
/// 前端要用 uuid 回传，用于移动实例 / 删组 / 调序。
///
/// # 返回值
///
/// 按分组表顺序排列的分组列表，含没有实例的空分组
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_get_groups() -> Vec<GroupDto> {
    mml_game::get_group_list()
        .into_iter()
        .map(|g| GroupDto {
            uuid: g.uuid.to_string(),
            name: g.name,
        })
        .collect()
}

/// 获取实例的游戏内语言列表，从资源索引查 minecraft/lang/*.json，资源未下载时为空
///
/// 每项带显示名：名字取自对应语言文件里的 `language.name`，如 zh_cn → 简体中文，
/// 读不到时回落为语言代码，见 `mml_game::get_instance_langs`。
///
/// 查询要读整份资源索引 JSON、再逐个读语言文件，是同步重活：丢到阻塞线程执行，
/// 不要占住 async runtime —— 否则同期的其它 IPC 与图片协议请求会被一起拖住。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 语言项列表，含代码与显示名；uuid 非法或资源未下载时为空列表
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub async fn main_get_instance_langs(uuid: String) -> Vec<InstanceLangDto> {
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Ok(id) = Uuid::parse_str(&uuid) else {
            return Vec::new();
        };
        mml_game::get_instance_langs(&id)
            .into_iter()
            .map(|item| InstanceLangDto {
                code: item.code,
                name: item.name,
            })
            .collect()
    })
    .await;

    res.unwrap_or_default()
}

/// 获取实例启动参数，核心实例读配置，遗留数据读内存缓存
///
/// - `window`: 调用方窗口，用于取本窗口模型里的遗留参数
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 启动参数 DTO；两处都取不到时返回默认值
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_get_instance_args(window: WebviewWindow, uuid: String) -> InstanceArgsDto {
    if let Some(core) = core_instance(&uuid) {
        let obj = core.instance.read().unwrap();
        return args_from_core(&obj);
    }
    if let Ok(store) = model(&window)
        && let Some(a) = store.lock().unwrap().args.get(&uuid)
    {
        return a.clone();
    }
    InstanceArgsDto::default()
}

/// 更新实例启动参数，核心实例写配置并保存，遗留数据只更新内存缓存
///
/// - `window`: 调用方窗口，用于取本窗口模型存遗留参数
/// - `uuid`: 实例 uuid
/// - `args`: 前端提交的启动参数，整份覆盖
///
/// # 返回值
///
/// 写入成功返回 `true`；模型缺失返回 `Err`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_update_instance_args(
    window: WebviewWindow,
    uuid: String,
    args: InstanceArgsDto,
) -> Result<bool, String> {
    if let Some(core) = core_instance(&uuid) {
        {
            let mut obj = core.instance.write().unwrap();
            apply_args_to_core(&mut obj, &args);
            obj.save();
        }
        // 这里**不发** instance-change：启动参数不进实例列表，也不需要刷新图标。
        // 发了的后果是每改一次设置，图标就换 URL 重拉一次、列表也白刷一遍
        return Ok(true);
    }
    let store = model(&window)?;
    store.lock().unwrap().args.insert(uuid, args);
    Ok(true)
}

/// 获取 Java 列表，来自 mml_jvms，配置加载 / 扫描异步进行
///
/// # 返回值
///
/// Java 项列表；配置尚未加载完成时为空
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_get_java_list() -> Vec<JavaInfoDto> {
    java_list()
}

/// 获取本机内存，单位 MiB，供启动参数里的内存设置显示参考值
///
/// 前端拿不到物理内存，只能问后端；查询失败时核心返回 `u64::MAX` 哨兵值，
/// 这里统一折算成 0，由前端决定不显示。
///
/// # 返回值
///
/// 总内存与可用内存，单位 MiB；取不到时为 0
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_get_system_memory() -> SystemMemoryDto {
    let total = mml_sys::memory_helper::get_memory_size();
    let free = mml_sys::memory_helper::get_memory_free();

    SystemMemoryDto {
        total: if total == u64::MAX { 0 } else { total },
        free: if free == u64::MAX { 0 } else { free },
    }
}

/// 获取游戏版本列表，从 mml-core 拉取版本清单，缓存于进程
///
/// # 返回值
///
/// 版本列表，缓存命中直接返回；拉取失败时为空列表，不返回 `Err`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub async fn main_get_versions() -> Result<Vec<VersionInfoDto>, String> {
    {
        let cache = VERSIONS_CACHE.read().unwrap();
        if !cache.is_empty() {
            return Ok(cache.clone());
        }
    }
    let fetched = fetch_versions().await;
    *VERSIONS_CACHE.write().unwrap() = fetched.clone();
    Ok(fetched)
}

/// 强制刷新版本列表，清空缓存后重新从版本清单拉取
///
/// # 返回值
///
/// 重新拉取到的版本列表；失败时为空列表
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub async fn main_refresh_versions() -> Result<Vec<VersionInfoDto>, String> {
    VERSIONS_CACHE.write().unwrap().clear();
    main_get_versions().await
}

/// 获取 Minecraft 官方新闻，走 Mojang 新闻接口按页拉取，核心加载完成后可调用
///
/// - `page`: 页码，从 1 起；缺省为 1
///
/// # 返回值
///
/// 该页的新闻条目，id 为页内序号，图片与原文链接已补成绝对地址；接口失败返回 `Err`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub async fn main_get_news(page: Option<u32>) -> Result<Vec<NewsItem>, String> {
    let page = page.unwrap_or(1);
    let news = mml_net::mojang_api::get_minecraft_news(page)
        .await
        .map_err(|err| err.to_string())?;
    Ok(news
        .article_grid
        .into_iter()
        .enumerate()
        .map(|(i, grid)| {
            // 相对路径补全域名，否则 webview 加载不到图片
            let full = |path: String| {
                if path.starts_with("http") {
                    path
                } else {
                    format!("https://www.minecraft.net{path}")
                }
            };
            NewsItem {
                id: i as i64,
                title: grid.default_tile.title,
                date: grid.default_tile.sub_header,
                tag: grid.primary_category,
                image: full(grid.default_tile.image.image_url),
                url: full(grid.article_url),
            }
        })
        .collect())
}

/// 用系统浏览器打开网址，如新闻原文跳转
///
/// - `url`: 要打开的完整网址
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_open_url(url: String) {
    mml_sys::open_helper::open_url(&url);
}

/// `mml-image` 协议访问前缀，前端拼实例图标等地址用
///
/// # 返回值
///
/// 自定义图片协议的基础地址
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_image_base_url() -> String {
    image_manager::image_base_url().to_string()
}

/// `mml-music` 协议访问前缀，前端拼自定义音乐地址用
///
/// # 返回值
///
/// 自定义音乐协议的基础地址
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_music_base_url() -> String {
    crate::music::music_base_url().to_string()
}

/// 核心加载状态，前端兜底：启动太快时 load-done 事件会先于页面监听发出而被错过
///
/// # 返回值
///
/// 加载状态：`ok` 表示核心是否已就绪，`error` 恒为 `None`，失败详情走事件
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_load_state() -> LoadState {
    LoadState {
        ok: mml_core::get_state(),
        error: None,
    }
}

/// 查询服务器 MOTD
///
/// - `address`: 服务器地址，可写 host 或 host:port，端口缺省 25565
///
/// # 返回值
///
/// MOTD 查询结果；连不上或无数据时 `state` 为对应失败态，不返回 `Err`
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub async fn main_get_motd(address: String) -> MotdDto {
    let addr = parse_motd_addr(&address);
    mml_game::game_motd::get_server_info(&addr.host, addr.port)
        .await
        .into()
}

/// 从 mml_jvms 读取 Java 列表，配置加载 / 扫描异步进行，未完成时为空
///
/// # 返回值
///
/// Java 项列表；主版本号未知时为 0
fn java_list() -> Vec<JavaInfoDto> {
    mml_jvms::get_all_java()
        .iter()
        .map(|j| JavaInfoDto {
            name: j.name.clone(),
            path: j.path.to_string_lossy().to_string(),
            version: j.version.clone(),
            // 遗留占位条目即 Java 失效的，主版本号为 -1，前端用 0 表示未知
            major: j.major_version.max(0),
            java_type: j.java_type.clone(),
            arch: j.arch.to_string(),
        })
        .collect()
}
