// 皮肤 3D 渲染 shader
//
// 顶点已由 CPU 按各部件矩阵变换到世界坐标，这里只剩投影与贴图光照，
// 因此 uniform 里只有一个「投影 * 视图」矩阵。

struct Uniforms {
    // 投影乘视图矩阵，列主序
    mvp: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // 贴图坐标，原点在贴图左上角，已归一化到 0..1
    @location(0) uv: vec2<f32>,
    // 世界坐标，用于算光照方向
    @location(1) world: vec3<f32>,
    // 世界坐标法线，单位向量
    @location(2) normal: vec3<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
) -> VsOut {
    var out: VsOut;
    out.clip = uniforms.mvp * vec4<f32>(pos, 1.0);
    out.uv = uv;
    out.world = pos;
    out.normal = normal;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // 与旧 GLSL 同一套光照：环境光 0.15 加位于 (0, 1, 5) 的点光源漫反射，
    // 光源颜色为白，已折进系数
    let light_dir = normalize(vec3<f32>(0.0, 1.0, 5.0) - in.world);
    let diff = max(dot(normalize(in.normal), light_dir), 0.0);
    let texel = textureSample(tex, tex_sampler, in.uv);

    return vec4<f32>(texel.rgb * (0.15 + diff), texel.a);
}
