//! 资源包读取测试。
//!
//! 资源包样本不提交进 git：
//! - 用 `zip::ZipWriter` 程序化生成一个最小有效样本做精确断言，离线可跑；
//! - 通过 Modrinth API 解析多个常用资源包的最新版本并下载，网络不可用时跳过。

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

use mml_game::game_resourcepacks::{
    ResourcepackObj, process_resourcepack as process_resourcepack_raw,
};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// 测试里固定的界面语言，与前端 `locale` 同值
const LANG: &str = "zh_cn";

/// 网络样本用例的启动链，每进程一次
///
/// **必须先把日志系统起来**：`common::init_net` 里的 `mml_net::init` 会打日志，
/// 而 `mml_log` 没 `start()` 过时 `info()` 直接 panic，`SEM` / `STREAM` 都是
/// `OnceLock`，见 AGENTS.md §6。与 `real_pack_download.rs` 的 `ensure_init` 同一口径。
///
/// 只给走网络的那条用例用：纯本地用例自己造 zip、不碰日志，没有这个前置要求。
fn ensure_init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml-resourcepack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
    });
}

/// 按界面语言解析资源包，省得每个用例都写第二个参数；错误转成可读字符串
fn process_resourcepack<P: AsRef<Path>>(path: P) -> Result<ResourcepackObj, String> {
    process_resourcepack_raw(path, LANG).map_err(|err| err.to_string())
}

/// 1×1 透明 PNG，最小有效文件
const PACK_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// 生成一个最小资源包 zip：pack.mcmeta + pack.png。
fn make_mini_resourcepack() -> PathBuf {
    make_resourcepack_with(
        br#"{"pack":{"pack_format":15,"min_format":5,"max_format":30,"description":"mml test pack"}}"#,
    )
}

/// 生成一个资源包 zip，pack.mcmeta 内容由调用方给，用于覆盖各种写法
fn make_resourcepack_with(meta: &[u8]) -> PathBuf {
    let path = mml_testutil::temp_dir().join(format!(
        "mml-resourcepack-mini-{}.zip",
        uuid::Uuid::new_v4()
    ));
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    writer.start_file("pack.mcmeta", options).unwrap();
    writer.write_all(meta).unwrap();
    writer.start_file("pack.png", options).unwrap();
    writer.write_all(PACK_PNG).unwrap();
    writer.finish().unwrap();

    path
}

/// 生成一个带**自带语言表**的资源包 zip，表放在 `assets/<命名空间>/lang/<语言代码>.json`
fn make_resourcepack_with_lang(meta: &[u8], lang_files: &[(&str, &str)]) -> PathBuf {
    let path = mml_testutil::temp_dir().join(format!(
        "mml-resourcepack-lang-{}.zip",
        uuid::Uuid::new_v4()
    ));
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    writer.start_file("pack.mcmeta", options).unwrap();
    writer.write_all(meta).unwrap();

    for (name, text) in lang_files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }

    writer.finish().unwrap();

    path
}

/// 程序化生成的最小样本：精确断言描述、版本区间与图标。
#[test]
fn read_generated_resourcepack() {
    let path = make_mini_resourcepack();
    let obj: ResourcepackObj = process_resourcepack(&path).expect("应能解析生成的资源包");

    assert!(!obj.fail, "生成的样本应解析成功");
    assert_eq!(obj.description, "mml test pack");
    assert_eq!(obj.pack_format, 15);
    assert_eq!(obj.min_format, 5);
    assert_eq!(obj.max_format, 30);
    assert_eq!(obj.icon.as_deref(), Some(PACK_PNG.as_ref()));
}

/// `supported_formats` 写成**数组**：`[8, 9999]`
///
/// 真实踩过的坑：`SupportedFormats` 起初只认"对象"和"单个数字"，数组形式两种都不匹配，
/// untagged 全部失败、整个 `PackInfo` 反序列化报错，界面显示"读取失败"；数组语义是 `[min, max]`。
#[test]
fn read_supported_formats_as_array() {
    let meta = br#"{"pack":{"pack_format":8,"supported_formats":[8,9999],"description":"test"}}"#;
    let path = make_resourcepack_with(meta);
    let obj = process_resourcepack(&path).expect("应能解析数组形式的 supported_formats");

    assert!(!obj.fail, "数组形式的 supported_formats 不该导致读取失败");
    assert_eq!(obj.pack_format, 8);
    assert_eq!(obj.min_format, 8);
    assert_eq!(obj.max_format, 9999);
}

/// `supported_formats` 的三种写法都要认
#[test]
fn read_supported_formats_variants() {
    // 对象
    let obj = process_resourcepack(&make_resourcepack_with(
        br#"{"pack":{"pack_format":15,"supported_formats":{"min_inclusive":5,"max_inclusive":75}}}"#,
    ))
    .unwrap();
    assert!(!obj.fail);
    assert_eq!((obj.min_format, obj.max_format), (5, 75));

    // 单个数字，含义是"从这一版起"
    let obj = process_resourcepack(&make_resourcepack_with(
        br#"{"pack":{"pack_format":15,"supported_formats":20}}"#,
    ))
    .unwrap();
    assert!(!obj.fail);
    assert_eq!((obj.min_format, obj.max_format), (20, 0));

    // 数组只给一个元素，等价于"从这一版起"
    let obj = process_resourcepack(&make_resourcepack_with(
        br#"{"pack":{"pack_format":15,"supported_formats":[30]}}"#,
    ))
    .unwrap();
    assert!(!obj.fail);
    assert_eq!((obj.min_format, obj.max_format), (30, 0));
}

/// 显式写了 `min_format` / `max_format` 时，不让 `supported_formats` 盖掉
#[test]
fn explicit_min_max_wins() {
    let obj = process_resourcepack(&make_resourcepack_with(
        br#"{"pack":{"pack_format":15,"min_format":11,"max_format":22,"supported_formats":[5,75]}}"#,
    ))
    .unwrap();
    assert_eq!((obj.min_format, obj.max_format), (11, 22));
}

/// 数组形式 + 带 `§` 格式码且含换行的描述，样本取自 Slightly Improved Font 的 pack.mcmeta
///
/// 两个坑叠在一起：数组形式让**整包**解析失败，见上一个用例；描述里的 `§e` / `§b` 与 `\n`
/// 则要求文本原样保留 —— 上色是**前端**渲染时的事，见 mml-vue 的 lib/formatting.ts。
#[test]
fn read_real_font_pack() {
    let meta = r#"{"pack":{"pack_format":8,"supported_formats":[8,9999],"description":"§eSlightly Improved Font 1.1.5\n§bMade by latvian.dev"}}"#;
    let obj = process_resourcepack(&make_resourcepack_with(meta.as_bytes())).unwrap();

    assert!(!obj.fail, "这个包不该被判为读取失败");
    assert_eq!(obj.pack_format, 8);
    assert_eq!((obj.min_format, obj.max_format), (8, 9999));
    // 原始文本原样保留，`§` 码与换行都不动
    assert_eq!(
        obj.description,
        "§eSlightly Improved Font 1.1.5\n§bMade by latvian.dev"
    );
}

/// 组件数组形式的 `description` + `supported_formats` 区间
///
/// 真实踩过的坑，样本是 Sophisticated Backpacks 的数据包：`description` 写成
/// `[{"text": "…"}]` 时，旧的 `description: String` 会让**整包**反序列化失败。
#[test]
fn read_component_description_and_supported_formats() {
    let meta = br#"{"pack":{"pack_format":57,"supported_formats":{"min_inclusive":5,"max_inclusive":75},"description":[{"text":"123456"}]}}"#;
    let path = make_resourcepack_with(meta);
    let obj = process_resourcepack(&path).expect("应能解析组件描述的 pack.mcmeta");

    assert!(!obj.fail, "组件数组形式的 description 不该导致读取失败");
    assert_eq!(obj.description, "123456");
    assert_eq!(obj.pack_format, 57);
    assert_eq!(obj.min_format, 5, "区间应取自 supported_formats");
    assert_eq!(obj.max_format, 75);
}

/// `description` 的另外两种写法也应认：组件对象、多个组件拼接
#[test]
fn read_description_variants() {
    // 组件对象
    let obj = process_resourcepack(&make_resourcepack_with(
        r#"{"pack":{"pack_format":15,"description":{"text":"对象写法"}}}"#.as_bytes(),
    ))
    .unwrap();
    assert!(!obj.fail);
    assert_eq!(obj.description, "对象写法");

    // 多个组件：按顺序拼成一段话
    let obj = process_resourcepack(&make_resourcepack_with(
        r#"{"pack":{"pack_format":15,"description":[{"text":"前半"},{"text":"后半"}]}}"#.as_bytes(),
    ))
    .unwrap();
    assert_eq!(obj.description, "前半后半");

    // 数组里混裸字符串，也是合法写法
    let obj = process_resourcepack(&make_resourcepack_with(
        br#"{"pack":{"pack_format":15,"description":[{"text":"a"},"b"]}}"#,
    ))
    .unwrap();
    assert_eq!(obj.description, "ab");
}

/// 简介里的 `translate` 键用**资源包自带**的语言表翻
///
/// 样本是 FO 整合包里的 "Chat Reporting Helper"，取自真实 pack.mcmeta：它写的是
/// `{"translate":"fo.resourcePack.chatreportinghelper","fallback":"§7Explains…§r"}`，
/// 同时包自带 `assets/fo/lang/zh_cn.json` 写着中文译文。命中语言表时应当用**译文**，
/// 而不是 fallback —— 与游戏里的表现一致。
///
/// 注意这里走的是 `process_resourcepack` 的完整路径：语言表要从**同一个 zip** 里读，
/// 所以这条用例同时钉住了"读语言表"与"展平组件"两步的衔接。
#[test]
fn read_translate_from_pack_lang() {
    // 简介里的 `§` 是**字面字符**，真实文件就是 UTF-8 的 §，所以这里用普通原始字符串
    // 而不是 `br#""#`，字节串只能是 ASCII
    let meta = r#"{"pack":{"pack_format":18,"min_format":18,"max_format":97,"description":{"translate":"fo.resourcePack.chatreportinghelper","fallback":"§7Explains chat reporting with simple phrases and icons§r"}}}"#;
    let path = make_resourcepack_with_lang(
        meta.as_bytes(),
        &[(
            "assets/fo/lang/zh_cn.json",
            r#"{"fo.resourcePack.chatreportinghelper":"§7用简单的短语与图标说明聊天举报机制§r"}"#,
        )],
    );

    let obj = process_resourcepack(&path).expect("应能解析带语言表的资源包");
    assert!(!obj.fail);
    assert_eq!(obj.description, "§7用简单的短语与图标说明聊天举报机制§r");
}

/// 语言表里没有界面语言时用 `en_us` 兜底，包的作者一般都会写英文
#[test]
fn read_translate_falls_back_to_en_us_lang() {
    let meta = br#"{"pack":{"pack_format":15,"description":{"translate":"sodium.resource_pack.unofficial","fallback":"Unofficial translations for Sodium"}}}"#;
    let path = make_resourcepack_with_lang(
        meta,
        &[(
            "assets/sodium/lang/en_us.json",
            r#"{"sodium.resource_pack.unofficial":"EN text"}"#,
        )],
    );

    // 界面语言是 zh_cn，包里只有 en_us → 用 en_us 那份
    let obj = process_resourcepack(&path).expect("应能解析");
    assert_eq!(obj.description, "EN text");
}

/// 语言表里没这个 key、或整包没有语言表：退回 `fallback`，再没有就原样显示 key
#[test]
fn read_translate_without_lang_falls_back() {
    // 整包没有语言表 → 用 fallback
    let obj = process_resourcepack(&make_resourcepack_with(
        r#"{"pack":{"pack_format":15,"description":{"translate":"some.key","fallback":"兜底文字"}}}"#
            .as_bytes(),
    ))
    .unwrap();
    assert_eq!(obj.description, "兜底文字");

    // 有语言表、但表里没有这个 key → 还是 fallback
    let other_key = make_resourcepack_with_lang(
        r#"{"pack":{"pack_format":15,"description":{"translate":"some.key","fallback":"兜底文字"}}}"#
            .as_bytes(),
        &[("assets/fo/lang/zh_cn.json", r#"{"other.key":"别的"}"#)],
    );
    assert_eq!(
        process_resourcepack(&other_key).unwrap().description,
        "兜底文字"
    );

    // 连 fallback 都没有 → 把 key 原样给出来，比留空强
    let bare = make_resourcepack_with_lang(
        r#"{"pack":{"pack_format":15,"description":{"translate":"some.key"}}}"#.as_bytes(),
        &[("assets/fo/lang/zh_cn.json", r#"{"other.key":"别的"}"#)],
    );
    assert_eq!(process_resourcepack(&bare).unwrap().description, "some.key");
}

/// 从 Modrinth 下载多个常用资源包并解析。
/// 网络不可用时整体跳过；单个样本下载失败跳过，其余继续。
#[test]
fn read_real_resourcepacks() {
    ensure_init();

    let mut downloaded = 0;
    let mut parsed = 0;

    for slug in common::RESOURCEPACK_SLUGS {
        let Some(path) = common::download_latest(slug) else {
            eprintln!("[跳过] 资源包 {slug} 下载失败（网络不可用？）");
            continue;
        };
        downloaded += 1;

        match process_resourcepack(&path) {
            Ok(obj) => {
                // 下载成功但 pack.mcmeta 解析失败时 fail 置 true，属真实缺陷
                assert!(!obj.fail, "资源包 {slug} 的 pack.mcmeta 解析失败");
                // 新格式资源包 1.20.5+ 不再提供 pack_format，只用 min/max_format
                assert!(
                    obj.pack_format > 0 || obj.min_format > 0 || obj.max_format > 0,
                    "资源包 {slug} 未提供任何格式版本字段"
                );
                parsed += 1;
                println!(
                    "{slug}: pack_format = {}, min_format = {}, max_format = {}, description = {}",
                    obj.pack_format, obj.min_format, obj.max_format, obj.description
                );
            }
            Err(err) => {
                eprintln!("[警告] 资源包 {slug} 读取失败：{err}");
            }
        }
    }

    if downloaded == 0 {
        eprintln!("全部资源包样本均无法下载（需要网络），测试跳过");
        return;
    }
    assert!(parsed > 0, "下载成功但未能解析任何资源包");
}
