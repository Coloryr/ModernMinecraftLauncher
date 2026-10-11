//! 立方体几何基元模块
//!
//! 提供单位立方体的顶点坐标、法线与三角面索引数据，
//! 以及按轴缩放 / 偏移生成方块顶点数据的工具函数。

/// 方块模型的半边长，立方体边长为 1
pub const VALUE: f32 = 0.5;

/// 立方体每个顶点的法线数据，24 个顶点 × 3 分量，取值 -1 / 0 / 1
pub const VERTICES: [f32; 72] = [
    0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0,
    0.0, 0.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0,
    1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0,
    1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0,
];

/// 立方体 24 个顶点的坐标数据，6 面 × 4 顶点 × 3 分量，单位半边长 VALUE
const CUBE: [f32; 72] = [
    VALUE, VALUE, -VALUE, /* 背面。 */
    VALUE, -VALUE, -VALUE, -VALUE, -VALUE, -VALUE, -VALUE, VALUE, -VALUE, -VALUE, VALUE,
    VALUE, /* 前面。 */
    -VALUE, -VALUE, VALUE, VALUE, -VALUE, VALUE, VALUE, VALUE, VALUE, -VALUE, VALUE,
    -VALUE, /* 左面。 */
    -VALUE, -VALUE, -VALUE, -VALUE, -VALUE, VALUE, -VALUE, VALUE, VALUE, VALUE, VALUE,
    VALUE, /* 右面。 */
    VALUE, -VALUE, VALUE, VALUE, -VALUE, -VALUE, VALUE, VALUE, -VALUE, -VALUE, VALUE,
    -VALUE, /* 顶面。 */
    -VALUE, VALUE, VALUE, VALUE, VALUE, VALUE, VALUE, VALUE, -VALUE, VALUE, -VALUE,
    -VALUE, /* 底面。 */
    VALUE, -VALUE, VALUE, -VALUE, -VALUE, VALUE, -VALUE, -VALUE, -VALUE,
];

/// 立方体的三角面索引，6 面 × 2 三角形 × 3 索引
const CUBE_INDICES: [u16; 36] = [
    0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16, 17, 18,
    16, 18, 19, 20, 21, 22, 20, 22, 23,
];

/// 按轴乘数、偏移与放大系数变换出方块顶点坐标
///
/// - `multiply_x`: X 轴乘数
/// - `multiply_y`: Y 轴乘数
/// - `multiply_z`: Z 轴乘数
/// - `add_x`: X 轴偏移
/// - `add_y`: Y 轴偏移
/// - `add_z`: Z 轴偏移
/// - `enlarge`: 放大系数，先作用于单位立方体再乘轴乘数
///
/// # 返回值
///
/// 返回变换后的顶点坐标数组，每个顶点 x / y / z 三个分量，单位同 `VALUE`
pub fn get_square(
    multiply_x: f32,
    multiply_y: f32,
    multiply_z: f32,
    add_x: f32,
    add_y: f32,
    add_z: f32,
    enlarge: f32,
) -> Vec<f32> {
    let mut temp = vec![0.0; CUBE.len()];

    for (a, value) in temp.iter_mut().enumerate() {
        *value = CUBE[a] * enlarge;

        match a % 3 {
            0 => *value = *value * multiply_x + add_x,
            1 => *value = *value * multiply_y + add_y,
            _ => *value = *value * multiply_z + add_z,
        }
    }

    temp
}

/// 生成标准方块的三角面索引，可整体叠加偏移
///
/// - `offset`: 顶点索引偏移量，加到每个索引上
///
/// # 返回值
///
/// 返回偏移后的三角面索引数组
pub fn get_square_indices(offset: u16) -> Vec<u16> {
    let mut temp = vec![0; CUBE_INDICES.len()];

    for (a, value) in temp.iter_mut().enumerate() {
        *value = CUBE_INDICES[a] + offset;
    }

    temp
}

/// 用默认参数取标准立方体的顶点坐标
///
/// # 返回值
///
/// 返回标准立方体的顶点坐标数组
pub fn get_square_default() -> Vec<f32> {
    get_square(1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0)
}

/// 用默认参数取标准立方体的三角面索引
///
/// # 返回值
///
/// 返回无偏移的三角面索引数组
pub fn get_square_indices_default() -> Vec<u16> {
    get_square_indices(0)
}

#[cfg(test)]
mod tests {
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
}
