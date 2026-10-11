// 26.3 GUI 物品渲染 shader，照 item.vsh/item.fsh + light.glsl
// 顶点：MVP 变换 + 法线矩阵；片元：双方向半兰伯特光照，cutout 带阈值 discard

struct Uniforms {
    mvp: mat4x4<f32>,
    normal_mat: mat4x4<f32>,
    light0: vec4<f32>,
    light1: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    // glint 附魔光效的采样坐标：RotZ(π/18)·S(8)·uv，t=0 帧无平移偏移
    @location(3) glint_uv: vec2<f32>,
    // 1.0 表示自发光，火焰等发光物品不参与光照衰减
    @location(4) fullbright: f32,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) fullbright: f32,
) -> VsOut {
    var out: VsOut;
    out.pos = u.mvp * vec4<f32>(pos, 1.0);
    out.uv = uv;
    out.normal = normalize((u.normal_mat * vec4<f32>(normal, 0.0)).xyz);
    out.color = color;
    out.fullbright = fullbright;
    // glint 坐标：先放大 8 倍再绕 z 转 10°，静态图标取 t=0，无平移偏移
    let g = uv * 8.0;
    let a = 0.17453292519943295; // π/18
    out.glint_uv = vec2<f32>(g.x * cos(a) - g.y * sin(a), g.x * sin(a) + g.y * cos(a));
    return out;
}

// light.glsl: minecraft_mix_light —— lightmap 全亮 15728880 时等价于直接乘
fn light_acc(n: vec3<f32>, color: vec4<f32>, fullbright: f32) -> vec4<f32> {
    let l = max(
        vec2<f32>(dot(u.light0.xyz, n), dot(u.light1.xyz, n)),
        vec2<f32>(0.0),
    );
    let accum = mix(min(1.0, (l.x + l.y) * 0.6 + 0.4), 1.0, fullbright);
    return vec4<f32>(color.rgb * accum, color.a);
}

// ITEM_CUTOUT：ALPHA_CUTOUT=0.1，无混合
@fragment
fn fs_cutout(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    if (c.a < 0.1) {
        discard;
    }
    return c * light_acc(in.normal, in.color, in.fullbright);
}

// ITEM_TRANSLUCENT：无 discard，SrcAlpha 混合
@fragment
fn fs_translucent(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    return c * light_acc(in.normal, in.color, in.fullbright);
}

// GLINT 附魔光效：加色混合在物品像素上叠加光纹；
// 采样坐标用 glint_uv，REPEAT 采样；depth EQUAL 只画物品已写像素
@fragment
fn fs_glint(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.glint_uv);
    if (c.a < 0.1) {
        discard;
    }
    return vec4<f32>(c.rgb, c.a);
}
