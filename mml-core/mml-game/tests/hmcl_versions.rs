//! 外部启动器版本 JSON 的读取测试，样本在 `tests/versions/`，是 HMCL 导出的真实文件。
//!
//! 目的：验证"直接把别的启动器的 `.minecraft/versions` 当实例导入"时，版本号与加载器
//! 是否识别正确。样本覆盖四种形态：
//!
//! | 文件 | 形态 |
//! | --- | --- |
//! | `26.2.json` | 原版：无 `patches`、无 `inheritsFrom`，版本号取 `id` |
//! | `26.2-forbric.json` | 继承式：无 `patches`、有 `inheritsFrom`，版本号取 `inheritsFrom` |
//! | `26.2-Fabric.json` / `26.2-Forge.json` / `26.2-NeoForge.json` | patch 式：版本号与加载器都在 `patches` 里 |
//!
//! 前两种的版本号要靠 `have_version()` 在**版本清单**里查得到，清单来自 Mojang 版本列表缓存，
//! 清单没加载时会被判成空，这正是本测试要盯住的行为。
//!
//! 版本清单样本用环境变量 `MML_VERSION_LIST` 覆盖；不设则用启动器运行目录里的那份缓存，
//! 目录由 `MML_RUN_DIR` 指定，默认 `target/debug/mml`；两个变量都没设时跳过相关断言。
//!
//! 运行：`cargo test -p mml-game --test hmcl_versions -- --nocapture`

use std::path::{Path, PathBuf};
use std::sync::Once;

use mml_game::other_launcher::official_obj::OfficialObj;

/// 测试运行目录，临时目录加进程号，避免多进程冲突
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join(format!("mml-hmcl-versions-{}", std::process::id()))
}

/// 版本 JSON 样本目录
fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/versions")
}

/// 初始化链，与 `mml_core::init` 相同，每进程一次
///
/// 之后把一份**版本清单缓存**放进运行目录：`have_version()` 依赖它才能认出 `26.2`。
/// 清单来源见 [`find_version_list`]；找不到就不放，此时无 patches 的版本号只能为空，
/// 相关断言会给出"清单不可用"的明确失败原因。
fn ensure_init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = run_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_game::init(mml_base::get_base_dir()).unwrap();

        if let Some(src) = find_version_list() {
            let dst_dir = mml_base::get_base_dir().join("minecraft/versions");
            // 目标目录可能还没建，版本路径的目录是首次用到才创建，先补上
            let _ = std::fs::create_dir_all(&dst_dir);
            let dst = dst_dir.join(mml_names::names::VERSION_FILE);
            match std::fs::copy(&src, &dst) {
                Ok(_) => println!("版本清单缓存：{}", src.display()),
                Err(e) => println!("复制版本清单失败（{e}），版本号可能判成空"),
            }
        } else {
            println!("没有可用的版本清单缓存，have_version 会判 false");
        }
    });
}

/// 找一份可用的版本清单缓存，即 Mojang 版本列表
fn find_version_list() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("MML_VERSION_LIST") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let dir = std::env::var("MML_RUN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/debug/mml"));
    // 相对路径按仓库根目录解析，cargo test 的工作目录是 crate 目录
    let dir = if dir.is_absolute() {
        dir
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(dir)
    };

    let cached = dir.join("minecraft/versions/version.json");
    cached.is_file().then_some(cached)
}

/// 一个版本样本的解析结果
struct Dump {
    /// 文件名，不含 .json 后缀
    name: String,
    /// json 里的 id
    id: String,
    /// json 里的 inheritsFrom，没有时为空串
    inherits_from: String,
    /// patches 是否为空
    no_patches: bool,
    /// `to_instance()` 得到的版本号
    version: String,
    /// `to_instance()` 得到的加载器，Debug 文本
    loader: String,
    /// `to_instance()` 得到的加载器版本，原版为 `None`
    loader_version: Option<String>,
}

/// 读一个版本样本并打印解析结果
///
/// - `file`: 版本 JSON 样本路径
///
/// # 返回值
///
/// 解析失败时返回 `None`
fn dump_one(file: &Path) -> Option<Dump> {
    let name = file.file_stem()?.to_string_lossy().to_string();

    let obj = match OfficialObj::read_from_file(file) {
        Ok(obj) => obj,
        Err(e) => {
            println!("{name}: 解析失败 {e}");
            return None;
        }
    };

    let patches: Vec<String> = obj
        .patches
        .iter()
        .map(|p| format!("{}={}", p.id, p.version))
        .collect();
    println!("==== {name} ====");
    println!(
        "  id={} inherits_from={} patches=[{}]",
        obj.id,
        obj.inherits_from,
        patches.join(", ")
    );

    let id = obj.id.clone();
    let inherits_from = obj.inherits_from.clone();
    let no_patches = obj.patches.is_empty();

    let instance = obj.to_instance();
    println!(
        "  => version='{}' loader={:?} loader_version={:?}",
        instance.version, instance.loader, instance.loader_version
    );

    Some(Dump {
        name,
        id,
        inherits_from,
        no_patches,
        version: instance.version,
        loader: format!("{:?}", instance.loader),
        loader_version: instance.loader_version,
    })
}

/// 遍历样本目录，逐个校验版本号与加载器的识别结果
#[test]
fn read_hmcl_versions() {
    ensure_init();

    let root = samples_dir();
    if !root.is_dir() {
        println!("跳过：样本目录不存在 {}", root.display());
        return;
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("读取样本目录失败")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();

    let dumps: Vec<Dump> = files.iter().filter_map(|f| dump_one(f)).collect();
    println!("共 {} 个版本样本", dumps.len());
    assert!(!dumps.is_empty(), "样本目录里没有 json，检查一下是否漏提交");

    for d in &dumps {
        if d.no_patches {
            // 无 patches：版本号来自 json（inheritsFrom 优先），且必须能被 have_version 认出来
            let want = if d.inherits_from.is_empty() {
                &d.id
            } else {
                &d.inherits_from
            };
            assert_eq!(
                &d.version, want,
                "{}: 无 patches 的版本，版本号应识别为 {want}（空 = 版本清单没加载出来）",
                d.name
            );
        } else {
            // patches 式：版本号来自 patches 里的 game 项，加载器也在 patches 里
            assert!(
                !d.version.is_empty(),
                "{}: patches 式版本没解析出版本号",
                d.name
            );
            assert_ne!(
                d.loader, "Normal",
                "{}: patches 里带了加载器，却没识别出来",
                d.name
            );
            assert!(
                d.loader_version.is_some(),
                "{}: 识别出加载器但没拿到加载器版本",
                d.name
            );
        }
    }
}
