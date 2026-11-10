//! 物品图标渲染
//!
//! 遍历物品表、解析模型、走 GPU / CPU 渲染并写盘。
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use mml_base::{archives::BaseArchive, serialize_tools};
use mml_game::gui_hook::ProgressGui;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use tiny_skia::Pixmap;

use crate::{
    block::{fit_composite, render_baked},
    gpu::GpuCtx,
    model::{
        BakedModel, BakedModelTextures, GuiTransform, load_texture,
    },
};
use mml_sys::path_helper;


use super::icons;
use super::*;

/// 按jar内items/*.json渲染全部物品图标
///
/// 渲染流程与render_blocks一致：rayon并发，各工作线程thread_local持有只读jar句柄
/// 与贴图缓存；GPU优先逐图标回退CPU。物品定义解析失败，如special/empty/引用缺失，即跳过
///
/// `gui`可选：按已处理的物品数上报进度，约每1%一次
///
/// - `archive`: 客户端 jar 归档，只读枚举与渲染
/// - `gui`: 进度回调，传 `None` 不上报
///
/// # 返回值
///
/// 输出目录未初始化或 GPU 不可用时返回相应错误，成功返回 `Ok(())`
pub fn render_items(archive: &BaseArchive, gui: ProgressGui) -> CoreResult<()> {
    // 输出目录未初始化时报错
    crate::get_item_dir().ok_or(ErrorType::DownloadFileFail)?;

    // 图标渲染只有 GPU 一条路径，无 CPU 软件光栅化回退，全不可用时直接报错
    let Some(gpu) = GpuCtx::try_new() else {
        return Err(ErrorType::GpuNotAvailable);
    };
    let names = crate::extract_langs(archive);

    // 枚举jar内全部物品定义；block表已有的ID不重复渲染，图标由方块表负责
    let defs: Vec<String> = archive
        .entries()
        .iter()
        .filter_map(|e| {
            let rest = e.name.strip_prefix("assets/minecraft/items/")?;
            let name = rest.strip_suffix(".json")?;
            (!e.is_dir && !name.contains('/')).then(|| name.to_string())
        })
        .filter(|name| !crate::block::icons::has_icon(name))
        .collect();

    let total = defs.len();
    let done = AtomicUsize::new(0);
    // 每1%左右上报一次，避免高频回调刷爆GUI
    let step = (total / 100).max(1);

    // 与render_blocks相同的工作线程资源：只读jar句柄 + 贴图解码缓存
    thread_local! {
        static LOCAL_ARCHIVE: RefCell<Option<Option<BaseArchive>>> = const { RefCell::new(None) };
        static TEX_CACHE: RefCell<HashMap<String, Pixmap>> = RefCell::new(HashMap::new());
    }

    // 本轮取消令牌：取消后跳过剩余条目，进度随之停住；
    // 已写出的 PNG 留着，下次全量渲染覆盖，与渲染失败时的残留一致
    let cancel = crate::cancel_token();

    let rendered: Vec<Option<ItemIconOutput>> = defs
        .par_iter()
        .map(|id| {
            if cancel.is_cancelled() {
                return None;
            }
            LOCAL_ARCHIVE.with(|local_archive| {
                TEX_CACHE.with(|tex_cache| {
                    let mut tex_cache = tex_cache.borrow_mut();
                    let mut local_archive = local_archive.borrow_mut();
                    let archive = local_archive
                        .get_or_insert_with(|| BaseArchive::open_readonly(archive.path()).ok())
                        .as_ref()
                        .unwrap_or(archive);

                    // 处理完一个计一个，含跳过的，每跨过step阈值上报一次
                    let out = render_item(Some(&gpu), archive, &mut tex_cache, id);
                    let now = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if now % step == 0
                        && let Some(gui) = &gui
                    {
                        gui.set_progress_now(now, Some(total));
                    }
                    out
                })
            })
        })
        .collect();

    // 取消：本轮作废。版本号由调用方在返回 Ok 后才写入，这里提前返回
    // 不会把半成品标成「已渲染」
    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 完成时补一次100%，避免进度停在最后一个step前
    if let Some(gui) = &gui {
        gui.set_progress_now(total, Some(total));
    }

    // 单线程汇总写入物品状态
    let mut items = crate::items_write();
    for icon in rendered.into_iter().flatten() {
        let key = format!("minecraft:{}", icon.id);
        items.tex.insert(key.clone(), icon.out_name);
        // 创造分组，与方块表同一套itemGroup分类
        if let Some(cat) = icons::item_cat(&icon.id) {
            items.cat.insert(key.clone(), cat.to_string());
        }
        // 物品语言键优先，方块物品如stone回落block.minecraft.*
        if let Some(name) = names.item.get(&key).or_else(|| names.block.get(&key)) {
            items.name.insert(key, name.clone());
        }
    }

    Ok(())
}

/// 渲染并写盘完成的一个物品图标
pub struct ItemIconOutput {
    /// 物品 ID，不带命名空间，如 `apple`
    pub id: String,
    /// 输出文件名，如 `minecraft_apple.png`
    pub out_name: String,
}

/// 渲染单个物品图标并写盘
///
/// 公开供单物品诊断渲染 tests/render_one.rs 与render_items复用
///
/// - `gpu`: GPU 上下文，`None` 时由 render_baked 内部回退 CPU 路径
/// - `archive`: 客户端 jar 归档
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
/// - `id`: 物品ID，不带命名空间，如 `apple`
///
/// # 返回值
///
/// 见 [`ItemIconOutput`]；定义不可渲染或渲染失败时返回 `None`
pub fn render_item(
    gpu: Option<&GpuCtx>,
    archive: &BaseArchive,
    tex_cache: &mut HashMap<String, Pixmap>,
    id: &str,
) -> Option<ItemIconOutput> {
    let data = archive
        .read(&format!("assets/minecraft/items/{id}.json"))
        .ok()?;
    let def = serialize_tools::json_from_bytes::<ItemDefJson>(&data).ok()?;
    let children = resolve_item_models(def.model.as_ref()?)?;
    if children.is_empty() {
        return None;
    }

    // glint：vanilla 默认注册带附魔光效的物品
    let glint_tex = if GLINT_IDS.contains(&id) {
        load_texture(archive, tex_cache, GLINT_TEX)
    } else {
        None
    };

    // 各子模型烘焙合并，composite；gui变换与光照取首part
    let mut quads = Vec::new();
    let mut textures: HashMap<String, Pixmap> = HashMap::new();
    let mut base: Option<(bool, GuiTransform)> = None;
    // composite 子模型带 transformation，床等，超出单格，渲染前按投影包围盒适配画布
    let mut oversized = false;
    for child in &children {
        let BakedModelTextures {
            model: part,
            textures: texs,
        } = bake_item_model(
            archive,
            &child.model,
            tex_cache,
            &child.tints,
            child.transform,
        )?;
        textures.extend(texs);
        oversized |= child.transform.is_some();
        if base.is_none() {
            base = Some((part.gui_light_3d, part.transform));
        }
        quads.extend(part.quads);
    }
    let (gui_light_3d, transform) = base?;
    let mut model = BakedModel {
        quads,
        gui_light_3d,
        transform,
    };
    if oversized {
        fit_composite(&mut model);
    }

    let data = render_baked(gpu, archive, model, textures, glint_tex.as_ref())?;
    let out_name = format!("minecraft_{id}.png");
    path_helper::write_bytes(&crate::get_item_dir()?.join(&out_name), &data).ok()?;
    Some(ItemIconOutput {
        id: id.to_string(),
        out_name,
    })
}
