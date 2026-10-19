//! 迁移引擎的用例

use super::window::*;

use std::path::PathBuf;


use mml_base::get_base_dir;



use super::detect::{asked};
use super::migrate::migrate;

// 迁移引擎的用例
//
// 数据两种：
// - **真实样本** `tests/colormc/`，从真 ColorMC 工作目录裁出来的一份，见那边的 README ——
//   验"真实的 ColorMC 目录能被识别、里面的实例 M²L 读得出来、结论表覆盖齐全"；
// - **临时小目录**：边界情况现场造，覆盖同名文件、拒绝套娃路径、只有 auth.json 的目录。
//
// 全部在 `target/temp` 下操作，见 AGENTS.md §6，而且**只读**真实样本 —— 移动用例先把样本
// 复制一份再搬，反复跑也不会把样本搬没。

// 单测要同时用到三个子模块的实现细节，原先是同一个文件里的 `use super::*`
use std::fs;
use std::path::Path;

use mml_base::serialize_tools;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_names::names;

use super::check::*;
use super::detect::*;
use super::migrate::*;
use crate::test_support::{cleanup, ensure_boot, temp_dir};

/// 真实样本目录 `mml-gui/src-tauri/tests/colormc`
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("colormc")
}

/// 兼容性结论的查表助手
fn level_of(checked: &Checked, name: &str) -> String {
    checked
        .compat
        .iter()
        .find(|item| item.name == name)
        .map(|item| item.level.clone())
        .unwrap_or_default()
}

/// 真实样本：能识别成 ColorMC 工作目录，三个实例 M²L 都读得出来
#[test]
fn real_fixture_is_recognized_and_readable() {
    ensure_boot();
    let src = fixture();
    assert!(src.is_dir(), "缺少真实样本目录：{}", src.display());

    // 识别靠 `minecraft/instances` 命中，而不是"目录存在"或里面有几个 json
    assert!(looks_like_colormc(&src));

    // 三个真实实例都在
    let instances = list_instances(&src);
    assert_eq!(instances.len(), 3, "实际：{instances:?}");
    assert!(
        instances.contains(&String::from("1.21.11-NeoForge-21.11.38-beta")),
        "实际：{instances:?}"
    );

    // 以样本目录自身为"目标"只读地跑兼容性检测：真实实例不该读不出来
    let entries = top_entries(&src);
    let checked = check(None, &entries, &instances, &src);
    assert!(
        checked.instances_bad.is_empty(),
        "真实实例读不出来：{:?}",
        checked.instances_bad
    );
    assert_eq!(checked.instances_ok.len(), 3);
    // 那个 NeoForge 实例带着 ColorMC 的 guisetting.json：M²L 不读它，报告里要提一句
    assert_eq!(
        checked.instances_legacy_gui,
        vec![String::from("1.21.11-NeoForge-21.11.38-beta")]
    );

    // 顶层结论：minecraft 可用、config.json 有偏差、用不到的按 extra
    assert_eq!(level_of(&checked, names::MINECRAFT_DIR), "ok");
    assert_eq!(level_of(&checked, names::CONFIG_FILE), "partial");
    assert_eq!(level_of(&checked, names::COLLECT_FILE), "ok");
    assert_eq!(level_of(&checked, "lock"), "extra");
}

/// 兼容性结论要覆盖真实工作目录的**全部**顶层条目
///
/// 样本里只搬了"有内容且小"的那些；`download` / `block` / `java` 之类太大没入仓，
/// 这里按真实目录的名字建同名空目录来验结论表。
#[test]
fn compat_covers_all_real_top_level_entries() {
    ensure_boot();
    let src = temp_dir("compat");
    for name in [
        "minecraft",
        "download",
        "block",
        "image",
        "java",
        "tools",
        "frpc",
        "inputs",
    ] {
        fs::create_dir_all(src.join(name)).unwrap();
    }
    for name in [
        "cloud.json",
        "collect.json",
        "config.json",
        "count.dat",
        "frp.json",
        "gui.json",
        "lock",
        "logs.log",
        "window.json",
    ] {
        fs::write(src.join(name), "").unwrap();
    }

    let entries = top_entries(&src);
    let checked = check(None, &entries, &[], &src);

    // 能直接用
    assert_eq!(level_of(&checked, "minecraft"), "ok");
    assert_eq!(level_of(&checked, "collect.json"), "ok");
    // 能用但有偏差 / 会丢东西
    assert_eq!(level_of(&checked, "config.json"), "partial");
    assert_eq!(level_of(&checked, "count.dat"), "partial");
    assert_eq!(level_of(&checked, "logs.log"), "partial");
    assert_eq!(level_of(&checked, "java"), "partial");
    // M²L 读不懂，留着不碍事 —— 包括表里没列到的
    for name in [
        "download",
        "block",
        "image",
        "tools",
        "frpc",
        "inputs",
        "gui.json",
        "window.json",
        "lock",
        "cloud.json",
        "frp.json",
    ] {
        assert_eq!(level_of(&checked, name), "extra", "{name} 该归 extra");
    }

    cleanup(&[&src]);
}

/// 复制真实样本：源目录原样保留，目标拿到实例且仍然可用
#[test]
fn copy_real_fixture_keeps_source_and_instances_usable() {
    ensure_boot();
    let src = fixture();
    let dst = temp_dir("copy");

    let report = migrate(None, &src, &dst, false).unwrap();

    assert!(!report.moved);
    assert!(src.join(names::CONFIG_FILE).is_file(), "复制不该动源目录");
    assert!(dst.join(names::CONFIG_FILE).is_file());
    assert!(
        report.failed.is_empty(),
        "不该有失败项：{:?}",
        report.failed
    );
    assert_eq!(
        report.instances_ok.len(),
        3,
        "搬过去的实例应全部可用，认不出的：{:?}",
        report.instances_bad
    );

    // 实例里的文件真的搬过去了，不是只建了目录，而且还能被 M²L 解析
    let game = dst
        .join(names::MINECRAFT_DIR)
        .join(names::INSTANCE_DIR)
        .join("1.21.11-NeoForge-21.11.38-beta")
        .join(names::GAME_FILE);
    assert!(game.is_file());
    assert!(serialize_tools::json_from_file::<InstanceSettingObj>(&game).is_ok());

    cleanup(&[&dst]);
}

/// 移动：条目被重命名过去，搬空后源目录被删
///
/// **先复制再移动** —— 真实样本不能被搬走，用例要能反复跑。
#[test]
fn move_renames_entries_and_removes_source() {
    ensure_boot();
    let staging = temp_dir("move-src");
    let mut done: u32 = 0;
    let mut failed: Vec<String> = Vec::new();
    copy_entry(
        None,
        "copy",
        &fixture(),
        &staging,
        "",
        &mut done,
        0,
        &mut failed,
    )
    .unwrap();
    assert!(failed.is_empty(), "准备阶段就不该失败：{failed:?}");

    let dst = temp_dir("move-dst");
    let report = migrate(None, &staging, &dst, true).unwrap();

    assert!(report.moved);
    assert!(!staging.exists(), "搬空后源目录该被删掉");
    assert!(dst.join(names::CONFIG_FILE).is_file());
    assert_eq!(report.instances_ok.len(), 3);

    cleanup(&[&dst]);
}

/// 复制时目标里的同名文件被覆盖，用户选的是"一律覆盖"
#[test]
fn copy_overwrites_same_named_file() {
    ensure_boot();
    let src = temp_dir("over-src");
    fs::create_dir_all(src.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR)).unwrap();
    fs::write(
        src.join(names::CONFIG_FILE),
        "{\"Version\":\"from-colormc\"}",
    )
    .unwrap();

    let dst = temp_dir("over-dst");
    fs::write(dst.join(names::CONFIG_FILE), "{\"Version\":\"from-mml\"}").unwrap();

    migrate(None, &src, &dst, false).unwrap();

    let text = fs::read_to_string(dst.join(names::CONFIG_FILE)).unwrap();
    assert!(
        text.contains("from-colormc"),
        "同名文件该被覆盖，实际：{text}"
    );

    cleanup(&[&src, &dst]);
}

/// 拒绝：目标 = 源 / 目标在源目录里面 / 目录不像 ColorMC 工作目录
#[test]
fn rejects_bad_source_and_nested_target() {
    ensure_boot();
    let src = temp_dir("bad-src");
    fs::create_dir_all(src.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR)).unwrap();
    let dst = temp_dir("bad-dst");

    assert!(
        migrate(None, &src, &src, false).is_err(),
        "目标就是源目录，该拒绝"
    );
    assert!(
        migrate(None, &src, &src.join("inner"), false).is_err(),
        "目标在源目录里面（会递归复制自己），该拒绝"
    );

    // 不像 ColorMC 的空目录
    let empty = temp_dir("bad-empty");
    assert!(migrate(None, &empty, &dst, false).is_err());

    cleanup(&[&src, &dst, &empty]);
}

/// 探测判据：只有 `auth.json` 的目录**不算**工作目录，`%LOCALAPPDATA%\ColorMC` 那种
///
/// ColorMC 只要跑过一次就会建出那个目录，里面是账户与方块解锁状态，与工作目录在哪无关，
/// 拿它当命中会误报 —— 这条用例把这个坑钉住。
#[test]
fn only_auth_json_is_not_a_working_dir() {
    ensure_boot();
    let dir = temp_dir("looks");
    fs::write(dir.join(names::AUTH_FILE), "[]").unwrap();
    assert!(!looks_like_colormc(&dir), "只有 auth.json 不该算工作目录");

    fs::create_dir_all(dir.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR)).unwrap();
    assert!(looks_like_colormc(&dir), "有 minecraft/instances 才算");

    cleanup(&[&dir]);
}

/// 选「不迁移」会写下标记：之后就是"问过了"，重启也不弹
///
/// 标记落在**运行目录**，测试里是 `test_support` boot 时那个临时目录。
/// 这条同时把"标记文件是唯一判据"钉住：删掉它就能重新触发询问。
#[test]
fn skip_writes_marker_and_stops_asking() {
    ensure_boot();
    let file = get_base_dir().join(names::COLORMC_MIGRATE_FILE);
    let _ = fs::remove_file(&file);
    assert!(!asked(), "标记文件在用例开始时该不存在");

    window_skip_colormc().unwrap();

    assert!(file.is_file(), "「不迁移」该写下标记：{}", file.display());
    assert!(asked(), "写过标记之后就算问过了");
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("skip"),
        "标记里该记下这次的选择，实际：{text}"
    );

    let _ = fs::remove_file(&file);
    assert!(!asked(), "删掉标记就该重新问");
}
