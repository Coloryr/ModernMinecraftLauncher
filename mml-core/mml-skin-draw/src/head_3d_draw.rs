//! 头像的 3D 立方体渲染
//!
//! 把皮肤贴图的 12 个面，即 6 个基础面加 6 个 1.125 倍头顶层，投影到
//! 屏幕空间逐三角形填充，最后超采样降采样得到抗锯齿结果。

use std::f32::consts::PI;

use glam::{Mat4, Vec3, Vec4};
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, Paint, PathBuilder, Pattern, Pixmap, SpreadMode, Transform,
};

/// 立方体 16 个顶点：前 8 个是基础层、后 8 个是 1.125 倍的头顶层，
/// 坐标为相对中心的三向偏移，以半边长 1 为基准
static CUBE_VERTICES: [[f32; 3]; 16] = [
    // 前面
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
    // 背面
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    // 前面，1.125 倍缩放的顶层
    [-1.125, -1.125, 1.125],
    [1.125, -1.125, 1.125],
    [1.125, 1.125, 1.125],
    [-1.125, 1.125, 1.125],
    // 背面，顶层
    [-1.125, -1.125, -1.125],
    [1.125, -1.125, -1.125],
    [1.125, 1.125, -1.125],
    [-1.125, 1.125, -1.125],
];

/// 面朝下视角的绘制顺序：每面 4 个顶点，按远 → 近排列
static CUBE_INDICES: [usize; 48] = [
    8, 12, 15, 11, // 背面，顶层
    8, 12, 13, 9, // 底面，顶层
    8, 9, 10, 11, // 右面，顶层
    0, 4, 7, 3, // 背面
    0, 4, 5, 1, // 底面
    0, 1, 2, 3, // 右面
    3, 7, 6, 2, // 顶面
    4, 5, 6, 7, // 左面
    1, 5, 6, 2, // 前面
    11, 15, 14, 10, // 顶面，顶层
    12, 13, 14, 15, // 左面，顶层
    9, 13, 14, 10, // 前面，顶层
];

/// 面朝上视角的绘制顺序：与 `CUBE_INDICES` 相比顶 / 底面的远近对调，
/// 被挡的顶面先画、可见的底面后画；顶层与基础层的顶 / 底各自对调
static CUBE_INDICES_UP: [usize; 48] = [
    8, 12, 15, 11, // 背面，顶层
    11, 15, 14, 10, // 顶面，顶层
    8, 9, 10, 11, // 右面，顶层
    0, 4, 7, 3, // 背面
    3, 7, 6, 2, // 顶面
    0, 1, 2, 3, // 右面
    0, 4, 5, 1, // 底面
    4, 5, 6, 7, // 左面
    1, 5, 6, 2, // 前面
    8, 12, 13, 9, // 底面，顶层
    12, 13, 14, 15, // 左面，顶层
    9, 13, 14, 10, // 前面，顶层
];

/// 每个面在皮肤贴图上的左上角，单位像素，每面 8x8
static FACE_POS: [[i32; 2]; 12] = [
    [56, 8], // 背面，顶层
    [48, 0], // 底面，顶层
    [48, 8], // 右面，顶层
    [24, 8], // 背面
    [16, 0], // 底面
    [16, 8], // 右面
    [8, 0],  // 顶面
    [0, 8],  // 左面
    [8, 8],  // 前面
    [40, 0], // 顶面，顶层
    [32, 8], // 左面，顶层
    [40, 8], // 前面，顶层
];

/// 面朝上视角的贴图位置，与 `CUBE_INDICES_UP` 一一对应
static FACE_POS_UP: [[i32; 2]; 12] = [
    [56, 8], // 背面，顶层
    [40, 0], // 顶面，顶层
    [48, 8], // 右面，顶层
    [24, 8], // 背面
    [8, 0],  // 顶面
    [16, 8], // 右面
    [16, 0], // 底面
    [0, 8],  // 左面
    [8, 8],  // 前面
    [48, 0], // 底面，顶层
    [32, 8], // 左面，顶层
    [40, 8], // 前面，顶层
];

/// 每个面四个角的归一化 uv，按面顺序每面 4 个
static SOURCE_VERTICES: [[f32; 2]; 48] = [
    [0.0, 1.0],
    [1.0, 1.0],
    [1.0, 0.0],
    [0.0, 0.0], // 背面
    [1.0, 0.0],
    [0.0, 0.0],
    [0.0, 1.0],
    [1.0, 1.0], // 底面
    [1.0, 1.0],
    [0.0, 1.0],
    [0.0, 0.0],
    [1.0, 0.0], // 右面
    [0.0, 1.0],
    [1.0, 1.0],
    [1.0, 0.0],
    [0.0, 0.0], // 背面
    [1.0, 0.0],
    [0.0, 0.0],
    [0.0, 1.0],
    [1.0, 1.0], // 底面
    [1.0, 1.0],
    [0.0, 1.0],
    [0.0, 0.0],
    [1.0, 0.0], // 右面
    [1.0, 0.0],
    [0.0, 0.0],
    [0.0, 1.0],
    [1.0, 1.0], // 顶面
    [0.0, 1.0],
    [1.0, 1.0],
    [1.0, 0.0],
    [0.0, 0.0], // 左面
    [1.0, 1.0],
    [0.0, 1.0],
    [0.0, 0.0],
    [1.0, 0.0], // 前面
    [1.0, 0.0],
    [0.0, 0.0],
    [0.0, 1.0],
    [1.0, 1.0], // 顶面
    [0.0, 1.0],
    [1.0, 1.0],
    [1.0, 0.0],
    [0.0, 0.0], // 左面
    [1.0, 1.0],
    [0.0, 1.0],
    [0.0, 0.0],
    [1.0, 0.0], // 前面
];

/// 创建固定缩放与平移、可指定角度的模型变换矩阵
///
/// - `pitch`: 俯仰角，单位度，负 = 面朝下视角 / 俯视，正 = 面朝上 / 仰视
/// - `yaw`: 偏航角，单位度
///
/// # 返回值
///
/// 返回模型空间到画布像素空间的变换矩阵，y 轴取负以适配向下为 +y 的画布
fn create_tran(pitch: f32, yaw: f32) -> Mat4 {
    let roty = Mat4::from_rotation_y(yaw * PI / 180.0);
    let rotx = Mat4::from_rotation_x(pitch * PI / 180.0);

    let scale = Mat4::from_scale(Vec3::new(100.0, -100.0, 100.0));

    let tran = Mat4::from_translation(Vec3::new(200.0, 200.0, 0.0));

    tran * scale * rotx * roty
}

/// 投影一个顶点到屏幕坐标
///
/// - `tran`: 模型变换矩阵
/// - `point`: 模型坐标顶点
/// - `enable_z`: 是否启用伪透视，按深度缩放坐标
///
/// # 返回值
///
/// 返回 `(x, y)` 屏幕坐标，单位像素
pub(crate) fn project(tran: &Mat4, point: [f32; 3], enable_z: bool) -> (f32, f32) {
    let mut res = tran * Vec4::new(point[0], point[1], point[2], 1.0);

    if res.w != 0.0 {
        res.x /= res.w;
        res.y /= res.w;
        res.z /= res.w;
    }

    if enable_z {
        let z = res.z * 0.0001 + 1.0;
        res.x /= z;
        res.y /= z;
    }

    (res.x, res.y)
}

/// 输出边长，单位像素，裁剪前为正方形画布
pub(crate) const SIZE: u32 = 400;

/// 超采样倍数
///
/// 不对每个三角形单独开抗锯齿：相邻三角形会在公共边上各贡献一次半覆盖，
/// 叠加后形成细线，每个面的内部对角线和立方体的棱上都会出现。
/// 改为按二进制覆盖绘制、再降采样得到抗锯齿，从根上避开接缝。
pub(crate) const SUPERSAMPLE: u32 = 2;

/// 用一个纹理三角形填充屏幕三角形
///
/// - `pixmap`: 目标位图，超采样尺寸
/// - `texture`: 皮肤贴图
/// - `triangle`: 三个顶点的 `(屏幕坐标, 纹理坐标)`，屏幕坐标单位像素
/// - `scale`: 超采样倍数，屏幕坐标先乘以它
///
/// 三个顶点给出屏幕坐标与纹理坐标的对应关系，据此求出「屏幕 → 纹理」的仿射变换；
/// `Pattern` 的 transform 方向相反，是纹理 → 屏幕，所以传它的逆。
pub(crate) fn fill_triangle(
    pixmap: &mut Pixmap,
    texture: &Pixmap,
    triangle: [((f32, f32), (f32, f32)); 3],
    scale: f32,
) {
    let (p0, t0) = triangle[0];
    let (p1, t1) = triangle[1];
    let (p2, t2) = triangle[2];

    // 屏幕空间的两条边，放大到超采样坐标
    let p0 = (p0.0 * scale, p0.1 * scale);
    let p1 = (p1.0 * scale, p1.1 * scale);
    let p2 = (p2.0 * scale, p2.1 * scale);

    let (ax, ay) = (p1.0 - p0.0, p1.1 - p0.1);
    let (bx, by) = (p2.0 - p0.0, p2.1 - p0.1);
    let det = ax * by - ay * bx;

    // 完全退化的三角形会投影成一条线，让下面的仿射变换不可逆，丢掉
    if det.abs() < f32::EPSILON {
        return;
    }

    // 纹理空间的两条边
    let (ux, uy) = (t1.0 - t0.0, t1.1 - t0.1);
    let (vx, vy) = (t2.0 - t0.0, t2.1 - t0.1);

    let inv = 1.0 / det;
    let m00 = (ux * by - vx * ay) * inv;
    let m01 = (-ux * bx + vx * ax) * inv;
    let m10 = (uy * by - vy * ay) * inv;
    let m11 = (-uy * bx + vy * ax) * inv;
    let m02 = t0.0 - (m00 * p0.0 + m01 * p0.1);
    let m12 = t0.1 - (m10 * p0.0 + m11 * p0.1);

    // 注意 `from_row` 的参数是 (sx, ky, kx, sy, tx, ty)，对应矩阵 | sx kx tx ; ky sy ty |，
    // 所以这里不能按行序传，传错等于把矩阵转置，纹理采样会整片错位
    let Some(texture_to_screen) = Transform::from_row(m00, m10, m01, m11, m02, m12).invert() else {
        return;
    };

    let pattern = Pattern::new(
        texture.as_ref(),
        SpreadMode::Pad,
        FilterQuality::Nearest,
        1.0,
        texture_to_screen,
    );

    let mut paint = Paint::default();
    paint.shader = pattern;
    // 关掉抗锯齿：抗锯齿交给降采样，见 `SUPERSAMPLE` 的说明
    paint.anti_alias = false;
    paint.blend_mode = BlendMode::SourceOver;

    let mut builder = PathBuilder::new();
    builder.move_to(p0.0, p0.1);
    builder.line_to(p1.0, p1.1);
    builder.line_to(p2.0, p2.1);
    builder.close();

    let Some(path) = builder.finish() else {
        return;
    };

    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// 把立方体全部面投影并逐三角形绘制到目标位图
///
/// - `pixmap`: 目标位图，超采样尺寸
/// - `texture`: 皮肤贴图
/// - `indices` / `face_pos`: 面的绘制顺序，几何顶点加贴图位置，按远 → 近排列
/// - `tran`: 模型变换矩阵
/// - `enable_z`: 是否启用伪透视，按深度缩放
/// - `texel_eps`: 贴图坐标向块内收缩的纹素量，0 = 不收缩
/// - `scale`: 超采样倍数
fn draw_texture_faces(
    pixmap: &mut Pixmap,
    texture: &Pixmap,
    indices: &[usize; 48],
    face_pos: &[[i32; 2]; 12],
    tran: &Mat4,
    enable_z: bool,
    texel_eps: f32,
    scale: f32,
) {
    let face_count = indices.len() / 4;

    // 每个面的四边形按 (0,1,2) (0,2,3) 展开为两个三角形
    const TRI_ORDER: [usize; 6] = [0, 1, 2, 0, 2, 3];

    for face in 0..face_count {
        let base = face * 4;
        let pos = face_pos[face];

        // 先把这个面的四个角都投影出来
        let mut corners = [((0.0f32, 0.0f32), (0.0f32, 0.0f32)); 4];
        for (i, corner) in corners.iter_mut().enumerate() {
            let screen = project(tran, CUBE_VERTICES[indices[base + i]], enable_z);
            let src = SOURCE_VERTICES[base + i];
            // 贴图坐标向块内收缩一个极小量：仿射插值的 f32 舍入会让公共边上的像素
            // 越过块边界，Nearest 整格取到相邻块并在轮廓外沿形成 1px 细线；参考实现传 0 以保持逐字节一致
            let tex = (
                pos[0] as f32 + src[0] * 8.0 - (src[0] * 2.0 - 1.0) * texel_eps,
                pos[1] as f32 + src[1] * 8.0 - (src[1] * 2.0 - 1.0) * texel_eps,
            );
            *corner = (screen, tex);
        }

        for tri in 0..2 {
            let idx = [
                TRI_ORDER[tri * 3],
                TRI_ORDER[tri * 3 + 1],
                TRI_ORDER[tri * 3 + 2],
            ];
            fill_triangle(
                pixmap,
                texture,
                [corners[idx[0]], corners[idx[1]], corners[idx[2]]],
                scale,
            );
        }
    }
}

/// 按不透明像素的包围盒裁剪，四周留出 `margin` 像素边距
///
/// - `pixmap`: 待裁剪的位图
/// - `margin`: 内容四周保留的边距，单位像素
///
/// # 返回值
///
/// 返回裁剪后的位图，全透明时返回 `None`
///
/// 立方体只占画布中央一部分，四周的透明边距在账户头像这类小尺寸场景里
/// 会透出底层底色；裁到内容包围盒后头像在任何显示方都是「铺满」的
pub(crate) fn crop_content(pixmap: &Pixmap, margin: u32) -> Option<Pixmap> {
    let width = pixmap.width();
    let height = pixmap.height();
    let data = pixmap.data();

    let mut bounds: Option<(u32, u32, u32, u32)> = None; // (min_x, min_y, max_x, max_y)
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            if data[offset + 3] == 0 {
                continue;
            }
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((min_x, min_y, max_x, max_y)) => {
                    (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                }
            });
        }
    }

    let Some((min_x, min_y, max_x, max_y)) = bounds else {
        return None; // 全透明
    };

    let x0 = min_x.saturating_sub(margin);
    let y0 = min_y.saturating_sub(margin);
    let x1 = (max_x + 1 + margin).min(width);
    let y1 = (max_y + 1 + margin).min(height);
    let w = x1 - x0;
    let h = y1 - y0;

    let mut out = Pixmap::new(w, h)?;
    for y in 0..h {
        let src = (((y0 + y) * width + x0) * 4) as usize;
        let dst = ((y * w) * 4) as usize;
        out.data_mut()[dst..dst + (w * 4) as usize]
            .copy_from_slice(&data[src..src + (w * 4) as usize]);
    }
    Some(out)
}

/// 整数倍降采样，预乘像素直接求平均即可，不需要还原成直乘
///
/// - `src`: 源位图，宽高须为 `factor` 的整数倍
/// - `factor`: 降采样倍数
///
/// # 返回值
///
/// 返回降采样后的位图，采样越界或分配失败时返回 `None`
pub(crate) fn downsample(src: &Pixmap, factor: u32) -> Option<Pixmap> {
    let width = src.width() / factor;
    let height = src.height() / factor;
    let mut out = Pixmap::new(width, height)?;

    let count = (factor * factor) as u32;

    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];

            for dy in 0..factor {
                for dx in 0..factor {
                    let px = src.pixel(x * factor + dx, y * factor + dy)?;
                    sum[0] += px.red() as u32;
                    sum[1] += px.green() as u32;
                    sum[2] += px.blue() as u32;
                    sum[3] += px.alpha() as u32;
                }
            }

            let offset = ((y * width + x) * 4) as usize;
            let dst = &mut out.data_mut()[offset..offset + 4];
            for (i, value) in sum.iter().enumerate() {
                dst[i] = (value / count) as u8;
            }
        }
    }

    Some(out)
}

/// 渲染 3D 头像，输出尺寸为内容包围盒加 2px 边距
///
/// - `image`: 皮肤贴图
/// - `indices` / `face_pos`: 面的绘制顺序，几何顶点加贴图位置，按远 → 近排列
/// - `tran`: 模型变换矩阵
/// - `enable_z`: 是否启用伪透视，按深度缩放
/// - `texel_eps`: 贴图坐标向块内收缩的纹素量，0 = 不收缩并保持参考实现输出
///
/// # 返回值
///
/// 返回裁剪后的渲染结果，分配失败时返回 `None`
fn draw_head(
    image: &Pixmap,
    indices: &[usize; 48],
    face_pos: &[[i32; 2]; 12],
    tran: &Mat4,
    enable_z: bool,
    texel_eps: f32,
) -> Option<Pixmap> {
    let scale = SUPERSAMPLE as f32;
    let size = SIZE * SUPERSAMPLE;

    // 新建的位图是全透明的，与旧实现 clear(透明) 一致
    let mut pixmap = Pixmap::new(size, size)?;

    draw_texture_faces(
        &mut pixmap,
        image,
        indices,
        face_pos,
        tran,
        enable_z,
        texel_eps,
        scale,
    );

    downsample(&pixmap, SUPERSAMPLE).and_then(|out| crop_content(&out, 2))
}

/// 渲染固定角度的 3D 头像，面朝上即仰视，不启用伪透视
///
/// 绘制顺序用 `CUBE_INDICES_UP` / `FACE_POS_UP`：与面朝下视角相比
/// 顶 / 底面的远近对调，被挡的顶面先画、可见的底面后画
///
/// - `image`: 皮肤贴图
///
/// # 返回值
///
/// 返回 `SIZE` x `SIZE` 的渲染结果，分配失败时返回 `None`
pub fn draw_head_3d_typea_up(image: &Pixmap) -> Option<Pixmap> {
    let tran = create_tran(30.0, 45.0);
    draw_head(image, &CUBE_INDICES_UP, &FACE_POS_UP, &tran, false, 0.001)
}

/// 渲染固定角度的 3D 头像，面朝下即俯视，不启用伪透视
///
/// - `image`: 皮肤贴图
///
/// # 返回值
///
/// 返回 `SIZE` x `SIZE` 的渲染结果，分配失败时返回 `None`
pub fn draw_head_3d_typea_down(image: &Pixmap) -> Option<Pixmap> {
    let tran = create_tran(-30.0, 45.0);
    draw_head(image, &CUBE_INDICES, &FACE_POS, &tran, false, 0.0)
}

/// 渲染可指定角度的 3D 头像，启用伪透视
///
/// - `image`: 皮肤贴图
/// - `x`: 俯仰角，单位度
/// - `y`: 偏航角，单位度
///
/// # 返回值
///
/// 返回 `SIZE` x `SIZE` 的渲染结果，分配失败时返回 `None`
pub fn draw_head_3d_typeb(image: &Pixmap, x: f32, y: f32) -> Option<Pixmap> {
    let tran = create_tran(-x, y);
    draw_head(image, &CUBE_INDICES, &FACE_POS, &tran, true, 0.0)
}
