//! 皮肤整体 3D 渲染
//!
//! TypeA / TypeA下 走本文件的正交等距渲染，固定视角正交投影 + 深度测试；
//! TypeB 走内置的透视渲染引擎（[`crate::skin_3d_render`] 与 [`crate::skin_3d_gpu`]）。
//!
//! 模型坐标约定与 TypeB 引擎一致，即 [`crate::skin_3d_cube`] 的顶点顺序加
//! [`crate::skin_3d_texture`] 的 UV 布局：
//! 角色面朝 +z、x− 为角色右手侧、+y 为上，原点取身高一半处；贴图 v 向下。
//! 旧版 64x32 皮肤先补成 64x64，右肢镜像填进左肢块，再统一按新版布局取贴图。
//! 底层部件先画并写深度，顶层 1.125 倍 overlay 后画并做半透明混合。

use std::f32::consts::PI;

use glam::{Mat4, Vec3, Vec4};
use mml_skin::{SkinType, skin_type_checker};
use tiny_skia::Pixmap;

use crate::gpu_3d::{face_verts, render_3d};
use crate::skin_3d_gpu::SkinRenderWgpu;
use crate::skin_3d_render::SkinRenderType;
use crate::skin_draw::{BPP, draw, row_bytes};

/// 输出宽度，单位像素，与皮肤 2D 叠加一致
const SIZE_W: u32 = 272;
/// 输出高度，单位像素，与皮肤 2D 叠加一致
const SIZE_H: u32 = 532;

/// TypeB 档位的画布边长，单位像素，模型居中且不铺满
const SIZE_TYPEB: u32 = 256;

/// 模型缩放，全身 32 像素加 overlay 边缘 33 像素共 478px，画布 532 高
const SCALE: f32 = 14.5;

/// 超采样倍数，2x 渲染后降采样得到抗锯齿
const SUPERSAMPLE: u32 = 2;

/// 读取指定像素的原始 4 字节，预乘 BGRA，仅镜像归一化用
fn get_pixel(image: &Pixmap, x: i32, y: i32) -> [u8; 4] {
    let offset = y as usize * row_bytes(image) + x as usize * BPP;
    let mut out = [0u8; 4];
    out.copy_from_slice(&image.data()[offset..offset + BPP]);
    out
}

/// 写入指定像素的原始 4 字节，预乘 BGRA
fn set_pixel(image: &mut Pixmap, x: i32, y: i32, color: [u8; 4]) {
    let offset = y as usize * row_bytes(image) + x as usize * BPP;
    image.data_mut()[offset..offset + BPP].copy_from_slice(&color);
}

/// 复制一个面区域到目标位置，可选水平翻转
///
/// - `dest`: 目标位图
/// - `dx` / `dy`: 目标区域左上角
/// - `src`: 源位图
/// - `sx` / `sy`: 源区域左上角
/// - `w` / `h`: 区域尺寸，单位像素
/// - `flip`: 是否沿水平方向翻转
///
/// # 返回值
///
/// 恒返回 `Some(())`，返回类型只为与调用处的 `?` 统一
fn copy_face(
    dest: &mut Pixmap,
    dx: i32,
    dy: i32,
    src: &Pixmap,
    sx: i32,
    sy: i32,
    w: i32,
    h: i32,
    flip: bool,
) -> Option<()> {
    for j in 0..h {
        for i in 0..w {
            let si = if flip { w - 1 - i } else { i };
            let pix = get_pixel(src, sx + si, sy + j);
            set_pixel(dest, dx + i, dy + j, pix);
        }
    }
    Some(())
}

/// 旧版 64x32 皮肤补成 64x64：右腿 / 右臂镜像填进左腿 / 左臂的贴图块
///
/// - `image`: 原始皮肤贴图
///
/// # 返回值
///
/// 返回 64x64 的贴图；非 64x32 / 64x64 的尺寸原样返回
///
/// 镜像沿身体左右对称面进行：顶 / 底 / 前 / 后面水平翻转，
/// 左右侧面原样，镜像后朝向互换而贴图本身不翻转。
fn normalize_texture(image: &Pixmap) -> Option<Pixmap> {
    if image.width() == 64 && image.height() == 64 {
        return Some(image.clone());
    }
    if image.width() != 64 || image.height() != 32 {
        // 其它尺寸如 HD 皮肤不处理，按原样取贴图
        return Some(image.clone());
    }

    let mut out = Pixmap::new(64, 64)?;
    // 上半部分即头 / 身体 / 右肢，原样复制
    draw(&mut out, image, 0, 0, 0, 0, 64, 32)?;

    // 右腿块 (0,16) → 左腿块 (16,48)；右臂块 (40,16) → 左臂块 (32,48)
    for (sx, sy, dx, dy) in [(0, 16, 16, 48), (40, 16, 32, 48)] {
        copy_face(&mut out, dx + 4, dy, image, sx + 4, sy, 4, 4, true)?; // 顶
        copy_face(&mut out, dx + 8, dy, image, sx + 8, sy, 4, 4, true)?; // 底
        // 左肢的右侧面 = 源左侧面，镜像后朝向互换，原样复制
        copy_face(&mut out, dx, dy + 4, image, sx + 8, sy + 4, 4, 12, false)?;
        copy_face(&mut out, dx + 4, dy + 4, image, sx + 4, sy + 4, 4, 12, true)?; // 前
        copy_face(&mut out, dx + 8, dy + 4, image, sx, sy + 4, 4, 12, false)?;
        copy_face(
            &mut out,
            dx + 12,
            dy + 4,
            image,
            sx + 12,
            sy + 4,
            4,
            12,
            true,
        )?; // 后
    }

    Some(out)
}

/// 面方向下标，对应 cube.rs 顶点表的面顺序
pub(crate) mod face {
    /// 背面 z−，角色背后
    pub const BACK: usize = 0;
    /// 正面 z+，角色面朝方向
    pub const FRONT: usize = 1;
    /// 角色右手侧 x−
    pub const RIGHT: usize = 2;
    /// 角色左手侧 x+
    pub const LEFT: usize = 3;
    /// 顶面 y+
    pub const TOP: usize = 4;
    /// 底面 y−
    pub const BOTTOM: usize = 5;
    /// 全部六个方向，深度测试自动处理遮挡，顺序不影响结果
    pub const ALL: [usize; 6] = [BACK, FRONT, RIGHT, LEFT, TOP, BOTTOM];
}

/// 取立方体某个面的四个角，含相对部件中心的偏移与块内归一化 uv
///
/// - `h`: 部件半尺寸，模型坐标，单位皮肤像素
/// - `d`: 面方向下标，取 `face` 模块的常量
///
/// # 返回值
///
/// 返回四个 `(位置, uv)`；位置是相对部件中心的模型坐标偏移，
/// uv 是 0..1 的块内比例，v=0 在贴图块顶部
///
/// 顶点顺序与 uv 布局来自 [`crate::skin_3d_cube`] 的 CUBE 与 [`crate::skin_3d_texture`]
/// 的 UV 表：正面 / 侧面 / 顶面 u 沿面向右、v 沿面向下；背面 / 底面 u 镜像。
pub(crate) fn face_corners(h: [f32; 3], d: usize) -> [([f32; 3], [f32; 2]); 4] {
    let (hx, hy, hz) = (h[0], h[1], h[2]);
    match d {
        face::BACK => [
            ([hx, hy, -hz], [1.0, 0.0]),
            ([hx, -hy, -hz], [1.0, 1.0]),
            ([-hx, -hy, -hz], [0.0, 1.0]),
            ([-hx, hy, -hz], [0.0, 0.0]),
        ],
        face::FRONT => [
            ([-hx, hy, hz], [0.0, 0.0]),
            ([-hx, -hy, hz], [0.0, 1.0]),
            ([hx, -hy, hz], [1.0, 1.0]),
            ([hx, hy, hz], [1.0, 0.0]),
        ],
        face::RIGHT => [
            ([-hx, hy, -hz], [0.0, 0.0]),
            ([-hx, -hy, -hz], [0.0, 1.0]),
            ([-hx, -hy, hz], [1.0, 1.0]),
            ([-hx, hy, hz], [1.0, 0.0]),
        ],
        face::LEFT => [
            ([hx, hy, hz], [0.0, 0.0]),
            ([hx, -hy, hz], [0.0, 1.0]),
            ([hx, -hy, -hz], [1.0, 1.0]),
            ([hx, hy, -hz], [1.0, 0.0]),
        ],
        face::TOP => [
            ([-hx, hy, -hz], [0.0, 0.0]),
            ([-hx, hy, hz], [0.0, 1.0]),
            ([hx, hy, hz], [1.0, 1.0]),
            ([hx, hy, -hz], [1.0, 0.0]),
        ],
        _ => [
            ([hx, -hy, -hz], [1.0, 0.0]),
            ([hx, -hy, hz], [1.0, 1.0]),
            ([-hx, -hy, hz], [0.0, 1.0]),
            ([-hx, -hy, -hz], [0.0, 0.0]),
        ],
    }
}

/// 一个面的贴图区域，按皮肤贴图上的 tx / ty / tw / th 排列
pub(crate) type FaceRect = [i32; 4];

/// 从部件尺寸与贴图块左上角取六个面的贴图区域
///
/// - `w` / `h` / `d`: 部件宽 / 高 / 深，单位皮肤像素
/// - `bx` / `by`: 贴图块左上角，单位皮肤像素
/// - `di`: 面方向下标，取 `face` 模块的常量
///
/// # 返回值
///
/// 返回该面的 `[tx, ty, tw, th]`，坐标为皮肤贴图像素
///
/// 块内布局见 texture.rs：上排为 顶(W,D) 与 底(W,D)，
/// 下排为 右(D,H)、前(W,H)、左(D,H)、后(W,H)；
/// 「右」指角色右手侧面 x−，即贴图块最左列。
pub(crate) fn face_rect(w: i32, h: i32, d: i32, bx: i32, by: i32, di: usize) -> FaceRect {
    match di {
        face::BACK => [bx + 2 * d + w, by + d, w, h],
        face::FRONT => [bx + d, by + d, w, h],
        face::RIGHT => [bx, by + d, d, h],
        face::LEFT => [bx + d + w, by + d, d, h],
        face::TOP => [bx + d, by, w, d],
        _ => [bx + d + w, by, w, d],
    }
}

/// 一个身体部件，记录尺寸、基础层与 overlay 层的中心与半尺寸、贴图块左上角
struct Part {
    /// 部件尺寸，按宽 / 高 / 深排列，单位皮肤像素
    dims: [i32; 3],
    /// 基础层中心，模型坐标
    base_center: [f32; 3],
    /// 基础层半尺寸，模型坐标，单位皮肤像素
    base_half: [f32; 3],
    /// overlay 层半尺寸，基础层半尺寸乘 1.125，模型坐标
    overlay_half: [f32; 3],
    /// 基础层贴图块左上角，单位皮肤像素
    base_block: [i32; 2],
    /// overlay 层贴图块左上角，单位皮肤像素
    overlay_block: [i32; 2],
}

/// 组装全身部件，纤细手臂 3 像素宽、普通 4 像素宽
///
/// - `skin_type`: 皮肤类型，决定手臂宽度
///
/// # 返回值
///
/// 返回头 / 身体 / 双腿 / 双臂共 6 个部件
///
/// 手臂整段贴在身体侧面之外：身体 x 半宽 4，手臂从 ±4 往外延伸；角色右手侧在 x−。
fn parts(skin_type: SkinType) -> Vec<Part> {
    let slim = skin_type == SkinType::NewSlim;
    let aw = if slim { 3 } else { 4 }; // 手臂宽，单位皮肤像素
    let enlarge = 1.125; // overlay 层放大系数

    let mk = |dims: [i32; 3], center: [f32; 3], base_block: [i32; 2], overlay_block: [i32; 2]| {
        let half = [
            dims[0] as f32 / 2.0,
            dims[1] as f32 / 2.0,
            dims[2] as f32 / 2.0,
        ];
        Part {
            dims,
            base_center: center,
            base_half: half,
            overlay_half: [half[0] * enlarge, half[1] * enlarge, half[2] * enlarge],
            base_block,
            overlay_block,
        }
    };

    vec![
        // 头：中心在颈部上方 4px，即 y = 12
        mk([8, 8, 8], [0.0, 12.0, 0.0], [0, 0], [32, 0]),
        // 身体
        mk([8, 12, 4], [0.0, 2.0, 0.0], [16, 16], [16, 32]),
        // 右腿 / 左腿，右手侧在 x−
        mk([4, 12, 4], [-2.0, -10.0, 0.0], [0, 16], [0, 32]),
        mk([4, 12, 4], [2.0, -10.0, 0.0], [16, 48], [0, 48]),
        // 右臂 / 左臂：整段贴在身体侧面之外
        mk(
            [aw, 12, 4],
            [-4.0 - aw as f32 / 2.0, 2.0, 0.0],
            [40, 16],
            [40, 32],
        ),
        mk(
            [aw, 12, 4],
            [4.0 + aw as f32 / 2.0, 2.0, 0.0],
            [32, 48],
            [48, 48],
        ),
    ]
}

/// 模型变换矩阵：俯仰 + 偏航 → 缩放 → 平移到画布中心
///
/// - `pitch`: 俯仰角，单位度，负 = 面朝上 / 抬头，正 = 面朝下 / 低头
/// - `yaw`: 偏航角，单位度，角色面朝 +z，正值 = 正面朝向屏幕右侧
///
/// # 返回值
///
/// 返回模型空间到画布像素空间的变换矩阵，y 轴取负以适配向下为 +y 的画布
fn create_tran(pitch: f32, yaw: f32) -> Mat4 {
    let roty = Mat4::from_rotation_y(yaw * PI / 180.0);
    let rotx = Mat4::from_rotation_x(pitch * PI / 180.0);

    let scale = Mat4::from_scale(Vec3::new(SCALE, -SCALE, SCALE));

    let tran = Mat4::from_translation(Vec3::new(SIZE_W as f32 / 2.0, SIZE_H as f32 / 2.0, 0.0));

    tran * scale * rotx * roty
}

/// 渲染 3D 等距全身图，角度由调用方指定
///
/// 渲染 3D 等距全身图，角度由调用方指定
///
/// TypeA / TypeA下 的内置实现，只在本 crate 内按固定角度调用。
///
/// - `image`: 皮肤贴图，支持 64x64 与 64x32，其它尺寸按原样取贴图
/// - `skin_type`: 皮肤类型，`None` 表示按贴图自动检测
/// - `pitch`: 俯仰角，单位度，负 = 面朝上，正 = 面朝下
/// - `yaw`: 偏航角，单位度
///
/// # 返回值
///
/// 返回 `SIZE_W` x `SIZE_H` 的渲染结果，分配失败或无可用 GPU 时返回 `None`
fn draw_skin_3d(
    image: &Pixmap,
    skin_type: Option<SkinType>,
    pitch: f32,
    yaw: f32,
) -> Option<Pixmap> {
    let st = skin_type.unwrap_or_else(|| skin_type_checker::get_skin_type(image));
    let texture = normalize_texture(image)?;
    let part_list = parts(st);
    let (tw, th) = (texture.width() as f32, texture.height() as f32);

    let mut base = Vec::new();
    let mut overlay = Vec::new();
    for part in &part_list {
        for &d in &face::ALL {
            base.extend_from_slice(&face_verts(
                face_corners(part.base_half, d).map(|(pos, uv)| {
                    (
                        [
                            pos[0] + part.base_center[0],
                            pos[1] + part.base_center[1],
                            pos[2] + part.base_center[2],
                        ],
                        uv,
                    )
                }),
                face_rect(
                    part.dims[0],
                    part.dims[1],
                    part.dims[2],
                    part.base_block[0],
                    part.base_block[1],
                    d,
                ),
                tw,
                th,
            ));
            overlay.extend_from_slice(&face_verts(
                face_corners(part.overlay_half, d).map(|(pos, uv)| {
                    (
                        [
                            pos[0] + part.base_center[0],
                            pos[1] + part.base_center[1],
                            pos[2] + part.base_center[2],
                        ],
                        uv,
                    )
                }),
                face_rect(
                    part.dims[0],
                    part.dims[1],
                    part.dims[2],
                    part.overlay_block[0],
                    part.overlay_block[1],
                    d,
                ),
                tw,
                th,
            ));
        }
    }

    render_3d(
        &texture,
        &base,
        &overlay,
        create_tran(pitch, yaw),
        SIZE_W,
        SIZE_H,
        SUPERSAMPLE,
    )
}

/// 渲染 3D 等距全身图，固定水平 45°、面朝上 30° 的仰视角度
///
/// - `image`: 皮肤贴图
/// - `skin_type`: 皮肤类型，`None` 为自动检测
///
/// # 返回值
///
/// 返回 `SIZE_W` x `SIZE_H` 的渲染结果，分配失败或无可用 GPU 时返回 `None`
pub fn typea_up(image: &Pixmap, skin_type: Option<SkinType>) -> Option<Pixmap> {
    draw_skin_3d(image, skin_type, -30.0, 45.0)
}

/// 渲染 3D 等距全身图，固定水平 45°、面朝下 30° 的俯视角度
///
/// - `image`: 皮肤贴图
/// - `skin_type`: 皮肤类型，`None` 为自动检测
///
/// # 返回值
///
/// 返回 `SIZE_W` x `SIZE_H` 的渲染结果，分配失败或无可用 GPU 时返回 `None`
pub fn typea_down(image: &Pixmap, skin_type: Option<SkinType>) -> Option<Pixmap> {
    draw_skin_3d(image, skin_type, 30.0, 45.0)
}

/// 渲染 TypeB 风格的 3D 全身图：透视视角、透明背景、含顶层
///
/// 与 TypeA / TypeA下 的正交等距图不同，这一档走内置的透视渲染引擎，画布是边长
/// `SIZE_TYPEB` 的正方形，模型居中且不铺满画布。
///
/// - `image`: 皮肤贴图，宽度须为 64
/// - `skin_type`: 皮肤类型，`None` 为自动检测
/// - `pitch`: 俯仰角，单位度，负 = 面朝上，正 = 面朝下
/// - `yaw`: 偏航角，单位度，取 0 为正视图
///
/// # 返回值
///
/// 返回 `SIZE_TYPEB` x `SIZE_TYPEB` 的渲染结果；贴图宽度不是 64、皮肤类型未知
/// 或无可用 GPU 时返回 `None`
pub fn typeb(
    image: &Pixmap,
    skin_type: Option<SkinType>,
    pitch: f32,
    yaw: f32,
) -> Option<Pixmap> {
    let st = skin_type.unwrap_or_else(|| skin_type_checker::get_skin_type(image));

    let mut render = SkinRenderWgpu::new();
    render.width = SIZE_TYPEB as i32;
    render.height = SIZE_TYPEB as i32;
    render.skin_type = st;
    render.enable_top = true;
    render.back_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
    render.render_type = SkinRenderType::MSAA;
    render.set_skin_tex(image).ok()?;
    render.rotate(pitch, yaw);

    render.render().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::IntSize;

    /// 构造 64x64 新版皮肤：头 / 身体正面用可辨识颜色，其余透明
    fn make_skin() -> Pixmap {
        let data = vec![0u8; 64 * 64 * BPP];
        let mut bm = Pixmap::from_vec(data, IntSize::from_wh(64, 64).unwrap()).unwrap();
        // 头正面 (8,8,8,8) 红；身体正面 (20,20,8,12) 蓝
        for j in 0..8 {
            for i in 0..8 {
                set_pixel(&mut bm, 8 + i, 8 + j, [255, 0, 0, 255]);
            }
        }
        for j in 0..12 {
            for i in 0..8 {
                set_pixel(&mut bm, 20 + i, 20 + j, [0, 0, 255, 255]);
            }
        }
        bm
    }

    /// 渲染输出 272x532；角色面向屏幕右：头正面中心 ≈ (177, 95)、
    /// 身体正面中心 ≈ (156, 231)
    #[test]
    fn test_draw_skin_3d_layout() {
        let skin = make_skin();
        let out = typea_up(&skin, Some(SkinType::New)).expect("渲染应成功");
        assert_eq!(out.width(), SIZE_W);
        assert_eq!(out.height(), SIZE_H);

        let head = get_pixel(&out, 177, 95);
        assert_eq!(head[3], 255, "头正面应不透明");
        assert!(head[0] > 128, "头正面应为红色系 {:?}", head);

        let body = get_pixel(&out, 156, 231);
        assert_eq!(body[3], 255, "身体正面应不透明");
        assert!(body[2] > 128, "身体正面应为蓝色系 {:?}", body);

        // 画布四角应保持透明，模型居中
        for (x, y) in [(5, 5), (267, 5), (5, 527), (267, 527)] {
            assert_eq!(get_pixel(&out, x, y)[3], 0, "四角应透明 ({x},{y})");
        }
    }

    /// 手臂应整段贴在身体侧面之外：把右臂块的四个侧面都涂绿，
    /// 45° 视角下绿色区域最左端 ≈ 75；若手臂半埋进身体，x 从 -6 起，只能到 ≈ 95
    #[test]
    fn test_draw_skin_3d_arm_outside() {
        let data = vec![0u8; 64 * 64 * BPP];
        let mut bm = Pixmap::from_vec(data, IntSize::from_wh(64, 64).unwrap()).unwrap();
        // 右臂块下排四个侧面 (40,20)-(56,32) 绿色
        for j in 0..12 {
            for i in 0..16 {
                set_pixel(&mut bm, 40 + i, 20 + j, [0, 255, 0, 255]);
            }
        }
        let out = typea_up(&bm, Some(SkinType::New)).expect("渲染应成功");

        let mut leftmost = SIZE_W as i32;
        for y in 0..SIZE_H as i32 {
            for x in 0..SIZE_W as i32 {
                if get_pixel(&out, x, y)[3] > 0 {
                    leftmost = x;
                    break;
                }
            }
            if leftmost < SIZE_W as i32 {
                break;
            }
        }
        assert!(
            leftmost < 80,
            "手臂应伸出身体轮廓外（左缘 {leftmost} 应 < 80）"
        );
    }

    /// 旧版 64x32 皮肤：右肢镜像补进左肢块后应能正常渲染
    #[test]
    fn test_draw_skin_3d_old_skin() {
        let data = vec![0u8; 64 * 32 * BPP];
        let mut bm = Pixmap::from_vec(data, IntSize::from_wh(64, 32).unwrap()).unwrap();
        // 头正面
        for j in 0..8 {
            for i in 0..8 {
                set_pixel(&mut bm, 8 + i, 8 + j, [255, 0, 0, 255]);
            }
        }
        let out = typea_up(&bm, None).expect("旧皮肤渲染应成功");
        assert_eq!(out.width(), SIZE_W);
        assert_eq!(get_pixel(&out, 177, 95)[0], 255, "头正面应为红色");

        // 镜像归一化后输出 64x64
        let norm = normalize_texture(&bm).expect("归一化应成功");
        assert_eq!(norm.width(), 64);
        assert_eq!(norm.height(), 64);
    }

    /// TypeB 档位输出 SIZE_TYPEB 见方、背景透明，模型画在画面里且不铺满
    #[test]
    fn test_draw_skin_3d_typeb() {
        let skin = make_skin();
        let out = typeb(&skin, Some(SkinType::New), 0.0, 30.0).expect("渲染应成功");
        assert_eq!(out.width(), SIZE_TYPEB);
        assert_eq!(out.height(), SIZE_TYPEB);

        // 背景 alpha 为 0，四角应透明
        for (x, y) in [(0, 0), (255, 0), (0, 255), (255, 255)] {
            assert_eq!(get_pixel(&out, x, y)[3], 0, "四角应透明 ({x},{y})");
        }

        let drawn = (0..SIZE_TYPEB)
            .flat_map(|y| (0..SIZE_TYPEB).map(move |x| (x as i32, y as i32)))
            .filter(|(x, y)| get_pixel(&out, *x, *y)[3] > 0)
            .count();
        assert!(drawn > 1000, "模型应画在画面里，实际不透明像素 {drawn}");

        // 旋转是入参，换个偏航角结果必须不同
        let turned = typeb(&skin, Some(SkinType::New), 0.0, -30.0).expect("渲染应成功");
        assert_ne!(out.data(), turned.data(), "偏航角变化后画面必须变化");

        // 宽度不是 64 的贴图直接拒绝
        let narrow = Pixmap::new(32, 32).unwrap();
        assert!(
            typeb(&narrow, Some(SkinType::New), 0.0, 30.0).is_none(),
            "宽度非 64 的贴图不应出图"
        );
    }
}
