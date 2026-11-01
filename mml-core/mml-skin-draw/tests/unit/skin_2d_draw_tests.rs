use super::*;
use tiny_skia::IntSize;

/// 创建位图并对每个像素调用填充函数写入 4 字节
fn make_bitmap(w: u32, h: u32, fill: impl Fn(i32, i32) -> [u8; 4]) -> Pixmap {
    let mut data = vec![0u8; (w * h) as usize * BPP];
    for (i, chunk) in data.chunks_exact_mut(BPP).enumerate() {
        let (x, y) = (i as i32 % w as i32, i as i32 / w as i32);
        chunk.copy_from_slice(&fill(x, y));
    }

    Pixmap::from_vec(data, IntSize::from_wh(w, h).unwrap()).unwrap()
}

/// 读取位图某像素的原始 4 字节
fn get_bytes(bm: &Pixmap, x: i32, y: i32) -> [u8; 4] {
    get_pixel(bm, x, y)
}

/// 新版 1.8+ 皮肤的 2D 展开：各部件应落在 16x32 中间画布的对应区域，
/// 缩放 8 倍后为 128x256。透明 overlay 不应破坏底层的颜色。
#[test]
fn test_skin_2d_draw_typea_new_layout() {
    // 构造 64x64 新版皮肤：每个部件用唯一的字节模式标记，overlay 区域全透明
    let image = make_bitmap(64, 64, |x, y| {
        let in_area = |ax: i32, ay: i32, aw: i32, ah: i32| {
            x >= ax && x < ax + aw && y >= ay && y < ay + ah
        };
        // 底层部件
        if in_area(8, 8, 8, 8) {
            [10, 0, 0, 255] // 头部正面 (8,8,8,8)
        } else if in_area(20, 20, 8, 12) {
            [20, 0, 0, 255] // 身体 (20,20,8,12)
        } else if in_area(44, 20, 4, 12) {
            [30, 0, 0, 255] // 右手 (44,20,4,12)
        } else if in_area(36, 52, 4, 12) {
            [40, 0, 0, 255] // 左手 (36,52,4,12)
        } else if in_area(4, 20, 4, 12) {
            [50, 0, 0, 255] // 右腿 (4,20,4,12)
        } else if in_area(20, 52, 4, 12) {
            [60, 0, 0, 255] // 左腿 (20,52,4,12)
        } else {
            [0, 0, 0, 0] // 其余含所有 overlay 区域，透明
        }
    });

    let out = typea(&image, Some(SkinType::New)).expect("typea 2D 展开应成功");
    assert_eq!(out.width(), 128, "typea 输出宽度应为 16 * 8");
    assert_eq!(out.height(), 256, "typea 输出高度应为 32 * 8");

    // 16x32 中间画布上的目标区域 -> 缩放 8 倍后的输出区域取中心点校验
    // 头部 (4,0,8,8) -> 输出中心 (64, 32)
    assert_eq!(get_bytes(&out, 64, 32), [10, 0, 0, 255], "头部");
    // 身体 (4,8,8,12) -> 输出中心 (64, 128)
    assert_eq!(get_bytes(&out, 64, 128), [20, 0, 0, 255], "身体");
    // 右手 (0,8,4,12) -> 输出中心 (16, 128)
    assert_eq!(get_bytes(&out, 16, 128), [30, 0, 0, 255], "右手");
    // 左手 (12,8,4,12) -> 输出中心 (112, 128)
    assert_eq!(get_bytes(&out, 112, 128), [40, 0, 0, 255], "左手");
    // 右腿 (4,20,4,12) -> 输出中心 (48, 208)
    assert_eq!(get_bytes(&out, 48, 208), [50, 0, 0, 255], "右腿");
    // 左腿 (8,20,4,12) -> 输出中心 (80, 208)
    assert_eq!(get_bytes(&out, 80, 208), [60, 0, 0, 255], "左腿");

    // 部件之间的空隙应保持透明，如 16x32 画布的 (0,0) 对应输出 (0,0)
    assert_eq!(get_bytes(&out, 0, 0), [0, 0, 0, 0], "空隙应透明");
}

/// 纤细皮肤的右手只有 3 像素宽，左手位置与普通皮肤一致
#[test]
fn test_skin_2d_draw_typea_slim_layout() {
    let image = make_bitmap(64, 64, |x, y| {
        let in_area = |ax: i32, ay: i32, aw: i32, ah: i32| {
            x >= ax && x < ax + aw && y >= ay && y < ay + ah
        };
        if in_area(8, 8, 8, 8) {
            [10, 0, 0, 255] // 头部正面
        } else if in_area(44, 20, 3, 12) {
            [30, 0, 0, 255] // 纤细右手 (44,20,3,12)
        } else if in_area(36, 52, 3, 12) {
            [40, 0, 0, 255] // 纤细左手 (36,52,3,12)
        } else {
            [0, 0, 0, 0]
        }
    });

    let out = typea(&image, Some(SkinType::NewSlim)).expect("纤细皮肤 typea 展开应成功");
    assert_eq!(out.width(), 128);
    assert_eq!(out.height(), 256);

    assert_eq!(get_bytes(&out, 64, 32), [10, 0, 0, 255], "头部");
    // 纤细右手放在 (1,8,3,12) -> 输出中心 (20, 128)
    assert_eq!(get_bytes(&out, 20, 128), [30, 0, 0, 255], "纤细右手");
    // 纤细左手放在 (12,8,3,12) -> 输出中心 (112, 128)
    assert_eq!(get_bytes(&out, 112, 128), [40, 0, 0, 255], "纤细左手");
}

