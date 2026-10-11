//! GPU 渲染测试：每后端渲染同一组模型出图对照，含方块模型、extrude 物品与 glint 物品
//!
//! 每个后端一个用例，可单独过滤运行；VK 与 GL 在部分机器上可能无适配器，失败只打印跳过：
//! `cargo test -p mml-tex-draw --test render_gpu -- --ignored --nocapture render_dx12`
//! 环境变量 MML_TEST_JAR 指定客户端 jar，缺省用反编译参考 jar
//! 输出：tests/out/{furnace,apple,enchanted_book}_{DX12,VULKAN,GL}.png

use std::{collections::HashMap, path::PathBuf, sync::Once};

use mml_base::archives::BaseArchive;
use mml_tex_draw::{
    gpu::GpuCtx,
    model::{BakedModel, BakedModelTextures, bake_model},
};

/// 取参考客户端 jar 路径，环境变量 MML_TEST_JAR 优先
fn ref_jar() -> PathBuf {
    std::env::var("MML_TEST_JAR").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(std::env::var("TEMP").unwrap_or_default())
            .join("mml-262-ref")
            .join("client.jar")
    })
}

/// 取 tests/out 目录，不存在时创建
fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("out");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// item::render_item 写盘需要 item 输出目录，每进程初始化一次
fn ensure_item_dir() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        mml_tex_draw::init(out_dir()).unwrap();
    });
}

/// 把 RGBA 像素写成 size×size 的 PNG 到 tests/out
fn write_png(name: &str, size: u32, rgba: &[u8]) {
    let path = out_dir().join(name);
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = png::Encoder::new(file, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
    println!("输出：{}", path.display());
}

/// 烘焙对照方块模型 furnace，多贴图元素模型
fn bake_furnace() -> BakedModelTextures {
    let jar = ref_jar();
    assert!(jar.exists(), "参考jar不存在：{}", jar.display());
    let archive = BaseArchive::open(&jar).unwrap();
    let mut cache = HashMap::new();
    let BakedModelTextures {
        model,
        mut textures,
    } = bake_model(&archive, "block/furnace", &mut cache).expect("bake_model 失败");

    // 动画贴图换成首帧：渲染器按单帧 uv 取样，直接喂整条帧条带会渲染错
    mml_tex_draw::block::use_first_frame(&mut textures);

    BakedModelTextures { model, textures }
}

/// 诊断：每个 quad 的屏幕包围盒与深度，与 GPU 渲染用同一矩阵
fn print_quad_diag(model: &BakedModel) {
    use glam::{Mat3, Mat4, Vec3, Vec4};
    let size = 256.0f32;
    let model_m = Mat4::from_translation(Vec3::new(size / 2.0, size / 2.0, 0.0))
        * Mat4::from_scale(Vec3::new(size, -size, size))
        * mml_tex_draw::gpu::item_transform_matrix(&model.transform);
    let mml_tex_draw::gpu::LightDirs { light0, light1 } =
        mml_tex_draw::gpu::light_dirs(model.gui_light_3d);
    println!(
        "furnace：{} quads，L0=({:.2},{:.2},{:.2}) L1=({:.2},{:.2},{:.2})",
        model.quads.len(),
        light0[0], light0[1], light0[2],
        light1[0], light1[1], light1[2],
    );
    for q in &model.quads {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut z = 0.0;
        for p in &q.pos {
            let v = model_m * Vec4::new(p[0], p[1], p[2], 1.0);
            min_x = min_x.min(v.x);
            max_x = max_x.max(v.x);
            min_y = min_y.min(v.y);
            max_y = max_y.max(v.y);
            z += v.z;
        }
        let n = Mat3::from_mat4(model_m).inverse().transpose() * Vec3::from(q.normal);
        let n = n.normalize();
        let d = Vec3::from_slice(&light0[..3]).dot(n).max(0.0)
            + Vec3::from_slice(&light1[..3]).dot(n).max(0.0);
        println!(
            "  {:8} 屏幕x[{:.0},{:.0}] y[{:.0},{:.0}] depth={:.2} n=({:.2},{:.2},{:.2}) accum={:.3}",
            q.tex.split('/').next_back().unwrap_or(""),
            min_x, max_x, min_y, max_y,
            z / 4.0,
            n[0], n[1], n[2],
            (d * 0.6 + 0.4).min(1.0),
        );
    }
}

/// 单后端全套出图：方块直调 GPU，物品经 render_item，extrude 与 glint 各一
fn render_backend_suite(tag: &str, ctx: &GpuCtx) {
    ensure_item_dir();
    let archive = BaseArchive::open(ref_jar()).expect("参考jar应能打开");

    // 方块：furnace，多贴图元素模型
    let BakedModelTextures { model, textures } = bake_furnace();
    print_quad_diag(&model);
    let pixels = ctx.render(&model, &textures, None, 256);
    match pixels {
        Some(pixels) => write_png(&format!("furnace_{tag}.png"), 256, &pixels),
        None => println!("{tag} furnace 渲染失败"),
    }

    // 物品：apple 走 extrude 挤出，enchanted_book 走 glint 附魔光效
    let mut cache = HashMap::new();
    for id in ["apple", "enchanted_book"] {
        let out = mml_tex_draw::item::render_item(Some(ctx), &archive, &mut cache, id)
            .unwrap_or_else(|| panic!("{tag} {id} 渲染失败"));
        let dst = out_dir().join(format!("{id}_{tag}.png"));
        std::fs::copy(out_dir().join("items").join(&out.out_name), &dst).unwrap();
        println!("输出：{}", dst.display());
    }
}

/// DX12 后端，Windows 缺省
#[test]
#[ignore]
fn render_dx12() {
    match GpuCtx::try_new_backend("DX12", wgpu::Backends::DX12) {
        Some(ctx) => render_backend_suite("DX12", &ctx),
        None => println!("DX12 无可用适配器，跳过"),
    }
}

/// VULKAN 后端
#[test]
#[ignore]
fn render_vulkan() {
    match GpuCtx::try_new_backend("VULKAN", wgpu::Backends::VULKAN) {
        Some(ctx) => render_backend_suite("VULKAN", &ctx),
        None => println!("VULKAN 无可用适配器，跳过"),
    }
}

/// GL 后端
#[test]
#[ignore]
fn render_gl() {
    match GpuCtx::try_new_backend("GL", wgpu::Backends::GL) {
        Some(ctx) => render_backend_suite("GL", &ctx),
        None => println!("GL 无可用适配器，跳过"),
    }
}
