use tiny_skia::IntSize;

use super::*;

/// 造一张纯色不透明皮肤
fn skin_bitmap(color: [u8; 4]) -> Pixmap {
    let mut data = vec![0u8; 64 * 64 * 4];
    for px in data.chunks_exact_mut(4) {
        px.copy_from_slice(&color);
    }

    Pixmap::from_vec(data, IntSize::from_wh(64, 64).unwrap()).unwrap()
}

/// 取像素的直乘 RGBA
fn pixel(image: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let px = image.pixel(x, y).unwrap().demultiply();

    [px.red(), px.green(), px.blue(), px.alpha()]
}

/// 造一个渲染器；无可用 GPU 时返回 `None`，用例随之跳过
fn renderer() -> Option<SkinRenderWgpu> {
    mml_gpu::device()?;

    Some(SkinRenderWgpu::new())
}

/// 装载一张纯色新版宽臂皮肤并设好画布
fn prepare(render: &mut SkinRenderWgpu, size: i32, color: [u8; 4]) {
    render.width = size;
    render.height = size;
    render.skin_type = SkinType::New;
    render.set_skin_tex(&skin_bitmap(color)).unwrap();
}

/// 统计画面里非背景色的像素数，背景为不透明黑
fn drawn_pixels(image: &Pixmap) -> usize {
    (0..image.height())
        .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(image, *x, *y) != [0, 0, 0, 255])
        .count()
}

/// 加载皮肤后应把模型画在背景上：画面中心出现非背景像素，四角仍是背景
#[test]
fn render_draws_model_over_background() {
    let Some(mut render) = renderer() else {
        return;
    };
    prepare(&mut render, 128, [80, 160, 80, 255]);

    let image = render.render().expect("有皮肤且尺寸为正时应渲染成功");
    assert_eq!((image.width(), image.height()), (128, 128));

    let drawn = drawn_pixels(&image);
    assert!(drawn > 1000, "模型应覆盖画面中心，实际非背景像素 {drawn}");

    // 四角保持背景色，说明模型没有铺满整张画布
    for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127)] {
        assert_eq!(pixel(&image, x, y), [0, 0, 0, 255], "({x},{y}) 应是背景");
    }
}

/// MSAA 档位也应渲染出模型
#[test]
fn render_msaa_draws_model() {
    let Some(mut render) = renderer() else {
        return;
    };
    prepare(&mut render, 64, [200, 60, 60, 255]);
    render.render_type = SkinRenderType::MSAA;

    let image = render.render().expect("MSAA 档位应渲染成功");
    assert_eq!((image.width(), image.height()), (64, 64));

    let drawn = drawn_pixels(&image);
    assert!(drawn > 200, "MSAA 下模型应可见，实际非背景像素 {drawn}");
}

/// 自转矩阵必须真的进到画面里：转 90 度后应与正面视图不同
#[test]
fn render_applies_rotation() {
    let Some(mut render) = renderer() else {
        return;
    };
    prepare(&mut render, 128, [80, 160, 80, 255]);

    let front = render.render().expect("正面应渲染成功");

    render.rotate(0.0, 90.0);
    let side = render.render().expect("转 90 度后应渲染成功");

    assert_ne!(front.data(), side.data(), "转动后画面必须变化");
}

/// 没有皮肤时按 NoSkin 报错
#[test]
fn render_without_skin_is_rejected() {
    let Some(render) = renderer() else {
        return;
    };

    assert_eq!(render.render(), Err(ErrorType::NoSkin));
}

/// 皮肤类型未知时按 NoSkin 报错，且每次渲染都给同样的答案
#[test]
fn render_unknown_skin_is_rejected() {
    let Some(mut render) = renderer() else {
        return;
    };

    // set_skin_tex 只查宽度，类型未知由渲染阶段判
    render
        .set_skin_tex(&skin_bitmap([10, 20, 30, 255]))
        .unwrap();

    assert_eq!(render.render(), Err(ErrorType::NoSkin));
    assert_eq!(render.render(), Err(ErrorType::NoSkin));
}

/// 画布尺寸非正时按 InvalidSize 报错
#[test]
fn render_zero_size_is_rejected() {
    let Some(mut render) = renderer() else {
        return;
    };
    prepare(&mut render, 0, [0, 0, 255, 255]);

    assert_eq!(render.render(), Err(ErrorType::InvalidSize));
}

