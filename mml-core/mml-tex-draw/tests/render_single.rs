//! 单方块与单物品渲染：GPU 管道冒烟与遮挡、透明诊断，直接用本地 jar，不走下载链
//!
//! 运行：cargo test -p mml-tex-draw --test render_single -- --ignored --nocapture
//! 环境变量：
//! - MML_TEST_JAR 指定客户端 jar，缺省用反编译参考 jar
//! - MML_TEST_BLOCK 指定方块模型，如 block/big_dripleaf，缺省 block/oak_stairs
//! - MML_TEST_ITEM 指定物品 ID，如 enchanted_book，缺省 apple
//! 出图到 tests/out/

use std::{collections::HashMap, path::PathBuf};

use mml_base::archives::BaseArchive;

/// 取参考客户端 jar 路径，环境变量 MML_TEST_JAR 优先
fn ref_jar() -> PathBuf {
    std::env::var("MML_TEST_JAR").map(PathBuf::from).unwrap_or_else(|_| {
        // 优先用渲染测试已下载的客户端 jar tests/out/run，其次 H 盘 Temp 的反编译参考 jar
        let run_jar = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "tests/out/run/minecraft/libraries/net/minecraft/client/26.3/client-26.3.jar",
        );
        if run_jar.exists() {
            run_jar
        } else {
            PathBuf::from("H:/Temp/mml-262-ref/client.jar")
        }
    })
}

/// 取 tests/out 下的输出路径，目录不存在时创建
fn out_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("out");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// 把 RGBA 像素写成 size×size 的 PNG 到指定路径
fn write_png(path: &PathBuf, size: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
    println!("输出：{}", path.display());
}

/// 渲染单个方块模型并出图，用于诊断遮挡与透明问题
#[test]
#[ignore]
fn render_one_block() {
    let jar = ref_jar();
    assert!(jar.exists(), "参考jar不存在：{}", jar.display());
    let archive = BaseArchive::open(&jar).unwrap();

    let rel = std::env::var("MML_TEST_BLOCK").unwrap_or_else(|_| "block/oak_stairs".into());
    let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();

    let mut cache = HashMap::new();
    let Some(mml_tex_draw::model::BakedModelTextures {
        model,
        mut textures,
    }) = mml_tex_draw::model::bake_model(&archive, &rel, &mut cache)
    else {
        panic!("bake_model 失败");
    };
    // 动画贴图换成首帧：渲染器按单帧 uv 取样，直接喂整条帧条带会渲染错
    // 图标管线 render_baked 内部同样会做这一步
    mml_tex_draw::block::use_first_frame(&mut textures);
    println!(
        "bake：{} quads，{} 贴图，gui_light_3d={}",
        model.quads.len(),
        textures.len(),
        model.gui_light_3d
    );
    for q in &model.quads {
        println!(
            "  quad tex={} n=({:.0},{:.0},{:.0}) translucent={} p0={:.3?} p2={:.3?}",
            q.tex.split('/').next_back().unwrap_or(""),
            q.normal[0],
            q.normal[1],
            q.normal[2],
            q.translucent,
            q.pos[0],
            q.pos[2]
        );
    }

    // CPU 侧复算 shader 光照，诊断明暗是否符合预期
    {
        use glam::{Mat3, Mat4, Vec3};
        let slot = 256.0;
        println!(
            "  transform rot={:?} t={:?} s={:?}",
            model.transform.rotation, model.transform.translation, model.transform.scale
        );
        let model_m = Mat4::from_translation(Vec3::new(slot / 2.0, slot / 2.0, 0.0))
            * Mat4::from_scale(Vec3::new(slot, -slot, slot))
            * mml_tex_draw::gpu::item_transform_matrix(&model.transform);
        let n3 = Mat3::from_mat4(model_m).inverse().transpose();
        println!("  n3列={:?}", n3.to_cols_array());
        let mml_tex_draw::gpu::LightDirs { light0, light1 } =
            mml_tex_draw::gpu::light_dirs(model.gui_light_3d);
        println!(
            "  L0=({:.3},{:.3},{:.3}) L1=({:.3},{:.3},{:.3})",
            light0[0], light0[1], light0[2], light1[0], light1[1], light1[2]
        );
        for q in &model.quads {
            let n = n3 * Vec3::from(q.normal);
            let n = n.normalize();
            let d = Vec3::from_slice(&light0[..3]).dot(n).max(0.0)
                + Vec3::from_slice(&light1[..3]).dot(n).max(0.0);
            let accum = (d * 0.6 + 0.4).min(1.0);
            println!(
                "  光照 tex={} n=({:.2},{:.2},{:.2}) accum={:.3}",
                q.tex.split('/').next_back().unwrap_or(""),
                n[0],
                n[1],
                n[2],
                accum
            );
        }
    }

    let ctx = mml_tex_draw::gpu::GpuCtx::try_new();
    if let Some(ctx) = ctx.as_ref() {
        let Some(pixels) = ctx.render(&model, &textures, None, 256) else {
            panic!("GPU渲染失败");
        };
        write_png(&out_path(&format!("{name}_gpu.png")), 256, &pixels);
    } else {
        println!("无可用GPU后端，跳过GPU出图");
    }
}

/// 渲染单个物品图标：items/*.json → extrude 或元素烘焙 → 出图，仅 GPU
///
/// 样例：diamond_sword 挤出、enchanted_book glint、compass condition→range_dispatch、
/// spyglass select、leather_chestplate tint、trident select gui 分支
#[test]
#[ignore]
fn render_one_item() {
    let jar = ref_jar();
    assert!(jar.exists(), "参考jar不存在：{}", jar.display());
    let archive = BaseArchive::open(&jar).unwrap();

    let id = std::env::var("MML_TEST_ITEM").unwrap_or_else(|_| "apple".into());
    let name = id.rsplit('/').next().unwrap_or(&id).to_string();

    // 物品输出目录初始化到独立子目录，渲染结果再拷到 tests/out 查看
    let dir = out_path("item-render");
    mml_tex_draw::init(&dir).unwrap();

    let mut cache = HashMap::new();
    let gpu = mml_tex_draw::gpu::GpuCtx::try_new();
    let out = mml_tex_draw::item::render_item(gpu.as_ref(), &archive, &mut cache, &id)
        .unwrap_or_else(|| panic!("render_item GPU失败：{id}"));
    std::fs::copy(dir.join("items").join(&out.out_name), out_path(&format!("{name}_gpu.png"))).unwrap();
}
