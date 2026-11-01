use super::*;

/// get_square_default 应返回标准立方体的 24 个顶点共 72 个分量，
/// 每个分量只可能是 ±0.5，对应边长为 1 的立方体
#[test]
fn test_get_square_default() {
    let v = get_square_default();
    assert_eq!(v.len(), 72, "24 个顶点 x 3 分量");
    for c in &v {
        assert!(*c == VALUE || *c == -VALUE, "立方体分量应为 ±0.5，实际 {c}");
    }
    // 顶点数应为 24
    assert_eq!(v.len() / 3, 24);
}

/// get_square 的乘数 / 偏移 / 放大系数应按分量轴正确应用
#[test]
fn test_get_square_transform() {
    // x 方向：CUBE[0] = 0.5 -> 0.5 * 1(enlarge 2) * 2(mult) + 10 = 12
    let v = get_square(2.0, 1.0, 1.0, 10.0, 0.0, 5.0, 2.0);
    assert_eq!(v[0], 0.5 * 2.0 * 2.0 + 10.0);
    // y 方向：CUBE[1] = 0.5 -> 0.5 * 2 * 1 + 0 = 1
    assert_eq!(v[1], 0.5 * 2.0 * 1.0 + 0.0);
    // z 方向：CUBE[2] = -0.5 -> -0.5 * 2 * 1 + 5 = 4
    assert_eq!(v[2], -0.5 * 2.0 * 1.0 + 5.0);
}

/// get_square_indices 应在原有索引基础上叠加偏移
#[test]
fn test_get_square_indices() {
    let idx = get_square_indices(0);
    assert_eq!(idx.len(), 36, "6 个面 x 2 个三角形 x 3 个索引");
    // 索引最大值不应超过 23，即 24 个顶点
    assert_eq!(*idx.iter().max().unwrap(), 23);

    // 偏移 7：前 6 个索引变为 7,8,9,7,9,10
    let idx7 = get_square_indices(7);
    assert_eq!(&idx7[..6], &[7u16, 8, 9, 7, 9, 10]);

    // 默认索引的前 6 个应为 0,1,2,0,2,3
    let def = get_square_indices_default();
    assert_eq!(&def[..6], &[0u16, 1, 2, 0, 2, 3]);
}

/// 法线数组 VERTICES：72 个分量，取值只应为 -1 / 0 / 1
#[test]
fn test_vertices_normals() {
    assert_eq!(VERTICES.len(), 72);
    for n in &VERTICES {
        assert!(*n == -1.0 || *n == 0.0 || *n == 1.0, "法线分量 {n}");
    }
}

