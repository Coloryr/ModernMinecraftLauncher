//! 运行时环境与资讯
//!
//! Java、系统内存、可装版本与新闻都来自内核与本机探测；
//! 只读，失败时给出空列表而不是报错，界面照常显示。



use crate::dtos::main_dto::{CustomJsonItemDto, NewsItem, StartArgDto};
use crate::dtos::{JavaInfoDto, MotdDto, SystemMemoryDto, VersionInfoDto};
use crate::image_manager;

use crate::windows::main::args::{core_instance};
use crate::windows::main::catalog::{VERSIONS_CACHE, fetch_versions, parse_motd_addr};
use super::instance::{java_list};


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

/// 取走启动参数带来的待办（命令行 / `mml:` 协议）
///
/// 前端初始化完成后调一次：启动瞬间内核实例表还没加载，参数只能先存着等，
/// 见 [`crate::start_arg`]。
///
/// # 返回值
///
/// 有启动参数时返回它，否则 `None`；取过一次就没了
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_take_start_arg() -> Option<StartArgDto> {
    crate::start_arg::take()
}

/// 读取实例的自定义启动配置，即 `json/` 目录下每个文件的摘要
///
/// 对应 ColorMC 游戏编辑窗口 tab1 的那张表；按加载顺序排序，顺序相同再按文件名，
/// 这样界面上的顺序与内核拼启动参数时的取用顺序一致。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 配置摘要列表；目录不存在时为空列表。uuid 非法或实例不存在返回错误文案
#[gui_macros::ipc_group("main")]
#[tauri::command]
pub fn main_get_custom_json(uuid: String) -> Result<Vec<CustomJsonItemDto>, String> {
    let core = core_instance(&uuid).ok_or_else(|| crate::err_keys::GAME_NOT_FOUND.to_string())?;
    let inst = core.instance.read().unwrap();
    let mut list: Vec<CustomJsonItemDto> = inst
        .read_custom_json()
        .into_iter()
        .map(|(file, obj)| CustomJsonItemDto {
            file,
            name: obj.name,
            uid: obj.uid,
            version: obj.version,
            order: obj.order,
        })
        .collect();
    list.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.file.cmp(&b.file)));
    Ok(list)
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
