//! 方块 / 物品贴图离线渲染模块
//!
//! 从 Minecraft 客户端 jar 提取模型与贴图，离线渲染出创造模式物品栏
//! 使用的方块图标与物品图标 PNG，前者为等轴测 3D、后者为平面精灵，
//! 并提供图标路径、创造分组、语言翻译等查询接口。
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`block`] | 方块图标渲染，等轴测 3D |
//! | [`gpu`] | GPU 渲染支持 |
//! | [`item`] | 物品图标渲染，平面精灵 |
//! | [`model`] | 游戏模型解析 |
//! | [`order`] | 创造分类顺序与列表排序键 |

use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        LazyLock, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use mml_base::{
    archives::BaseArchive,
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_game::{
    gui_hook,
    launcher_path::{assets_path, libraries_path, version_path},
    mojang::{assets_obj::AssetsObj, game_arg_obj::GameArgObj},
};
use mml_names::{
    Lang,
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType, PathNotExistsData},
    names,
};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;

use crate::block::obj::{BlocksObj, ItemsObj};

pub mod block;
pub mod gpu;
pub mod item;
pub mod model;
pub mod order;

/// 渲染防重入标志：load_blocks 可能被并发触发，界面按钮连点或界面与内核各起一轮，
/// 不挡住的话会各自下发一次客户端 jar 下载任务，同一文件下两遍
static LOADING: AtomicBool = AtomicBool::new(false);

/// 本轮渲染的取消令牌，每轮 load_blocks 开始时换新，取消不会串到下一轮
///
/// 同时用于渲染循环按条目查询与本轮下发的下载任务，取消只影响本轮，
/// 不动下载队列里其它任务如整合包安装
static CANCEL: LazyLock<RwLock<CancellationToken>> =
    LazyLock::new(|| RwLock::new(CancellationToken::new()));

/// 全局渲染进度回调，GUI 启动时注册；调用 [`load_blocks`] 时没传回调的
/// 调用方经它上报进度，界面按钮那条路径自带回调，这里只是兜底
static GUI: OnceLock<gui_hook::ProgressGui> = OnceLock::new();

/// 注册全局渲染进度回调，启动时调用一次
///
/// - `gui`: 全局进度回调，重复注册会被忽略
pub fn set_gui_handel(gui: gui_hook::ProgressGui) {
    let _ = GUI.set(gui);
}

/// 是否正在渲染，供界面层防重入查询
pub fn is_loading() -> bool {
    LOADING.load(Ordering::Acquire)
}

/// 取消本轮渲染
///
/// 渲染循环与在途下载都是协作式取消：置位后循环跳过剩余条目、下载在下一个数据块
/// 处收手，`load_blocks` 返回 [`ErrorType::TaskCancel`]；也挂在核心停止链上
pub fn cancel() {
    CANCEL.read().unwrap().cancel();
}

/// 本轮渲染是否已被取消
pub fn is_cancelled() -> bool {
    CANCEL.read().unwrap().is_cancelled()
}

/// 取本轮取消令牌，渲染循环持有克隆，按条目无锁查询
pub(crate) fn cancel_token() -> CancellationToken {
    CANCEL.read().unwrap().clone()
}

/// 下载并渲染方块与物品贴图：版本清单 → 客户端 jar → 解包渲染
///
/// 已有渲染在跑时直接返回 `Ok(())`，调用方先查 [`is_loading`] 区分
///
/// - `gui`: 渲染期间的进度回调，未传时回退全局注册的回调
/// - `force`: 为 true 时忽略版本短路，全量重渲染
///
/// # 返回值
///
/// 被 [`cancel`] 取消时返回 [`ErrorType::TaskCancel`]
pub async fn load_blocks(gui: gui_hook::ProgressGui, force: bool) -> CoreResult<()> {
    if LOADING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    // 新一轮：换新令牌，上一轮的取消不残留
    *CANCEL.write().unwrap() = CancellationToken::new();
    let gui = gui.or_else(|| GUI.get().and_then(|g| g.clone()));
    let result = load_blocks_impl(gui.clone(), force).await;
    LOADING.store(false, Ordering::Release);
    // 收尾借回调发一次空文字事件，把 running 已复位的最终状态推给界面。
    // 进度事件只在渲染步进时触发，结束时没有任何后续事件，
    // 界面会一直停在最后一次步进事件的“渲染中”状态
    if let Some(gui) = &gui {
        gui.set_progress_text(None);
    }
    result
}

/// [`load_blocks`] 的实现：短路判断、补下载客户端 jar 与语言文件、再渲染
async fn load_blocks_impl(gui: gui_hook::ProgressGui, force: bool) -> CoreResult<()> {
    // 本轮取消令牌：渲染循环按条目查询，下载任务也挂在它上面
    let cancel = cancel_token();

    // 版本清单用本地缓存，缺了才在线拉；方块与物品分开短路，只补渲染缺的那部分
    let versions = version_path::get_version_obj_online().await?;
    let last = versions.latest.release.clone();
    let block_done = !force && block::mml_tex_draw_id() == last;
    let item_done = !force && item::items_id() == last;
    // 语言文件也算"已完成"：客户端 jar 只带 en_us，中文要按资源索引单独补下
    // 已渲染过的版本也可能还没下过，短路掉就永远补不上
    if block_done && item_done && langs_ready() {
        return Ok(());
    }

    let Some(ver) = versions.versions.iter().find(|v| v.id == last) else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Version(last)));
    };
    // 版本 JSON 带下载源切换与本地缓存
    let obj = version_path::add_game(ver).await?;
    if obj.downloads.client.url.is_empty() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Version(last)));
    }

    // 客户端 jar 与游戏库共用一份，已存在且 sha1 匹配时跳过下载
    let item = FileItemObj {
        name: format!("{last}.jar"),
        file: libraries_path::get_game_file(&last),
        url: mml_net::url_helper::get_minecraft_client(&obj.downloads.client.url, &last),
        hash: FileHash::Sha1(obj.downloads.client.sha1.clone()),
        later: LaterRun::None,
    };
    if !item.check_hash()
        && !mml_downloader::start_download_task_cancellable(vec![item.clone()], cancel.clone())
            .await
    {
        return Err(cancel_or(ErrorType::DownloadFileFail, &cancel));
    }

    // 语言文件是方块与物品的显示名，jar 里只有 en_us，中文等按资源索引补下
    // 取消要往外抛，本轮已作废；其它失败只记日志，界面退化成显示 ID，贴图照常渲染
    match download_langs(&obj, &cancel).await {
        Ok(()) => {}
        Err(ErrorType::TaskCancel) => return Err(ErrorType::TaskCancel),
        Err(err) => mml_log::error_type(err),
    }

    // 打开 jar 并渲染
    let archive = BaseArchive::open(&item.file)?;
    if !block_done {
        block::render_blocks(&archive, gui.clone())?;
        blocks_write().id = last.clone();
    }
    if !item_done {
        item::render_items(&archive, gui.clone())?;
        items_write().id = last.clone();
    }
    save()?;

    Ok(())
}

/// 收尾错误：本轮已被取消时报取消，用户主动中断不算失败，否则报原错误
///
/// - `err`: 取消未发生时要返回的错误
/// - `cancel`: 本轮取消令牌
///
/// # 返回值
///
/// 已取消返回 [`ErrorType::TaskCancel`]，否则返回 `err`
fn cancel_or(err: ErrorType, cancel: &CancellationToken) -> ErrorType {
    if cancel.is_cancelled() {
        ErrorType::TaskCancel
    } else {
        err
    }
}

/// 方块语言表缓存，按语言缓存整个翻译表
static LANGS: LazyLock<RwLock<HashMap<Lang, HashMap<String, String>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 读取方块语言翻译，数据来自 langs 目录下的语言文件，随 load_blocks 从客户端 jar 提取
///
/// - `lang`: 目标语言
/// - `key`: 语言键，如 `block.minecraft.stone`
///
/// # 返回值
///
/// 返回翻译文本，语言键不存在或语言文件未提取时返回 `None`
pub fn get_lang(lang: Lang, key: &str) -> Option<String> {
    // 命中缓存
    if let Some(map) = LANGS.read().unwrap().get(&lang) {
        return map.get(key).cloned();
    }

    // 从磁盘加载语言文件
    let name = match lang {
        Lang::zh_cn => "zh_cn",
        Lang::en_us => "en_us",
    };
    let file = get_lang_dir()?.join(format!("{name}.json"));
    let text = path_helper::read_text(&file).ok()?;
    let map: HashMap<String, String> = serialize_tools::json_from_str(&text).ok()?;

    let res = map.get(key).cloned();
    LANGS.write().unwrap().insert(lang, map);
    res
}

static BLOCK_FILE: OnceLock<PathBuf> = OnceLock::new();
static BLOCK_DIR: OnceLock<PathBuf> = OnceLock::new();
static LANG_DIR: OnceLock<PathBuf> = OnceLock::new();
static ITEM_FILE: OnceLock<PathBuf> = OnceLock::new();
static ITEM_DIR: OnceLock<PathBuf> = OnceLock::new();

static BLOCKS: LazyLock<RwLock<BlocksObj>> = LazyLock::new(|| RwLock::new(BlocksObj::default()));
static ITEMS: LazyLock<RwLock<ItemsObj>> = LazyLock::new(|| RwLock::new(ItemsObj::default()));

/// 初始化
///
/// - `path`: 数据根目录，方块 / 语言 / 物品目录及数据文件都创建在其下
///
/// # 返回值
///
/// 目录创建失败时返回相应错误，成功返回 `Ok(())`
pub fn init<P: AsRef<Path>>(path: P) -> CoreResult<()> {
    BLOCK_FILE.get_or_init(|| path.as_ref().join(names::BLOCK_FILE));
    ITEM_FILE.get_or_init(|| path.as_ref().join(names::ITEM_FILE));

    let dir = BLOCK_DIR.get_or_init(|| path.as_ref().join(names::BLOCK_DIR));
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    // 语言文件目录与 block 平级，方块与物品共用，随渲染从客户端 jar 提取
    let dir = LANG_DIR.get_or_init(|| path.as_ref().join(names::LANG_DIR));
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    let dir = ITEM_DIR.get_or_init(|| path.as_ref().join(names::ITEM_DIR));
    if !dir.exists() {
        path_helper::create_dir_all(dir)?;
    }

    Ok(())
}

/// 加载数据：只读回上次的渲染结果，版本号在 `block.json` / `items.json` 里
///
/// **不**在启动时自动补渲染，渲染一律由界面按钮经 [`load_blocks`] 触发；
/// 首次运行没有数据文件也正常，读不到就是「未渲染」
///
/// # 返回值
///
/// 数据文件缺失或解析失败时保持空表，仍返回 `Ok(())`
pub fn load() -> CoreResult<()> {
    if let Ok(obj) = serialize_tools::json_from_file::<BlocksObj>(BLOCK_FILE.get().unwrap()) {
        *BLOCKS.write().unwrap() = obj;
    }
    if let Some(file) = ITEM_FILE.get()
        && let Ok(obj) = serialize_tools::json_from_file::<ItemsObj>(file)
    {
        *ITEMS.write().unwrap() = obj;
    }

    Ok(())
}

/// 保存数据：方块表与物品表都写回各自的 json 文件
///
/// # 返回值
///
/// 数据文件路径未初始化或写盘失败时返回相应错误，成功返回 `Ok(())`
pub fn save() -> CoreResult<()> {
    let file = BLOCK_FILE.get().ok_or_else(|| {
        ErrorType::PathNotExists(PathNotExistsData {
            path: PathBuf::from(names::BLOCK_FILE),
        })
    })?;
    let obj = BLOCKS.read().unwrap().clone();
    serialize_tools::json_to_file(&obj, file)?;

    if let Some(file) = ITEM_FILE.get() {
        let obj = ITEMS.read().unwrap().clone();
        serialize_tools::json_to_file(&obj, file)?;
    }

    Ok(())
}

/// 获取方块图片路径
///
/// - `id`: 方块 ID，`minecraft:stone` 或裸 ID `stone`
///
/// # 返回值
///
/// 返回图标 PNG 路径，方块未渲染时返回 `None`
pub fn get_block_path(id: &str) -> Option<PathBuf> {
    let dir = BLOCK_DIR.get().unwrap();
    let binding = BLOCKS.read().unwrap();
    let file = binding.tex.get(id)?;

    Some(dir.join(file))
}

pub use block::icons::SpecialForm;
pub use block::skin::{SKIN_CAT, add_skin_block, remove_skin_block};

/// 判断方块 ID 是否有特殊形态图标，有则返回对应形态
///
/// - `id`: 方块 ID，带不带 `minecraft:` 前缀均可
///
/// # 返回值
///
/// 无特殊形态时返回 `None`
pub fn block_special_form(id: &str) -> Option<SpecialForm> {
    let base = id.strip_prefix("minecraft:").unwrap_or(id);
    block::icons::special_form(&format!("minecraft:{base}"))
}

/// 获取方块特殊形态的图片路径，形态图标 ID = 基础 ID + "_" + 形态后缀，如 oak_door_open
///
/// - `id`: 方块 ID，带不带 `minecraft:` 前缀均可
/// - `form`: 要取的形态
///
/// # 返回值
///
/// 该方块没有此形态时返回 `None`；形态条目不注册进方块列表，文件名按规则直接拼出
pub fn get_block_path_form(id: &str, form: SpecialForm) -> Option<PathBuf> {
    if block_special_form(id) != Some(form) {
        return None;
    }
    let base = id.strip_prefix("minecraft:").unwrap_or(id);
    Some(
        BLOCK_DIR
            .get()?
            .join(format!("minecraft_{base}_{}.png", form.suffix())),
    )
}

/// 获取方块数据目录
pub(crate) fn get_block_dir() -> Option<PathBuf> {
    BLOCK_DIR.get().cloned()
}

/// 获取方块语言文件目录
pub(crate) fn get_lang_dir() -> Option<PathBuf> {
    LANG_DIR.get().cloned()
}

/// jar 语言文件里提出的两张 ID → 语言键映射
///
/// 只存语言键，译文由界面按当前语言经 [`get_lang`] 查询
pub(crate) struct LangKeys {
    /// 方块 ID → 语言键，如 `minecraft:stone` → `block.minecraft.stone`
    pub(crate) block: std::collections::HashMap<String, String>,
    /// 物品 ID → 语言键，如 `minecraft:apple` → `item.minecraft.apple`
    pub(crate) item: std::collections::HashMap<String, String>,
}

/// 提取语言文件到 langs 目录
///
/// - `archive`: 客户端 jar 归档
///
/// # 返回值
///
/// 见 [`LangKeys`]；zh_cn 优先，en_us 兜底，两者都取不到时两张表均为空
pub(crate) fn extract_langs(archive: &BaseArchive) -> LangKeys {
    for name in ["zh_cn", "en_us"] {
        let Ok(data) = archive.read(&format!("assets/minecraft/lang/{name}.json")) else {
            continue;
        };
        // 复制到 langs 目录，供 get_lang 查询翻译
        if let Some(dir) = get_lang_dir() {
            let _ = path_helper::write_bytes(dir.join(format!("{name}.json")), &data);
        }
        let Ok(buf) = String::from_utf8(data) else {
            continue;
        };
        let Ok(lang) =
            serialize_tools::json_from_str::<std::collections::HashMap<String, String>>(&buf)
        else {
            continue;
        };
        // 方块与物品 ID → 语言键：block.minecraft.stone → minecraft:stone
        // Name 只存语言键，由 GUI 按当前语言经 get_lang 翻译
        let mut block_map = std::collections::HashMap::new();
        let mut item_map = std::collections::HashMap::new();
        for lang_key in lang.into_keys() {
            let keys: Vec<&str> = lang_key.split('.').collect();
            if keys.len() == 3 && keys[0] == "block" {
                block_map.insert(format!("{}:{}", keys[1], keys[2]), lang_key.clone());
            } else if keys.len() == 3 && keys[0] == "item" {
                item_map.insert(format!("{}:{}", keys[1], keys[2]), lang_key);
            }
        }
        return LangKeys {
            block: block_map,
            item: item_map,
        };
    }
    LangKeys {
        block: std::collections::HashMap::new(),
        item: std::collections::HashMap::new(),
    }
}

/// 需要从资源索引补下的语言文件
///
/// 客户端 jar 只内置 `en_us.json`，由 [`extract_langs`] 提取；其余语言只在资源索引里
/// 列出，这里列的是除 en_us 外界面用得到的语言
const EXTRA_LANGS: [&str; 1] = ["zh_cn"];

/// 语言文件路径，形如 `langs/<name>.json`，不保证存在
///
/// - `name`: 语言名，如 `zh_cn`
///
/// # 返回值
///
/// 返回文件路径；语言目录尚未初始化时返回 `None`
fn lang_path(name: &str) -> Option<PathBuf> {
    get_lang_dir().map(|dir| dir.join(format!("{name}.json")))
}

/// [`EXTRA_LANGS`] 里的语言文件是否都已在本地
///
/// # 返回值
///
/// 全部存在返回 `true`
fn langs_ready() -> bool {
    EXTRA_LANGS
        .iter()
        .all(|name| lang_path(name).is_some_and(|file| file.is_file()))
}

/// 从资源索引下载方块与物品名称需要的语言文件
///
/// 客户端 jar 只带 `en_us.json`，`zh_cn` 等语言由版本 JSON 的 `assetIndex` 列出、
/// 经资源下载站取回；缺了只能回退显示方块 ID。失败不算渲染失败，只记日志
///
/// - `obj`: 版本数据，取其 `assetIndex` 定位资源索引
/// - `cancel`: 本轮取消令牌，下载挂在它上面，取消只影响本次任务
///
/// # 返回值
///
/// 成功或无需下载返回 `Ok(())`；本轮被取消返回 [`ErrorType::TaskCancel`]；
/// 索引或语言文件获取失败返回对应错误
async fn download_langs(obj: &GameArgObj, cancel: &CancellationToken) -> CoreResult<()> {
    let Some(index) = &obj.asset_index else {
        return Ok(());
    };

    // 资源索引与游戏本体共用一份本地缓存 assets/indexes/<id>.json，缺了才在线拉
    let assets = match assets_path::get_index(index) {
        Ok(assets) => assets,
        Err(_) => {
            let mut url = index.url.clone();
            mml_net::url_helper::change_source(&mut url);
            let data = mml_net::mojang_api::get_assets(&url).await?;
            let assets: AssetsObj = serialize_tools::json_from_bytes(&data)?;
            assets_path::add_index(obj, &mut Cursor::new(data));
            assets
        }
    };

    // 只下本地缺失或大小不符的，大小不符说明客户端换版本了
    let mut list = Vec::new();
    for name in EXTRA_LANGS {
        let Some(asset) = assets.objects.get(&format!("minecraft/lang/{name}.json")) else {
            continue;
        };
        let Some(file) = lang_path(name) else {
            continue;
        };
        let done = match std::fs::metadata(&file) {
            Ok(meta) => meta.len() as i64 == asset.size,
            Err(_) => false,
        };
        if done {
            continue;
        }

        list.push(FileItemObj {
            name: format!("{name}.json"),
            file,
            url: mml_net::url_helper::get_download_assets(&asset.hash),
            hash: FileHash::Sha1(asset.hash.clone()),
            later: LaterRun::None,
        });
    }
    if list.is_empty() {
        return Ok(());
    }

    if !mml_downloader::start_download_task_cancellable(list, cancel.clone()).await {
        return Err(cancel_or(ErrorType::DownloadFileFail, cancel));
    }
    // 语言表按语言整体缓存，重下后要清掉旧版本的翻译
    LANGS.write().unwrap().clear();

    Ok(())
}

/// 获取方块数据，集合语义，顺序无意义
///
/// 底层是 `HashMap::keys()`，每次调用顺序都可能不同；要能直接展示的顺序用
/// [`ordered_entries`]
pub fn blocks() -> Vec<String> {
    BLOCKS
        .read()
        .unwrap()
        .tex
        .keys()
        .map(|item| item.clone())
        .collect()
}

/// 方块数据的渲染版本，未渲染过为空串
pub fn block_version() -> String {
    BLOCKS.read().unwrap().id.clone()
}

/// 是否已有渲染结果，方块或物品任一非空
///
/// 给"只想知道有没有渲染过"的调用方用：[`blocks`] / [`items`] 都是整表克隆，
/// 拿它们判空等于每次白拷两份全表，mml-gui 的方块状态快照就在每个进度回调里调
pub fn has_rendered() -> bool {
    !BLOCKS.read().unwrap().tex.is_empty() || !ITEMS.read().unwrap().tex.is_empty()
}

/// 方块 ID → 语言键，如 block.minecraft.stone，翻译经 `get_lang`
///
/// - `id`: 方块 ID
pub fn block_name_key(id: &str) -> Option<String> {
    BLOCKS.read().unwrap().name.get(id).cloned()
}

/// 物品数据的渲染版本，未渲染过为空串
pub fn item_version() -> String {
    ITEMS.read().unwrap().id.clone()
}

/// 物品 ID → 语言键，如 item.minecraft.apple，翻译经 `get_lang`
///
/// - `id`: 物品 ID
pub fn item_name_key(id: &str) -> Option<String> {
    ITEMS.read().unwrap().name.get(id).cloned()
}

/// 锁定方块数据表，内部写入用
pub(crate) fn blocks_write() -> std::sync::RwLockWriteGuard<'static, BlocksObj> {
    BLOCKS.write().unwrap()
}

/// 锁定方块数据表，内部读取用
pub(crate) fn blocks_read() -> std::sync::RwLockReadGuard<'static, BlocksObj> {
    BLOCKS.read().unwrap()
}

/// 获取物品图片路径
///
/// - `id`: 物品 ID，`minecraft:apple` 或裸 ID `apple`
///
/// # 返回值
///
/// 返回图标 PNG 路径，物品未渲染时返回 `None`
pub fn get_item_path(id: &str) -> Option<PathBuf> {
    let dir = ITEM_DIR.get().unwrap();
    let binding = ITEMS.read().unwrap();
    let file = binding.tex.get(id)?;

    Some(dir.join(file))
}

/// 获取方块创造分组，值为 itemGroup lang 键尾段，翻译经 `get_lang`
///
/// - `id`: 方块 ID
pub fn block_cat(id: &str) -> Option<String> {
    BLOCKS.read().unwrap().cat.get(id).cloned()
}

/// 获取物品创造分组，值与 `block_cat` 同一套
///
/// - `id`: 物品 ID
pub fn item_cat(id: &str) -> Option<String> {
    ITEMS.read().unwrap().cat.get(id).cloned()
}

/// 获取物品数据目录
pub(crate) fn get_item_dir() -> Option<PathBuf> {
    ITEM_DIR.get().cloned()
}

/// 获取物品数据，集合语义，顺序无意义
///
/// 同 [`blocks`]，要能直接展示的顺序用 [`ordered_entries`]
pub fn items() -> Vec<String> {
    ITEMS
        .read()
        .unwrap()
        .tex
        .keys()
        .map(|item| item.clone())
        .collect()
}

/// 锁定物品数据表，内部写入用
pub(crate) fn items_write() -> std::sync::RwLockWriteGuard<'static, ItemsObj> {
    ITEMS.write().unwrap()
}

/// 列表条目的来源，决定图标走 `block/` 还是 `item/`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// 方块，等轴测 3D 图标
    Block,
    /// 物品，平面精灵图标
    Item,
}

/// 一个待展示的方块 / 物品条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 分类值，来自 `block_cat` / `item_cat`；显示名由调用方按当前语言翻译
    pub cat: String,
    /// 来源，方块还是物品
    pub kind: EntryKind,
    /// 条目 id，如 `minecraft:stone`
    pub id: String,
}

/// 方块与物品合并后的展示顺序，列表窗口直接用这个
///
/// 全局按 [`order::order_key`] 排序，所以同一分类只出现一次、分类先后即创造栏标签页顺序；
/// 方块与物品分开排再拼接是错的，只有物品的分类会被挤到最后
///
/// - 同一个 id 既是方块又是物品时只保留方块那条，避免重复条目
/// - 分类值来自 `block_cat` / `item_cat`，显示名由调用方按当前语言翻译
///
/// # 返回值
///
/// 按展示顺序排列的条目，见 [`Entry`]
pub fn ordered_entries() -> Vec<Entry> {
    let mut list: Vec<Entry> = Vec::new();

    let all_blocks = blocks();
    let block_ids: std::collections::HashSet<&str> =
        all_blocks.iter().map(|id| id.as_str()).collect();
    for id in &all_blocks {
        let cat = block_cat(id).unwrap_or_default();
        list.push(Entry {
            cat,
            kind: EntryKind::Block,
            id: id.clone(),
        });
    }
    for id in items() {
        if block_ids.contains(id.as_str()) {
            continue;
        }
        let cat = item_cat(&id).unwrap_or_default();
        list.push(Entry {
            cat,
            kind: EntryKind::Item,
            id,
        });
    }

    list.sort_by(|a, b| order::order_key(&a.cat, &a.id).cmp(&order::order_key(&b.cat, &b.id)));
    list
}

/// 锁定物品数据表，内部读取用
pub(crate) fn items_read() -> std::sync::RwLockReadGuard<'static, ItemsObj> {
    ITEMS.read().unwrap()
}

/// 生成测试用的唯一临时目录路径，不自动创建
///
/// - `tag`: 目录名里的用例标识
///
/// # 返回值
///
/// 返回 `mml_testutil` 临时目录下带进程 ID 与纳秒时间戳的路径
#[cfg(test)]
fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    mml_testutil::temp_dir().join(format!(
        "mml-tex-draw-test-{}-{}-{}",
        tag,
        std::process::id(),
        nanos
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    /// init 应在指定路径下创建 block 数据目录，且目录名来自 names::BLOCK_DIR
    ///
    /// 注意：BLOCK_FILE / BLOCK_DIR 为进程级 OnceLock，首次调用后不再变化，
    /// 因此本二进制内的初始化断言集中在这一条测试中顺序执行。
    #[test]
    fn init_creates_block_dir() {
        let root = unique_temp_dir("unit");
        fs::create_dir_all(&root).unwrap();

        let result = init(&root);
        assert!(result.is_ok());

        let dir = root.join(names::BLOCK_DIR);
        assert!(dir.exists(), "init 后应创建 block 目录：{}", dir.display());

        // 重复调用不应报错，OnceLock 保持首次的路径
        assert!(init(&root).is_ok());

        // 清理临时目录
        let _ = fs::remove_dir_all(&root);
    }

    /// names 模块提供的常量应为预期的相对名称
    #[test]
    fn names_constants() {
        assert_eq!(names::BLOCK_DIR, "block");
        assert_eq!(names::BLOCK_FILE, "block.json");

        // 路径拼接不依赖平台分隔符写法
        let path = Path::new("data").join(names::BLOCK_FILE);
        assert!(path.ends_with("block.json"));
    }
}
