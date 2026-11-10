//! 皮肤头渲染
//!
//! 从皮肤贴图裁出头部六个面，烘成方块模型。
use std::collections::HashMap;

use crate::model::{BakedModel, BakedModelTextures, Quad};

use mml_base::archives::BaseArchive;
use tiny_skia::Pixmap;

/// 追加头颅立方体的 6 个面到 `quads`，照 SkullModel 的 head 部件
///
/// 模型空间 y 轴向下，8×8×8 立方体，x/z ∈ ±4、y ∈ -8..0；帽子层同尺寸外扩 0.25 像素、
/// 贴图偏移 (32,0)。摆放照 SkullBlockRenderer 绕 X 轴 180° 翻转后居中，头落在方块内
/// 4..12 / 0..8 / 4..12 像素处；纯旋转不改变绕序，无需重排 winding
///
/// uv 矩形取自各面在贴图上的展开位置：右(0,8) 前(8,8) 左(16,8) 后(24,8) 顶(8,0) 底(16,0)；
/// 每面 4 个顶点按索引取 [0]右上 [1]左上 [2]左下 [3]右下
///
/// - `quads`: 输出 quad 列表，就地追加
/// - `tex`: 头颅贴图，皮肤头部展开布局
/// - `path`: 贴图相对路径，写入 quad
/// - `tex_off`: 该层贴图在图上的偏移，单位像素，基础层 (0,0)、帽子层 (32,0)
/// - `grow`: 外扩距离，单位像素，基础层为 0
pub(super) fn push_head_cube(
    quads: &mut Vec<Quad>,
    tex: &Pixmap,
    path: &str,
    tex_off: [f32; 2],
    grow: f32,
) {
    let (tw, th) = (tex.width() as f32, tex.height() as f32);
    let min = [-4.0 - grow, -8.0 - grow, -4.0 - grow];
    let max = [4.0 + grow, 0.0 + grow, 4.0 + grow];

    // 立方体三边都是 8：uv 展开的各列与行位置 u1..u4、v1..v2
    let (u, v) = (tex_off[0], tex_off[1]);
    let (u1, u2, u3, u4) = (u + 8.0, u + 16.0, u + 24.0, u + 32.0);
    let (v1, v2) = (v + 8.0, v + 16.0);

    // 顶点：t* 在 z=min 面、l* 在 z=max 面，照 ModelPart.Cube 的命名
    let t0 = [min[0], min[1], min[2]];
    let t1 = [max[0], min[1], min[2]];
    let t2 = [max[0], max[1], min[2]];
    let t3 = [min[0], max[1], min[2]];
    let l0 = [min[0], min[1], max[2]];
    let l1 = [max[0], min[1], max[2]];
    let l2 = [max[0], max[1], max[2]];
    let l3 = [min[0], max[1], max[2]];

    // (顶点, 模型空间法线, uv矩形[left,top,right,bottom])，面顺序同 ModelPart.Cube
    let faces: [([[f32; 3]; 4], [f32; 3], [f32; 4]); 6] = [
        ([l1, l0, t0, t1], [0.0, -1.0, 0.0], [u1, v, u2, v1]), // 底
        ([t2, t3, l3, l2], [0.0, 1.0, 0.0], [u2, v1, u3, v]),  // 顶（矩形上下颠倒）
        ([t0, l0, l3, t3], [-1.0, 0.0, 0.0], [u, v1, u1, v2]), // 西
        ([t1, t0, t3, t2], [0.0, 0.0, -1.0], [u1, v1, u2, v2]), // 北
        ([l1, t1, t2, l2], [1.0, 0.0, 0.0], [u2, v1, u3, v2]), // 东
        ([l0, l1, l2, l3], [0.0, 0.0, 1.0], [u3, v1, u4, v2]), // 南
    ];

    for (verts, normal, rect) in faces {
        let (left, top, right, bottom) = (rect[0], rect[1], rect[2], rect[3]);
        // 顶点 i 的 uv 角照 Polygon 构造：[0]→(right,top) [1]→(left,top) [2]→(left,bottom) [3]→(right,bottom)
        let corners = [[right, top], [left, top], [left, bottom], [right, bottom]];
        let pos = verts.map(|p| [p[0] / 16.0 + 0.5, -p[1] / 16.0, -p[2] / 16.0 + 0.5]);
        let uv = corners.map(|c| [c[0] / tw, c[1] / th]);
        let uv_rect = [
            left.min(right) / tw,
            top.min(bottom) / th,
            left.max(right) / tw,
            top.max(bottom) / th,
        ];
        quads.push(Quad {
            pos,
            uv,
            normal: [normal[0], -normal[1], -normal[2]],
            // 头颅贴图无 tint：皮肤颜色已烘在贴图里
            color: [[1.0; 4]; 4],
            tex: path.to_string(),
            translucent: crate::model::rect_translucent(tex, None, &uv_rect),
            fullbright: false,
        });
    }
}

/// 烘焙一个头颅为 quad 列表：贴图、立方体几何与图标 gui 变换
///
/// gui 变换取 item/template_skull 的 display.gui：旋转 30/45/0、平移 0/3/0、缩放 1，
/// 与创造栏视角一致；模型空间已居中，无需再适配画布
///
/// - `archive`: 客户端 jar 归档，读取头颅贴图
/// - `tex_rel`: jar 内贴图相对路径，如 `entity/zombie/zombie`
/// - `hat`: 是否带帽子层；玩家头与僵尸头为 true，贴图须为 64×64
/// - `tex_cache`: 跨模型复用的贴图缓存，路径 → 解码位图
///
/// # 返回值
///
/// 见 [`BakedModelTextures`]；贴图读取失败时返回 `None`
pub fn bake_head(
    archive: &BaseArchive,
    tex_rel: &str,
    hat: bool,
    tex_cache: &mut HashMap<String, Pixmap>,
) -> Option<BakedModelTextures> {
    let path = crate::model::texture_path(tex_rel)?;
    let tex = crate::model::load_texture(archive, tex_cache, &path)?;

    // 帽子层取自 humanoidHeadLayer，其贴图是 64×64；mobHeadLayer 的 64×32 也"放得下"
    // texOffs(32,0)，但取到的是头部侧面，故按贴图高度兜底断言表的 hat 标记
    debug_assert!(
        !hat || tex.height() >= 64,
        "{tex_rel} 标了帽子层，但贴图 {}×{} 不是 humanoidHeadLayer 的 64×64",
        tex.width(),
        tex.height()
    );

    let mut textures = HashMap::new();
    textures.insert(path.clone(), tex.clone());
    Some(BakedModelTextures {
        model: bake_head_model(tex, hat, &path),
        textures,
    })
}

/// 头颅烘焙核心：从已加载的贴图直接出模型，外部皮肤文件的图标渲染也走这里
///
/// - `tex`: 头颅贴图
/// - `hat`: 是否带帽子层
/// - `path`: 贴图相对路径，写入 quad
///
/// # 返回值
///
/// 返回烘焙产物，基础层 6 面，带帽子层时再加 6 面
pub fn bake_head_model(tex: Pixmap, hat: bool, path: &str) -> BakedModel {
    let mut quads = Vec::new();
    push_head_cube(&mut quads, &tex, path, [0.0, 0.0], 0.0);
    if hat {
        push_head_cube(&mut quads, &tex, path, [32.0, 0.0], 0.25);
    }

    BakedModel {
        quads,
        // 头颅是 3D 模型，走 ITEMS_3D 侧向光照，同游戏
        gui_light_3d: true,
        transform: crate::model::GuiTransform {
            rotation: [30.0, 45.0, 0.0],
            // display 的平移单位是1/16方块，此处已按 GuiTransform 约定换算
            translation: [0.0, 3.0 / 16.0, 0.0],
            scale: [1.0; 3],
        },
    }
}
