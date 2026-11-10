//! 渲染管线
//!
//! 遍历图标表、按规格分派、汇总耗时与产出。

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use crate::block::icons::{BLOCK_ICONS, IconSpec};
use crate::gpu::GpuCtx;

use mml_base::archives::BaseArchive;
use mml_game::gui_hook::ProgressGui;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tiny_skia::Pixmap;

use super::*;

/// textures 表的值：老版为字符串，新版可为对象，如 glass 的 force_translucent
/// 公开供手动测试调试单个方块用
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextureRefObj {
    /// 老版：直接是贴图引用值
    Plain(String),
    /// 新版：对象形式，只取 sprite 字段
    Object { sprite: String },
}

impl TextureRefObj {
    /// 取引用值字符串
    ///
    /// # 返回值
    ///
    /// 返回贴图引用值，如 `block/stone`；两种形态都总有值
    pub(crate) fn value(&self) -> Option<&str> {
        match self {
            Self::Plain(value) => Some(value),
            Self::Object { sprite } => Some(sprite),
        }
    }
}

/// 分阶段耗时统计，纳秒累计；设置 MML_RENDER_PROFILE=1 时在渲染结束后打印
/// 用于定位并发渲染的性能瓶颈，正常路径只有两次 fetch_add
static PROFILE: LazyLock<RenderProfile> = LazyLock::new(|| RenderProfile {
    enabled: std::env::var("MML_RENDER_PROFILE").is_ok(),
    ..Default::default()
});

/// 渲染各阶段耗时累计，仅在 MML_RENDER_PROFILE 设置时输出
#[derive(Default)]
pub(super) struct RenderProfile {
    /// 是否启用统计，来自环境变量 MML_RENDER_PROFILE
    enabled: bool,
    /// 模型与贴图解析阶段的累计耗时，单位纳秒
    resolve: AtomicU64,
    /// GPU 绘制阶段的累计耗时，单位纳秒
    draw: AtomicU64,
    /// 出图写盘阶段的累计耗时，单位纳秒
    write: AtomicU64,
}

impl RenderProfile {
    /// 打印各阶段耗时统计，未启用时直接返回
    fn print(&self, total: std::time::Duration) {
        if !self.enabled {
            return;
        }
        let f = |nanos: u64| nanos as f64 / 1e9;
        println!(
            "[性能] 总计{:.1}s | resolve {:.1}s | draw {:.1}s | write {:.1}s | 其他 {:.1}s",
            total.as_secs_f64(),
            f(self.resolve.load(Ordering::Relaxed)),
            f(self.draw.load(Ordering::Relaxed)),
            f(self.write.load(Ordering::Relaxed)),
            total.as_secs_f64()
                - f(self.resolve.load(Ordering::Relaxed))
                - f(self.draw.load(Ordering::Relaxed))
                - f(self.write.load(Ordering::Relaxed)),
        );
    }
}

/// 按创造模式图标表渲染全部方块
///
/// 渲染清单与分类来自 `block_icons::BLOCK_ICONS`，是生成脚本从 26.2 客户端 jar 提取、
/// 26.3 增量更新后手工维护的固定表；下载流程见 lib 的 `load_blocks`，这里只做解包渲染。
/// 渲染按 CPU 核数经 rayon 并发，各线程持有只读 jar 句柄与贴图缓存，完成后单线程汇总
///
/// - `archive`: 客户端 jar 归档，只读
/// - `gui`: 进度回调，按已处理的表条目数上报，传 `None` 不上报
///
/// # 返回值
///
/// 输出目录未初始化或 GPU 不可用时返回相应错误，成功返回 `Ok(())`
pub fn render_blocks(archive: &BaseArchive, gui: ProgressGui) -> CoreResult<()> {
    let render_start = std::time::Instant::now();
    // 输出目录未初始化时报错；各图标写盘前也各自取一次
    crate::get_block_dir().ok_or(ErrorType::DownloadFileFail)?;

    // GPU 上下文按平台条件编译选择后端，DX12→VK→GL，创建一次跨线程复用
    // 图标渲染只有这一条路径，全不可用时直接报错
    let Some(gpu) = GpuCtx::try_new() else {
        return Err(ErrorType::GpuNotAvailable);
    };

    // 提取语言文件并构建 ID->语言键 映射
    let names = crate::extract_langs(archive);

    let total = BLOCK_ICONS.len();
    let done = AtomicUsize::new(0);
    // 每1%左右上报一次，避免高频回调刷爆 GUI
    let step = (total / 100).max(1);

    // 每个工作线程的独立资源，随线程存活跨 job 复用：jar 只读句柄用 thread_local 保证
    // 每线程只开一次，map_init 的 init 在工作被偷取时会按 job 重跑；贴图缓存避免反复解码
    thread_local! {
        static LOCAL_ARCHIVE: RefCell<Option<Option<BaseArchive>>> = const { RefCell::new(None) };
        static TEX_CACHE: RefCell<HashMap<String, Pixmap>> = RefCell::new(HashMap::new());
    }

    // 本轮取消令牌：取消后跳过剩余条目，进度随之停住；已写出的 PNG 留着
    // 下次全量渲染覆盖，与渲染失败时的残留一致
    let cancel = crate::cancel_token();

    // 并发渲染
    let rendered: Vec<Option<RenderedIcon>> = BLOCK_ICONS
        .par_iter()
        .map(|(id_rel, cat, spec)| {
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

                    // 单个表条目的处理，进度计数对跳过的条目也要生效
                    let item_start = std::time::Instant::now();
                    let out = render_icon(Some(&gpu), archive, &mut tex_cache, id_rel, spec);

                    // 定位异常慢的单方块，如超大动画贴图
                    if PROFILE.enabled {
                        let secs = item_start.elapsed().as_secs_f64();
                        if secs > 1.0 {
                            println!("[性能] 慢方块 {id_rel}：{secs:.1}s");
                        }
                    }

                    // 处理完一个计一个，含跳过的；每跨过 step 阈值上报一次
                    let now = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if now % step == 0
                        && let Some(gui) = &gui
                    {
                        gui.set_progress_now(now, Some(total));
                    }

                    // Form 形态只出图不注册：ID 非真实方块名，不进 blocks() 列表
                    out.filter(|_| !matches!(spec, IconSpec::Form(..)))
                        .map(|icon| RenderedIcon {
                            id: icon.id,
                            out_name: icon.out_name,
                            out_name2d: icon.out_name2d,
                            cat: cat.to_string(),
                        })
                })
            })
        })
        .collect();

    PROFILE.print(render_start.elapsed());

    // 取消：本轮作废。版本号由调用方在返回 Ok 后才写入，这里提前返回
    // 不会把半成品标成「已渲染」
    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 完成时补一次 100%，避免进度停在最后一个 step 前
    if let Some(gui) = &gui {
        gui.set_progress_now(total, Some(total));
    }

    let mut blocks = crate::blocks_write();
    for icon in rendered.into_iter().flatten() {
        blocks.tex.insert(icon.id.clone(), icon.out_name);
        if let Some(faces) = icon.out_name2d {
            blocks.tex2d.insert(icon.id.clone(), faces);
        }
        blocks.cat.insert(icon.id.clone(), icon.cat);
        if let Some(lang_key) = names.block.get(&icon.id) {
            blocks.name.insert(icon.id, lang_key.clone());
        }
    }

    Ok(())
}
