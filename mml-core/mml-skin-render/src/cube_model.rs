//! 立方体模型数据结构模块
//!
//! 定义由立方体拼装的角色模型 [`SteveModel`] 与对应的贴图 UV 数据 [`SteveTexture`]。

/// 单个部件的方块模型数据，含顶点与索引
#[derive(Debug, Clone)]
pub struct CubeModelItemObj {
    /// 顶点坐标数据，每个顶点 x / y / z 三个分量，单位同 `cube::VALUE`
    pub model: Vec<f32>,
    /// 三角面索引数据，指向 `model` 中的顶点
    pub point: Vec<u16>,
}

impl CubeModelItemObj {
    /// 创建方块模型数据
    ///
    /// - `model`: 顶点坐标数据，每个顶点三个分量
    /// - `point`: 三角面索引数据
    pub fn new(model: Vec<f32>, point: Vec<u16>) -> Self {
        Self { model, point }
    }
}

/// 史蒂夫角色模型，按部件存放方块模型数据
#[derive(Debug, Clone)]
pub struct SteveModel {
    /// 头部
    pub head: CubeModelItemObj,
    /// 身体
    pub body: CubeModelItemObj,
    /// 左臂
    pub left_arm: CubeModelItemObj,
    /// 右臂
    pub right_arm: CubeModelItemObj,
    /// 左腿
    pub left_leg: CubeModelItemObj,
    /// 右腿
    pub right_leg: CubeModelItemObj,
    /// 披风
    pub cape: CubeModelItemObj,
}

impl SteveModel {
    /// 创建史蒂夫模型
    ///
    /// - `head`: 头部模型
    /// - `body`: 身体模型
    /// - `left_arm`: 左臂模型
    /// - `right_arm`: 右臂模型
    /// - `left_leg`: 左腿模型
    /// - `right_leg`: 右腿模型
    /// - `cape`: 披风模型
    pub fn new(
        head: CubeModelItemObj,
        body: CubeModelItemObj,
        left_arm: CubeModelItemObj,
        right_arm: CubeModelItemObj,
        left_leg: CubeModelItemObj,
        right_leg: CubeModelItemObj,
        cape: CubeModelItemObj,
    ) -> Self {
        Self {
            head,
            body,
            left_arm,
            right_arm,
            left_leg,
            right_leg,
            cape,
        }
    }
}

/// 模型贴图数据
///
/// 存储各部件的 UV 坐标，已归一化到 0..1，每个顶点 u / v 两个分量。
#[derive(Debug, Clone)]
pub struct SteveTexture {
    /// 头部 UV
    pub head: Vec<f32>,
    /// 身体 UV
    pub body: Vec<f32>,
    /// 左臂 UV
    pub left_arm: Vec<f32>,
    /// 右臂 UV
    pub right_arm: Vec<f32>,
    /// 左腿 UV
    pub left_leg: Vec<f32>,
    /// 右腿 UV
    pub right_leg: Vec<f32>,
    /// 披风 UV
    pub cape: Vec<f32>,
}

impl SteveTexture {
    /// 创建空的贴图数据
    pub fn new() -> Self {
        Self {
            head: Vec::new(),
            body: Vec::new(),
            left_arm: Vec::new(),
            right_arm: Vec::new(),
            left_leg: Vec::new(),
            right_leg: Vec::new(),
            cape: Vec::new(),
        }
    }
}

impl Default for CubeModelItemObj {
    fn default() -> Self {
        Self {
            model: Vec::new(),
            point: Vec::new(),
        }
    }
}

impl Default for SteveTexture {
    fn default() -> Self {
        Self {
            head: Vec::new(),
            body: Vec::new(),
            left_arm: Vec::new(),
            right_arm: Vec::new(),
            left_leg: Vec::new(),
            right_leg: Vec::new(),
            cape: Vec::new(),
        }
    }
}
