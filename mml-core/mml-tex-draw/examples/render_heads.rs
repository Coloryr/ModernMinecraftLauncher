//! 头颅渲染工具：从客户端 jar 给原版头颅逐个出图，再用本地皮肤文件出一张
//!
//! 运行：`cargo run -p mml-tex-draw --example render_heads`
//! 环境变量 MML_TEST_JAR 指定客户端 jar，缺省 `tests/out/tools/client-26.3.jar`
//! 输出：tests/out/head_*.png

use std::{collections::HashMap, path::PathBuf};

use mml_base::archives::BaseArchive;
use mml_tex_draw::block::{bake_head, bake_head_model, decode_png};
use mml_tex_draw::gpu::GpuCtx;
use mml_tex_draw::model::BakedModelTextures;

/// 取参考客户端 jar 路径，环境变量 MML_TEST_JAR 优先
fn ref_jar() -> PathBuf {
    std::env::var("MML_TEST_JAR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/out/tools/client-26.3.jar")
        })
}

/// tests/out 下的输出路径，目录不存在时创建
fn out_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("out");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// 把 RGBA 像素写成 256×256 的 PNG
fn write_png(path: &PathBuf, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, 256, 256);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
    println!("输出：{}", path.display());
}

/// 原版头颅：单层 64×32 贴图与带帽子层 64×64 贴图各取两个
fn render_vanilla_heads() {
    let jar = ref_jar();
    assert!(jar.exists(), "客户端jar不存在：{}", jar.display());
    let archive = BaseArchive::open(&jar).unwrap();
    let gpu = GpuCtx::try_new().expect("无可用GPU后端");

    for (id, tex, hat) in [
        ("skeleton_skull", "entity/skeleton/skeleton", false),
        ("creeper_head", "entity/creeper/creeper", false),
        ("player_head", "entity/player/wide/steve", true),
        ("zombie_head", "entity/zombie/zombie", true),
    ] {
        let mut cache = HashMap::new();
        let BakedModelTextures { model, textures } =
            bake_head(&archive, tex, hat, &mut cache).unwrap_or_else(|| panic!("{id} 烘焙失败"));
        assert_eq!(model.quads.len(), if hat { 12 } else { 6 }, "{id} 面数不符");

        // 带帽子层的贴图必须是 humanoidHeadLayer 的 64×64，mobHeadLayer 是 64×32
        let tex_size = textures
            .values()
            .map(|p| (p.width(), p.height()))
            .next()
            .unwrap();
        assert_eq!(
            tex_size,
            if hat { (64, 64) } else { (64, 32) },
            "{id} 贴图尺寸不符"
        );

        let rgba = gpu
            .render(&model, &textures, None, 256)
            .expect("GPU渲染失败");
        write_png(&out_path(&format!("head_{id}.png")), &rgba);
    }
}

/// 用本地皮肤文件渲染头颅图标；slim 与 wide 只差手臂，头部区域一致，均按带帽子层烘焙
fn render_skin_head() {
    let skin =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mml-skin-draw/tests/skin_slim.png");
    assert!(skin.exists(), "皮肤文件不存在：{}", skin.display());
    let bytes = std::fs::read(&skin).unwrap();
    let tex = decode_png(&bytes).expect("皮肤解码失败");
    let gpu = GpuCtx::try_new().expect("无可用GPU后端");

    let model = bake_head_model(tex.clone(), true, "skin");
    assert_eq!(model.quads.len(), 12, "面数不符");
    let textures = HashMap::from([("skin".to_string(), tex)]);
    let rgba = gpu
        .render(&model, &textures, None, 256)
        .expect("GPU渲染失败");
    write_png(&out_path("head_skin_slim.png"), &rgba);
}

fn main() {
    render_vanilla_heads();
    render_skin_head();
}
