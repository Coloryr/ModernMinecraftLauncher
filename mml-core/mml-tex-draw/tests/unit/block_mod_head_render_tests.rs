use super::*;

/// 头颅渲染冒烟：各头颅出图到 tests/out/head_*.png，需 GPU 与本地客户端 jar
///
/// 运行：cargo test -p mml-tex-draw --lib head_render -- --ignored --nocapture
/// 环境变量 MML_TEST_JAR 指定客户端 jar，缺省用 tests/out/tools/client-26.3.jar
#[test]
#[ignore]
fn head_render_smoke() {
    let jar = std::env::var("MML_TEST_JAR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/out/tools/client-26.3.jar")
        });
    assert!(jar.exists(), "客户端jar不存在：{}", jar.display());

    let archive = BaseArchive::open(&jar).unwrap();
    let gpu = GpuCtx::try_new().expect("无可用GPU后端");

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/out");
    std::fs::create_dir_all(&dir).unwrap();

    // 各头颅：单层 64×32 贴图与带帽子层 64×64 贴图各取一个
    for (id, tex, hat) in [
        ("skeleton_skull", "entity/skeleton/skeleton", false),
        ("creeper_head", "entity/creeper/creeper", false),
        ("player_head", "entity/player/wide/steve", true),
        ("zombie_head", "entity/zombie/zombie", true),
    ] {
        let mut cache = HashMap::new();
        let BakedModelTextures { model, textures } = bake_head(&archive, tex, hat, &mut cache)
            .unwrap_or_else(|| panic!("{id} 烘焙失败"));
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

        let path = dir.join(format!("head_{id}.png"));
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(file, 256, 256);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
        println!("输出：{}", path.display());
    }
}

/// 用本地皮肤文件渲染头颅图标；slim 与 wide 只差手臂，头部区域一致，均按带帽子层烘焙
///
/// 运行：cargo test -p mml-tex-draw --lib head_render_skin_file -- --ignored --nocapture
#[test]
#[ignore]
fn head_render_skin_file() {
    let skin = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../mml-skin-draw/tests/skin_slim.png");
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

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/out");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("head_skin_slim.png");
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = png::Encoder::new(file, 256, 256);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
    println!("输出：{}", path.display());
}

