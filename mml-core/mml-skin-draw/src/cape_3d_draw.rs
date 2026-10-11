//! 披风 / 鞘翅本体的 3D 等距渲染
//!
//! 走与 [`crate::skin_3d_draw`] 的 TypeA 同一条正交管线（[`crate::gpu_3d::render_3d`]）：
//! 面料按盒展开拼成顶点，投影是像素空间正交投影，背景透明。
//! 摆放口径与前端在用的 `skinview3d` 一致：披风是一块 10x16x1 的布；
//! 鞘翅是两片 10x20x2 的翼，各自绕肩点（x = ±5）张开，翼盒中心在肩点下 10、后 1。

use std::f32::consts::PI;

use glam::{Mat4, Vec3};
use tiny_skia::Pixmap;

use crate::gpu_3d::{Vert, face_verts, render_3d};
use crate::skin_3d_draw::{face, face_corners, face_rect};

/// 输出边长，单位像素
const SIZE: u32 = 256;
/// 模型占画布的比例，四周留白
const FILL: f32 = 0.8;
/// 超采样倍数，2x 渲染后降采样得到抗锯齿
const SUPERSAMPLE: u32 = 2;

/// 肩点到身体中线的距离，单位皮肤像素
const WING_HINGE: f32 = 5.0;
/// 翼盒中心相对肩点的偏移，单位皮肤像素，取自 skinview3d 的 ElytraObject
const WING_OFFSET: [f32; 3] = [-5.0, -10.0, -1.0];
/// 收拢姿态的俯仰角，单位度，取自 skinview3d 的 ElytraObject 默认姿态
const WING_PITCH: f32 = 15.0;
/// 收拢姿态的滚转角，单位度
const WING_ROLL: f32 = 15.0;
/// 两片翼收拢时几乎完全重叠，给一点绕 y 的错开，免得共面 z-fighting
const WING_SEPARATE: f32 = 0.01;

/// 渲染披风本体的 3D 图
///
/// yaw 为 0 时看到的是正面，即贴图块 (1,1)，与 [`crate::draw_cape`] 的
/// `DrawCapeType::Front` 同一面；盒展开给 +z 面取的 u 是正的，所以不需要翻 u。
///
/// - `image`: 披风贴图，64x32 的盒展开
/// - `pitch`: 俯仰角，单位度，负 = 面朝上，正 = 面朝下
/// - `yaw`: 偏航角，单位度，0 为正对
///
/// # 返回值
///
/// 返回 `SIZE` 见方的透明底位图，无可用 GPU 时返回 `None`
pub fn cape(image: &Pixmap, pitch: f32, yaw: f32) -> Option<Pixmap> {
    let (tex_w, tex_h) = tex_size(image);
    let base = box_faces(10, 16, 1, 0, 0, Mat4::IDENTITY, false, tex_w, tex_h);

    render_fitted(image, base, pitch, yaw)
}

/// 渲染鞘翅本体的 3D 图，两片翼按收拢姿态摆放
///
/// 两片翼的贴图块都在 (22,0)。每片翼绕自身 x / z 各转 `WING_PITCH` / `WING_ROLL` 度、
/// 挂在肩点（`x = ±5`）下方，两片几乎叠成一片壳；右翼由左翼沿 x 镜像得到。
/// 这个是 skinview3d `ElytraObject` 的默认（未飞行）姿态。
///
/// - `image`: 鞘翅贴图，64x32 的盒展开
/// - `pitch`: 俯仰角，单位度
/// - `yaw`: 偏航角，单位度，0 为正对，看见的是鞘翅外侧
///
/// # 返回值
///
/// 返回 `SIZE` 见方的透明底位图，无可用 GPU 时返回 `None`
pub fn elytra(image: &Pixmap, pitch: f32, yaw: f32) -> Option<Pixmap> {
    let (tex_w, tex_h) = tex_size(image);

    // 收拢姿态：翼盒绕自身 x / z 各转一点、挂在肩点下方，两片几乎叠成一片壳；
    // 先绕 y 翻 180 度让有图案的外侧朝向观察者，flip_u 再抵消盒展开对 -z 面的镜像
    let place = Mat4::from_rotation_y(PI)
        * Mat4::from_translation(Vec3::new(WING_HINGE, 0.0, 0.0))
        * Mat4::from_rotation_x(WING_PITCH.to_radians())
        * Mat4::from_rotation_y(WING_SEPARATE)
        * Mat4::from_rotation_z(WING_ROLL.to_radians())
        * Mat4::from_translation(Vec3::from(WING_OFFSET));
    let wing = box_faces(10, 20, 2, 22, 0, place, true, tex_w, tex_h);

    let mut base = mirror_x(&wing);
    base.extend(wing);

    render_fitted(image, base, pitch, yaw)
}

/// 贴图宽高，单位像素，按实际尺寸归一化 uv
fn tex_size(image: &Pixmap) -> (f32, f32) {
    (image.width() as f32, image.height() as f32)
}

/// 把顶点集居中、按包围盒定好缩放再渲染
///
/// 缩放按最长边算，模型总是占画布的 [`FILL`]，鞘翅张开比披风宽得多也不会出画。
///
/// - `image`: 贴图
/// - `verts`: 模型顶点，模型坐标，单位皮肤像素
/// - `pitch` / `yaw`: 观察角度，单位度
///
/// # 返回值
///
/// 返回 `SIZE` 见方的透明底位图，无可用 GPU 时返回 `None`
fn render_fitted(image: &Pixmap, mut verts: Vec<Vert>, pitch: f32, yaw: f32) -> Option<Pixmap> {
    let (center, extent) = bounds(&verts);
    for vert in verts.iter_mut() {
        for (axis, value) in vert.pos.iter_mut().enumerate() {
            *value -= center[axis];
        }
    }

    let scale = SIZE as f32 * FILL / extent;
    render_3d(
        image,
        &verts,
        &[],
        tran(pitch, yaw, scale),
        SIZE,
        SIZE,
        SUPERSAMPLE,
    )
}

/// 顶点集的包围盒
///
/// - `verts`: 模型顶点
///
/// # 返回值
///
/// 返回包围盒中心与最长边的长度，顶点为空时返回原点与 1，避免除零
fn bounds(verts: &[Vert]) -> ([f32; 3], f32) {
    let Some(first) = verts.first() else {
        return ([0.0; 3], 1.0);
    };

    let (mut min, mut max) = (first.pos, first.pos);
    for vert in verts {
        for axis in 0..3 {
            min[axis] = min[axis].min(vert.pos[axis]);
            max[axis] = max[axis].max(vert.pos[axis]);
        }
    }

    let center = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    let extent = (0..3)
        .map(|axis| max[axis] - min[axis])
        .fold(0.0f32, f32::max);

    (center, extent)
}

/// 建一个盒的六个面，uv 取自盒展开的贴图块
///
/// - `w` / `h` / `d`: 盒的宽 / 高 / 厚，单位皮肤像素
/// - `bx` / `by`: 贴图块左上角，单位像素
/// - `place`: 把盒从自身中心搬到模型坐标的矩阵
/// - `tex_w` / `tex_h`: 贴图宽高，单位像素
///
/// # 返回值
///
/// 返回六个面的顶点，每面 6 个，可直接喂给 [`render_3d`]
fn box_faces(
    w: i32,
    h: i32,
    d: i32,
    bx: i32,
    by: i32,
    place: Mat4,
    flip_u: bool,
    tex_w: f32,
    tex_h: f32,
) -> Vec<Vert> {
    let half = [w as f32 / 2.0, h as f32 / 2.0, d as f32 / 2.0];
    let mut out = Vec::new();

    for &dir in &face::ALL {
        let mut corners = face_corners(half, dir);
        if flip_u {
            // 块内比例取反即块内水平镜像，贴图跟着翻，几何不动
            for (_, uv) in corners.iter_mut() {
                uv[0] = 1.0 - uv[0];
            }
        }

        for vert in face_verts(corners, face_rect(w, h, d, bx, by, dir), tex_w, tex_h) {
            out.push(Vert {
                pos: (place * Vec3::from(vert.pos).extend(1.0))
                    .truncate()
                    .to_array(),
                uv: vert.uv,
            });
        }
    }

    out
}

/// 把一组顶点沿 x 轴镜像
///
/// 位置取反会翻转绕序，每个三角形交换后两个顶点抵回来，镜像后外侧仍是正面；
/// uv 跟着顶点走，贴图于是自然呈现为镜像。
///
/// - `verts`: 每 6 个顶点一个面，来自 [`box_faces`]
///
/// # 返回值
///
/// 返回等长的镜像顶点
fn mirror_x(verts: &[Vert]) -> Vec<Vert> {
    let mut out = Vec::with_capacity(verts.len());

    for face in verts.chunks_exact(6) {
        // 一个面的 6 个顶点是 c0 c1 c2 / c0 c2 c3，两个三角形各自交换后两个顶点即可
        // 抵消位置取反带来的绕序翻转
        for i in [0usize, 2, 1, 3, 5, 4] {
            let vert = face[i];
            out.push(Vert {
                pos: [-vert.pos[0], vert.pos[1], vert.pos[2]],
                uv: vert.uv,
            });
        }
    }

    out
}

/// 模型空间到画布像素空间的变换：俯仰 + 偏航 → 缩放 → 平移到画布中心
///
/// - `pitch`: 俯仰角，单位度
/// - `yaw`: 偏航角，单位度
/// - `scale`: 每皮肤像素占多少画布像素
///
/// # 返回值
///
/// 返回模型到画布的变换矩阵，y 轴取负以适配向下为 +y 的画布
fn tran(pitch: f32, yaw: f32, scale: f32) -> Mat4 {
    let roty = Mat4::from_rotation_y(yaw.to_radians());
    let rotx = Mat4::from_rotation_x(pitch.to_radians());
    let scale = Mat4::from_scale(Vec3::new(scale, -scale, scale));
    let center = Mat4::from_translation(Vec3::new(SIZE as f32 / 2.0, SIZE as f32 / 2.0, 0.0));

    center * scale * rotx * roty
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin_draw::BPP;
    use tiny_skia::IntSize;

    /// 造一张 64x32 贴图，标记的像素不透明，其余透明
    fn make_texture(mark: impl Fn(i32, i32) -> bool) -> Pixmap {
        let mut data = vec![0u8; 64 * 32 * BPP];
        for y in 0..32 {
            for x in 0..64 {
                if mark(x, y) {
                    let off = y as usize * 64 * BPP + x as usize * BPP;
                    data[off..off + BPP].copy_from_slice(&[0, 0, 255, 255]);
                }
            }
        }

        Pixmap::from_vec(data, IntSize::from_wh(64, 32).unwrap()).unwrap()
    }

    /// 统计不透明像素数
    fn opaque(image: &Pixmap) -> usize {
        image
            .data()
            .chunks_exact(BPP)
            .filter(|px| px[3] > 0)
            .count()
    }

    /// 统计不透明像素在 x / y 方向各跨多少列 / 行
    fn spans(image: &Pixmap) -> (u32, u32) {
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (SIZE, 0, SIZE, 0);
        for y in 0..SIZE {
            for x in 0..SIZE {
                if image.pixel(x, y).unwrap().alpha() > 0 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        (max_x - min_x + 1, max_y - min_y + 1)
    }

    /// 披风 3D：整块布上色时应画满一块，四角透明
    #[test]
    fn test_cape_3d_draws_cloth() {
        // 正面块 (1,1)，yaw 为 0 时看的正是这一面
        let image = make_texture(|x, y| (1..11).contains(&x) && (1..17).contains(&y));

        let out = cape(&image, 0.0, 0.0).expect("披风 3D 应出图");
        assert_eq!((out.width(), out.height()), (SIZE, SIZE));
        assert!(opaque(&out) > 20_000, "正面看应画出一整块布");

        // 四角留白，说明模型没铺满画布
        for (x, y) in [(0, 0), (SIZE - 1, 0), (0, SIZE - 1), (SIZE - 1, SIZE - 1)] {
            assert_eq!(out.pixel(x, y).unwrap().alpha(), 0, "四角应透明 ({x},{y})");
        }
    }

    /// 鞘翅 3D：收拢姿态是一片竖长的壳，居中且撑到画布高度
    #[test]
    fn test_elytra_3d_folded() {
        let image = make_texture(|x, y| (36..46).contains(&x) && (2..22).contains(&y));

        let out = elytra(&image, 0.0, 0.0).expect("鞘翅 3D 应出图");
        assert_eq!((out.width(), out.height()), (SIZE, SIZE));
        assert!(opaque(&out) > 5_000, "两片翼应有可观面积");

        // 收拢后竖长，最长边撑到画布的 FILL
        let (width, height) = spans(&out);
        assert!(height > width, "收拢姿态应竖长，实际 {width} x {height}");
        assert!(
            height as f32 >= SIZE as f32 * FILL - 4.0,
            "最长边应撑到画布的 FILL，实际高 {height}"
        );

        // 居中：中列有像素，左右边缘留白
        let middle = SIZE / 2;
        assert!(
            (0..SIZE).any(|y| out.pixel(middle, y).unwrap().alpha() > 0),
            "正中应画到翼"
        );
        for x in [0, SIZE - 1] {
            assert!(
                (0..SIZE).all(|y| out.pixel(x, y).unwrap().alpha() == 0),
                "边缘 {x} 应留白"
            );
        }
    }

    /// 角度是入参：换个偏航角结果必须不同
    #[test]
    fn test_cape_3d_applies_angle() {
        let image = make_texture(|x, y| (1..11).contains(&x) && (1..17).contains(&y));

        let front = cape(&image, 0.0, 0.0).expect("应出图");
        let turned = cape(&image, 0.0, 40.0).expect("应出图");

        assert_ne!(front.data(), turned.data(), "偏航角变化后画面必须变化");
    }

    /// 两片翼都要画出来：收拢时两片叠在一起，正中那一列也该有像素
    #[test]
    fn test_elytra_3d_two_wings() {
        let image = make_texture(|x, y| (22..46).contains(&x) && (0..22).contains(&y));

        let out = elytra(&image, 0.0, 0.0).expect("鞘翅 3D 应出图");

        let middle = SIZE / 2;
        let drawn = (0..SIZE)
            .filter(|y| out.pixel(middle, *y).unwrap().alpha() > 0)
            .count();
        assert!(drawn > 20, "正中应有翼，实际不透明 {drawn} 像素");
    }
}
