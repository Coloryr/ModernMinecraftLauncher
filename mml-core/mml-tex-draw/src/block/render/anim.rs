//! 贴图动画
//!
//! 动画定义与时间轴，按刻取当前帧。

use mml_base::{archives::BaseArchive, serialize_tools};
use serde::{Deserialize, Serialize};
use tiny_skia::Pixmap;

use super::*;

/// 时间线上的一段：播放第几帧、持续多久
pub(super) struct AnimEntry {
    /// 源帧序号，索引 `AnimTimeline::src`；构建期已按帧数取模，不会越界
    frame: u32,
    /// 该段持续刻数，至少 1
    ticks: u32,
}

/// 动画时间线：源帧 + 展开后的帧序列，按刻惰性取帧
///
/// 取帧时才做克隆与插值混合，避免物化整条逐刻帧序列，frametime 大时可达上千帧位图
pub(crate) struct AnimTimeline {
    /// 源帧位图，按条带顺序切出
    src: Vec<Pixmap>,
    /// 展开后的帧序列，按播放顺序排列
    entries: Vec<AnimEntry>,
    /// 帧之间是否平滑插值
    interpolate: bool,
}

impl AnimTimeline {
    /// 由原始帧条带 + mcmeta 元数据构建时间线
    ///
    /// - `tex`: 竖排帧条带
    /// - `meta`: 动画配置，来自 mcmeta
    ///
    /// # 返回值
    ///
    /// 返回时间线，条带切帧失败时返回 `None`
    pub(crate) fn build(tex: &Pixmap, meta: &AnimMeta) -> Option<Self> {
        let count = (tex.height() / tex.width()).max(1) as usize;
        let src: Vec<Pixmap> = (0..count)
            .map(|i| extract_frame(tex, i))
            .collect::<Option<Vec<_>>>()?;
        let frametime = meta.frametime.max(1);
        // frames 列表优先，帧号越界按帧数取模；缺省按 0..n 顺序、每帧 frametime 刻
        let entries = meta
            .frames
            .as_ref()
            .map(|list| {
                list.iter()
                    .map(|f| AnimEntry {
                        frame: f.index % src.len() as u32,
                        ticks: f.time.max(1),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                (0..src.len() as u32)
                    .map(|i| AnimEntry {
                        frame: i,
                        ticks: frametime,
                    })
                    .collect()
            });
        Some(Self {
            src,
            entries,
            interpolate: meta.interpolate,
        })
    }

    /// 整条时间线的总时长，单位刻
    pub(crate) fn total_ticks(&self) -> u32 {
        self.entries.iter().map(|entry| entry.ticks).sum()
    }

    /// 取逐刻时间线上第 tick 刻的帧；interpolate 时向序列下一帧线性过渡
    ///
    /// - `tick`: 游戏刻，调用方需先按 `total_ticks` 取模
    ///
    /// # 返回值
    ///
    /// 返回该刻的帧位图，时间线为空时返回 `None`
    pub(crate) fn frame_at(&self, tick: u32) -> Option<Pixmap> {
        let mut acc = 0u32;
        for (i, entry) in self.entries.iter().enumerate() {
            if tick < acc + entry.ticks {
                let frame = &self.src[entry.frame as usize];
                if !self.interpolate {
                    return Some(frame.clone());
                }
                let next = self.entries[(i + 1) % self.entries.len()].frame as usize;
                let f = (tick - acc) as f32 / entry.ticks as f32;
                return blend_bitmap(frame, &self.src[next], f);
            }
            acc += entry.ticks;
        }
        None
    }
}

/// 按比例线性插值混合两张 RGBA 贴图
///
/// - `a`: 起始帧，f=0 时的结果
/// - `b`: 目标帧，f=1 时的结果
/// - `f`: 混合比例，取值 0..1
///
/// # 返回值
///
/// 返回混合后的位图，尺寸不一致或分配失败时返回 `None`
pub(super) fn blend_bitmap(a: &Pixmap, b: &Pixmap, f: f32) -> Option<Pixmap> {
    if a.width() != b.width() || a.height() != b.height() {
        return None;
    }

    let mut out = Pixmap::new(a.width(), a.height())?;

    for ((o, x), y) in out
        .data_mut()
        .chunks_exact_mut(4)
        .zip(a.data().chunks_exact(4))
        .zip(b.data().chunks_exact(4))
    {
        for c in 0..4 {
            o[c] = (f32::from(x[c]) * (1.0 - f) + f32::from(y[c]) * f).round() as u8;
        }
    }

    Some(out)
}

/// 动画贴图配置
#[derive(Clone)]
pub struct AnimMeta {
    /// 帧间隔，单位游戏刻，1 刻 = 50ms
    pub frametime: u32,
    /// 帧之间是否平滑插值
    pub interpolate: bool,
    /// mcmeta 的 frames 自定义帧序列，缺省按 0..n 顺序展开
    pub frames: Option<Vec<AnimFrame>>,
}

/// 帧序列的一项：帧号与该帧持续刻数，缺省时长为 frametime
#[derive(Clone, Copy)]
pub struct AnimFrame {
    /// 帧号，从 0 起，越界按帧数取模
    pub index: u32,
    /// 该帧持续刻数，单位刻
    pub time: u32,
}

/// mcmeta 文件顶层，JSON 镜像
#[derive(Serialize, Deserialize, Default)]
pub(super) struct TextureMetaObj {
    /// animation 节点，非动画贴图没有
    animation: Option<TextureAnimationObj>,
}

/// mcmeta 的 animation 节点，JSON 镜像
#[derive(Serialize, Deserialize, Default)]
pub(super) struct TextureAnimationObj {
    /// 帧间隔，单位刻
    frametime: Option<u32>,
    /// 帧之间是否平滑插值
    interpolate: Option<bool>,
    /// 自定义帧序列：数字项为帧号，对象项为{index, time}
    frames: Option<Vec<serde_json::Value>>,
}

/// 读取动画贴图配置：帧间隔、是否插值、自定义帧序列
///
/// - `archive`: 客户端 jar 归档，读取 `.mcmeta`
/// - `tex_path`: jar 内贴图路径
///
/// # 返回值
///
/// 返回动画配置；无 mcmeta 或解析失败时返回缺省配置，frametime 为 1 且不插值
pub fn read_anim_meta(archive: &BaseArchive, tex_path: &str) -> AnimMeta {
    let fallback = AnimMeta {
        frametime: 1,
        interpolate: false,
        frames: None,
    };
    let Ok(data) = archive.read(&format!("{tex_path}.mcmeta")) else {
        return fallback;
    };
    let Ok(buf) = String::from_utf8(data) else {
        return fallback;
    };
    let Ok(meta) = serialize_tools::json_from_str::<TextureMetaObj>(&buf) else {
        return fallback;
    };
    let Some(animation) = meta.animation else {
        return fallback;
    };
    let frametime = animation.frametime.filter(|t| *t > 0).unwrap_or(1);
    let frames = animation.frames.map(|list| {
        list.into_iter()
            .filter_map(|f| match f {
                serde_json::Value::Number(n) => n.as_u64().map(|i| AnimFrame {
                    index: i as u32,
                    time: frametime,
                }),
                serde_json::Value::Object(o) => {
                    let index = o.get("index").and_then(|v| v.as_u64())? as u32;
                    let time = o
                        .get("time")
                        .and_then(|v| v.as_u64())
                        .filter(|t| *t > 0)
                        .unwrap_or(frametime as u64) as u32;
                    Some(AnimFrame { index, time })
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    AnimMeta {
        frametime,
        interpolate: animation.interpolate.unwrap_or(false),
        frames: frames.filter(|f| !f.is_empty()),
    }
}

/// 当前已渲染的游戏版本
pub(crate) fn mml_tex_draw_id() -> String {
    crate::blocks_read().id.clone()
}
