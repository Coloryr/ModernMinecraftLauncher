//! OpenGL 模型缓冲对象模块
//!
//! 封装角色模型各部件的 VAO/VBO 缓冲与顶点布局。

use glam::{Vec2, Vec3};
use glow::{Buffer, Context, HasContext, VertexArray};

/// 一个部件的顶点缓冲对象集合
pub struct VaoItem {
    /// 顶点坐标缓冲
    pub vertex_buffer_object: Buffer,
    /// 三角面索引缓冲
    pub index_buffer_object: Buffer,
    /// 顶点数组对象
    pub vertex_array_object: VertexArray,
}

impl VaoItem {
    /// 创建缓冲对象
    ///
    /// - `gl`: OpenGL 上下文
    pub fn new(gl: &Context) -> Self {
        VaoItem {
            vertex_buffer_object: unsafe { gl.create_buffer().unwrap() },
            index_buffer_object: unsafe { gl.create_buffer().unwrap() },
            vertex_array_object: unsafe { gl.create_vertex_array().unwrap() },
        }
    }

    /// 删除缓冲对象
    ///
    /// - `gl`: OpenGL 上下文
    pub fn delete(&self, gl: &Context) {
        unsafe {
            gl.delete_vertex_array(self.vertex_array_object);
            gl.delete_buffer(self.vertex_buffer_object);
            gl.delete_buffer(self.index_buffer_object);
        };
    }
}

/// 一个模型的全部部件缓冲
pub struct ModelVao {
    /// 头部
    pub head: VaoItem,
    /// 身体
    pub body: VaoItem,
    /// 左臂
    pub left_arm: VaoItem,
    /// 右臂
    pub right_arm: VaoItem,
    /// 左腿
    pub left_leg: VaoItem,
    /// 右腿
    pub right_leg: VaoItem,
    /// 披风
    pub cape: VaoItem,
}

impl ModelVao {
    /// 创建全部部件的缓冲对象
    ///
    /// - `gl`: OpenGL 上下文
    pub fn new(gl: &Context) -> Self {
        ModelVao {
            head: VaoItem::new(gl),
            body: VaoItem::new(gl),
            left_arm: VaoItem::new(gl),
            right_arm: VaoItem::new(gl),
            left_leg: VaoItem::new(gl),
            right_leg: VaoItem::new(gl),
            cape: VaoItem::new(gl),
        }
    }

    /// 删除全部部件的缓冲对象
    ///
    /// - `gl`: OpenGL 上下文
    pub fn delete(&self, gl: &Context) {
        self.head.delete(gl);
        self.body.delete(gl);
        self.left_arm.delete(gl);
        self.right_arm.delete(gl);
        self.left_leg.delete(gl);
        self.right_leg.delete(gl);
        self.cape.delete(gl);
    }
}

/// 上传到 OpenGL 的顶点布局，与着色器中的 attrib 布局对应
#[repr(C)]
pub struct VertexOpenGL {
    /// 顶点坐标，模型坐标
    pub pos: Vec3,
    /// 贴图 UV，已归一化到 0..1
    pub uv: Vec2,
    /// 顶点法线，取值为单位轴向量
    pub normal: Vec3,
}
