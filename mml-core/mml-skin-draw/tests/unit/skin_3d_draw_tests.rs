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

