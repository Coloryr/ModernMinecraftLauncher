//! 平面图标烘焙
//!
//! 按 ItemModelGenerator 的规则把贴图挤出成前后两面加裙边的 quad。
use std::collections::HashMap;

use glam::{Mat3, Mat4, Vec3};
use mml_base::archives::BaseArchive;
use tiny_skia::Pixmap;

use crate::{
    block::read_anim_meta,
    model::{
        BakedModel, BakedModelTextures, Quad, Resolved, calculate_facing, face_index_of,
        face_normal, load_texture, rect_translucent, resolve, texture_path,
    },
};

use super::model::{FaceData, bake_face};

/// extrude 挤出烘焙，照反编译 ItemModelGenerator：
/// layer0..layer4 依次叠加遇缺失即止，layer0 缺失整体失败；每层
/// front/back 全幅面 + 不透明像素边界裙边，动画贴图取各帧并集
///
/// - `archive`: 客户端 jar 归档，读取贴图与动画元数据
/// - `textures`: 父链合并后的贴图引用表，取 layer0..layer4
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
/// - `tints`: tint 表，tintIndex = 层号
///
/// # 返回值
///
/// 返回全部层的 quad，layer0 缺失时返回 `None`
pub(super) fn bake_generated(
    archive: &BaseArchive,
    textures: &HashMap<String, crate::block::TextureRefObj>,
    tex_cache: &mut HashMap<String, Pixmap>,
    tints: &[[u8; 3]],
) -> Option<Vec<Quad>> {
    // 层贴图：layer0..layer4，遇缺失即止
    let layers: Vec<(String, Pixmap)> = (0..5)
        .map_while(|i| {
            let value = textures.get(&format!("layer{i}"))?.value()?;
            let path = texture_path(value)?;
            let tex = load_texture(archive, tex_cache, &path)?;
            Some((path, tex))
        })
        .collect();
    if layers.is_empty() {
        return None;
    }

    const MIN_Z: f32 = 7.5;
    const MAX_Z: f32 = 8.5;
    const UV_SHRINK: f32 = 0.1;

    let mut quads = Vec::new();
    for (layer_index, (path, tex)) in layers.iter().enumerate() {
        let anim = read_anim_meta(archive, path);
        // tintIndex = 层号，越界为白色，与游戏一致
        let tint = tints.get(layer_index).copied().unwrap_or([255, 255, 255]);
        let color = [
            tint[0] as f32 / 255.0,
            tint[1] as f32 / 255.0,
            tint[2] as f32 / 255.0,
            1.0,
        ];

        // 不透明像素判定：alpha != 0，照 SpriteContents.isTransparent；
        // 动画贴图取各帧并集，getSideFrames 走唯一帧集合
        let Some(crate::model::BitmapRgba {
            data: base,
            stride,
            width: w,
            height: h_total,
        }) = crate::model::bitmap_rgba(tex)
        else {
            continue;
        };
        if w == 0 {
            continue;
        }
        let frame_count = (h_total / w).max(1);
        let frame_h = h_total / frame_count;
        let opaque = |x: usize, y: usize| -> bool {
            (0..frame_count).any(|frame| {
                let row = frame * w + y;
                row < h_total && base[row * stride + x * 4 + 3] != 0
            })
        };

        // front/back 全幅面，盒为 (0,0,7.5)-(16,16,8.5)：
        // SOUTH uv [0,0,16,16] 正面 z=8.5；NORTH uv [16,0,0,16] 背面 z=7.5 镜像
        for (facing, uv16) in [
            (3usize, [0.0f32, 0.0, 16.0, 16.0]), // SOUTH
            (2usize, [16.0, 0.0, 0.0, 16.0]),    // NORTH
        ] {
            let FaceData { pos, uv, normal } =
                bake_face(&[0.0, 0.0, MIN_Z], &[16.0, 16.0, MAX_Z], facing, uv16);
            quads.push(Quad {
                pos,
                uv,
                normal,
                color: [color; 4],
                tex: path.clone(),
                translucent: rect_translucent(&tex, Some(&anim), &[0.0, 0.0, 1.0, 1.0]),
                fullbright: false,
            });
        }

        // 裙边：不透明像素且邻边透明，出界算透明，生成侧面。
        // SideDirection：UP=Direction.UP (0,1)、DOWN=DOWN (0,-1)、LEFT=EAST (1,0)、
        // RIGHT=WEST (-1,0)；邻居 = (x-stepX, y-stepY)
        let (fw, fh) = (w as f32, frame_h as f32);
        let (x_scale, y_scale) = (16.0 / fw, 16.0 / fh);
        for (facing, (dx, dy), horizontal) in [
            (1usize, (0isize, 1isize), true),   // UP
            (0usize, (0isize, -1isize), true),  // DOWN
            (5usize, (1isize, 0isize), false),  // LEFT = EAST
            (4usize, (-1isize, 0isize), false), // RIGHT = WEST
        ] {
            for y in 0..frame_h {
                for x in 0..w {
                    if !opaque(x, y) {
                        continue;
                    }
                    // 邻居透明，出界算透明，才生成裙边
                    let (nx, ny) = (x as isize - dx, y as isize - dy);
                    if nx >= 0
                        && ny >= 0
                        && (nx as usize) < w
                        && (ny as usize) < w
                        && opaque(nx as usize, ny as usize)
                    {
                        continue;
                    }

                    // uv：u0=x+0.1 u1=x+0.9；横向面 v0=y+0.1 v1=y+0.9，
                    // 纵向面 v 反转，v0=y+0.9 v1=y+0.1；按帧尺寸缩放
                    let u0 = (x as f32 + UV_SHRINK) * x_scale;
                    let u1 = (x as f32 + 1.0 - UV_SHRINK) * x_scale;
                    let (v0, v1) = if horizontal {
                        (
                            (y as f32 + UV_SHRINK) * y_scale,
                            (y as f32 + 1.0 - UV_SHRINK) * y_scale,
                        )
                    } else {
                        (
                            (y as f32 + 1.0 - UV_SHRINK) * y_scale,
                            (y as f32 + UV_SHRINK) * y_scale,
                        )
                    };

                    // 几何：startY/endY 按帧尺寸缩放后 y 翻转，即 16 - 值；
                    // 各方向照反编译 switch 取 from/to，退化盒由bakeFace归一角序
                    let (start_x, end_x, start_y, end_y) = match (facing, horizontal) {
                        (1, _) | (0, _) => (
                            x as f32,
                            x as f32 + 1.0,
                            if facing == 1 {
                                y as f32
                            } else {
                                y as f32 + 1.0
                            },
                            if facing == 1 {
                                y as f32
                            } else {
                                y as f32 + 1.0
                            },
                        ),
                        (5, _) => (x as f32, x as f32, y as f32, y as f32 + 1.0),
                        _ => (x as f32 + 1.0, x as f32 + 1.0, y as f32, y as f32 + 1.0),
                    };
                    let start_x = start_x * x_scale;
                    let end_x = end_x * x_scale;
                    let start_y = 16.0 - start_y * y_scale;
                    let end_y = 16.0 - end_y * y_scale;

                    let (from, to) = match facing {
                        // UP / DOWN：水平 quad，XZ 平面，Y 固定
                        1 => ([start_x, start_y, MIN_Z], [end_x, start_y, MAX_Z]),
                        0 => ([start_x, end_y, MIN_Z], [end_x, end_y, MAX_Z]),
                        // LEFT / RIGHT：垂直 quad，YZ 平面，X 固定
                        5 => ([start_x, start_y, MIN_Z], [start_x, end_y, MAX_Z]),
                        _ => ([end_x, start_y, MIN_Z], [end_x, end_y, MAX_Z]),
                    };
                    let FaceData { pos, uv, normal } =
                        bake_face(&from, &to, facing, [u0, v0, u1, v1]);
                    quads.push(Quad {
                        pos,
                        uv,
                        normal,
                        color: [color; 4],
                        tex: path.clone(),
                        translucent: rect_translucent(
                            &tex,
                            Some(&anim),
                            &[u0 / 16.0, v0 / 16.0, u1 / 16.0, v1 / 16.0],
                        ),
                        fullbright: false,
                    });
                }
            }
        }
    }
    Some(quads)
}

/// 物品模型烘焙：items/*.json 的 model 引用，取 models/ 相对名。
/// 只走2D平面物品，generated 链 → extrude；引用3D模型的物品形态
/// 如 items/stone.json → block/stone，由方块表渲染，这里跳过。
/// `extra`：节点 transformation，composite 子模型拼合用，烘焙期直接作用于顶点
///
/// - `archive`: 客户端 jar 归档
/// - `rel`: 模型文件相对路径，取 models/ 下的相对名
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
/// - `tints`: tint 表，extrude 按层号索引
/// - `extra`: 节点 transformation 矩阵，无则 `None`
///
/// # 返回值
///
/// 见 [`BakedModelTextures`]；非 generated 链或烘焙失败时返回 `None`
pub(crate) fn bake_item_model(
    archive: &BaseArchive,
    rel: &str,
    tex_cache: &mut HashMap<String, Pixmap>,
    tints: &[[u8; 3]],
    extra: Option<Mat4>,
) -> Option<BakedModelTextures> {
    let Resolved {
        textures,
        transform,
        gui_light_3d,
        generated,
        ..
    } = resolve(archive, rel)?;

    // 仅2D平面物品，即 builtin/generated 链；3D模型留给方块渲染
    if !generated {
        return None;
    }
    let mut quads = bake_generated(archive, &textures, tex_cache, tints)?;
    if quads.is_empty() {
        return None;
    }

    // 节点 transformation 直接作用于顶点，法线按逆转置变换后重新量化
    if let Some(m) = extra {
        let n3 = Mat3::from_mat4(m).inverse().transpose();
        for quad in &mut quads {
            for p in &mut quad.pos {
                *p = m.transform_point3(Vec3::from(*p)).to_array();
            }
            quad.normal = match calculate_facing(&quad.pos).and_then(|d| face_index_of(d.into())) {
                Some(f) => face_normal(f).to_array(),
                None => (n3 * Vec3::from(quad.normal)).normalize().to_array(),
            };
        }
    }

    let mut used_textures = HashMap::new();
    for quad in &quads {
        if !used_textures.contains_key(&quad.tex) {
            let tex = load_texture(archive, tex_cache, &quad.tex)?;
            used_textures.insert(quad.tex.clone(), tex);
        }
    }
    Some(BakedModelTextures {
        model: BakedModel {
            quads,
            gui_light_3d,
            transform,
        },
        textures: used_textures,
    })
}

/// ===== 渲染入口 =====

/// 已渲染完成的版本号，未渲染为空，load_blocks短路用
pub(crate) fn items_id() -> String {
    crate::items_read().id.clone()
}
