// 皮肤 3D 渲染 shader：MVP 变换 + nearest 采样
// 底层 fs_base 镂空 discard、无混合、写深度；顶层 fs_overlay SrcAlpha 混合、只读深度

struct Uniforms {
    mvp: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    out.pos = u.mvp * vec4<f32>(pos, 1.0);
    out.uv = uv;
    return out;
}

// 底层：ALPHA_CUTOUT 语义，镂空 discard，无混合
@fragment
fn fs_base(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    if (c.a < 0.1) {
        discard;
    }
    return c;
}

// 顶层：半透明 SrcAlpha 混合，镂空同样 discard，避免透明片写混合
@fragment
fn fs_overlay(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    if (c.a < 0.1) {
        discard;
    }
    return c;
}
