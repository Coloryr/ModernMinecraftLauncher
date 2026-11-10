//! 加载与落盘
//!
//! 从客户端 jar 渲染方块与物品图标，读写缓存文件。
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use mml_base::{
    archives::BaseArchive,
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools,
};
use mml_game::{
    gui_hook,
    launcher_path::{libraries_path, version_path},
};
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType, PathNotExistsData},
    names,
};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;

use crate::block::obj::{BlocksObj, ItemsObj};

use super::{lang::*, query::*, state::*};

use crate::{block, item};

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
pub(crate) async fn load_blocks_impl(gui: gui_hook::ProgressGui, force: bool) -> CoreResult<()> {
    // 本轮取消令牌：渲染循环按条目查询，下载任务也挂在它上面
    let cancel = cancel_token();

    // 版本清单用本地缓存，缺了才在线拉；方块与物品分开短路，只补渲染缺的那部分
    let versions = version_path::get_version_obj_online().await?;
    let last = versions.latest.release.clone();
    // 2D 图标是后加的：老数据只有 3D，表里为空时要补渲染一轮，否则永远没有 2D 图标
    let block_done =
        !force && block::mml_tex_draw_id() == last && !BLOCKS.read().unwrap().tex2d.is_empty();
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
            .is_done()
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
    let archive = BaseArchive::open_readonly(&item.file)?;
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
pub(crate) fn cancel_or(err: ErrorType, cancel: &CancellationToken) -> ErrorType {
    if cancel.is_cancelled() {
        ErrorType::TaskCancel
    } else {
        err
    }
}

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

    // 2D 平面图标与 3D 图标分开存放：两套要能同时存在，切换时不用重渲染
    let dir = BLOCK2D_DIR.get_or_init(|| path.as_ref().join(names::BLOCK2D_DIR));
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
