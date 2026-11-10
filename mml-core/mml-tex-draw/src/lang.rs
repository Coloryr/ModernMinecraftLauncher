//! 语言表
//!
//! 从资源索引下载语言文件，提供方块 / 物品名翻译。
use std::{
    collections::HashMap,
    io::Cursor,
    path::PathBuf,
    sync::{LazyLock, OnceLock, RwLock},
};

use mml_base::{
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_game::{
    launcher_path::assets_path,
    mojang::{assets_obj::AssetsObj, game_arg_obj::GameArgObj},
};
use mml_names::{
    Lang,
    i18_items::error_type::{CoreResult, ErrorType},
};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;

use crate::block::obj::{BlocksObj, ItemsObj};

use super::{load::*, query::*};

/// 方块语言表缓存，按语言缓存整个翻译表
pub(crate) static LANGS: LazyLock<RwLock<HashMap<Lang, HashMap<String, String>>>> =
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

pub(crate) static BLOCK_FILE: OnceLock<PathBuf> = OnceLock::new();
pub(crate) static BLOCK_DIR: OnceLock<PathBuf> = OnceLock::new();
pub(crate) static BLOCK2D_DIR: OnceLock<PathBuf> = OnceLock::new();
pub(crate) static LANG_DIR: OnceLock<PathBuf> = OnceLock::new();
pub(crate) static ITEM_FILE: OnceLock<PathBuf> = OnceLock::new();
pub(crate) static ITEM_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(crate) static BLOCKS: LazyLock<RwLock<BlocksObj>> =
    LazyLock::new(|| RwLock::new(BlocksObj::default()));
pub(crate) static ITEMS: LazyLock<RwLock<ItemsObj>> =
    LazyLock::new(|| RwLock::new(ItemsObj::default()));

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
pub(crate) fn lang_path(name: &str) -> Option<PathBuf> {
    get_lang_dir().map(|dir| dir.join(format!("{name}.json")))
}

/// [`EXTRA_LANGS`] 里的语言文件是否都已在本地
///
/// # 返回值
///
/// 全部存在返回 `true`
pub(crate) fn langs_ready() -> bool {
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
pub(crate) async fn download_langs(obj: &GameArgObj, cancel: &CancellationToken) -> CoreResult<()> {
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

    if !mml_downloader::start_download_task_cancellable(list, cancel.clone())
        .await
        .is_done()
    {
        return Err(cancel_or(ErrorType::DownloadFileFail, cancel));
    }
    // 语言表按语言整体缓存，重下后要清掉旧版本的翻译
    LANGS.write().unwrap().clear();

    Ok(())
}
