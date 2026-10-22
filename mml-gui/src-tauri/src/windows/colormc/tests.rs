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

use serde_json::{Value, json};
use uuid::Uuid;

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

/// 现场造一个最小实例：目录 + `game.json`
///
/// - `root`: 工作目录，实例落在 `minecraft/instances/<dir>`
/// - `dir`: 实例目录名
/// - `uuid`: 实例 uuid
/// - `name`: 实例显示名
/// - `group`: ColorMC 的组名，`None` 表示没有
///
/// # 返回值
///
/// 实例目录路径
fn write_instance(root: &Path, dir: &str, uuid: Uuid, name: &str, group: Option<&str>) -> PathBuf {
    let path = root
        .join(names::MINECRAFT_DIR)
        .join(names::INSTANCE_DIR)
        .join(dir);
    fs::create_dir_all(&path).unwrap();

    let mut game = json!({
        "UUID": uuid.to_string(),
        "Name": name,
        "DirName": dir,
        "Version": "1.20.4",
        "Loader": 0,
        "Encoding": 0,
    });
    if let Some(group) = group {
        game["GroupName"] = json!(group);
    }
    fs::write(
        path.join(names::GAME_FILE),
        serde_json::to_string_pretty(&game).unwrap(),
    )
    .unwrap();

    path
}

/// 读实例目录里的 `game.json`
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 解析出的 JSON
fn read_instance(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join(names::GAME_FILE)).unwrap()).unwrap()
}

/// 取实例配置里的字符串字段
fn field<'a>(game: &'a Value, key: &str) -> Option<&'a str> {
    game.get(key).and_then(Value::as_str)
}

/// 取实例配置里的 uuid
fn uuid_of(game: &Value) -> Option<Uuid> {
    field(game, "UUID").and_then(|uuid| Uuid::parse_str(uuid).ok())
}

/// 按组名找分组 uuid，找不到返回空串
fn group_key(saved: &Value, name: &str) -> String {
    saved["Names"]
        .as_object()
        .map(|names| names.iter())
        .into_iter()
        .flatten()
        .find(|(_, value)| value.as_str() == Some(name))
        .map(|(key, _)| key.clone())
        .unwrap_or_default()
}

/// 某分组里的实例 uuid
fn members(saved: &Value, key: &str) -> Vec<String> {
    saved["Groups"][key]
        .as_array()
        .map(|items| items.iter())
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
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
    let instances = list_instance_dirs(&instance_root(&src));
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
    let source_game = fixture()
        .join(names::MINECRAFT_DIR)
        .join(names::INSTANCE_DIR)
        .join("1.21.11-NeoForge-21.11.38-beta")
        .join(names::GAME_FILE);
    let before: InstanceSettingObj = serialize_tools::json_from_file(&source_game).unwrap();
    let after: InstanceSettingObj = serialize_tools::json_from_file(&game).unwrap();
    // 没有重名、也没有 uuid 冲突时，实例身份原样保留
    assert_eq!(after.uuid, before.uuid, "普通迁移不该改实例 uuid");

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

/// 同名实例不覆盖：导入的那个改名保留两份，uuid 冲突只换导入的
#[test]
fn same_named_instance_is_renamed_and_kept() {
    ensure_boot();
    let src = temp_dir("inst-src");
    let dst = temp_dir("inst-dst");

    let old = Uuid::new_v4();
    write_instance(&src, "共享", old, "共享", Some("导入组"));
    // 目标里已经有一个同名实例，uuid 还与源里的一样
    write_instance(&dst, "共享", old, "共享", None);

    migrate(None, &src, &dst, false).unwrap();

    let root = dst.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR);

    // 原有的那份原封不动
    let kept = read_instance(&root.join("共享"));
    assert_eq!(uuid_of(&kept), Some(old), "原有实例的 uuid 不该改");

    // 导入的那份改名保留，uuid 换新
    let imported_dir = root.join("共享1");
    assert!(
        imported_dir.join(names::GAME_FILE).is_file(),
        "重名实例应改名保留：{}",
        imported_dir.display()
    );
    let imported = read_instance(&imported_dir);
    assert_eq!(field(&imported, "Name"), Some("共享1"));
    assert_eq!(field(&imported, "DirName"), Some("共享1"));
    let imported_uuid = uuid_of(&imported).expect("导入的实例该有 uuid");
    assert_ne!(Some(imported_uuid), Some(old), "uuid 冲突时只换导入的那个");

    // ColorMC 的组名跟着导入的实例走：进组的是换过的新 uuid
    let saved: Value =
        serde_json::from_str(&fs::read_to_string(root.join(names::GROUP_FILE)).unwrap()).unwrap();
    let key = group_key(&saved, "导入组");
    assert!(!key.is_empty(), "导入的组该建出来");
    assert!(
        members(&saved, &key).contains(&imported_uuid.to_string()),
        "导入的实例该进「导入组」"
    );

    cleanup(&[&src, &dst]);
}

/// 只是 uuid 冲突、目录不重名：目录不动，只给导入的实例换 uuid
#[test]
fn uuid_conflict_only_replaces_imported_uuid() {
    ensure_boot();
    let src = temp_dir("uuid-src");
    let dst = temp_dir("uuid-dst");

    let old = Uuid::new_v4();
    write_instance(&src, "导入", old, "导入", None);
    write_instance(&dst, "原有", old, "原有", None);

    migrate(None, &src, &dst, false).unwrap();

    let root = dst.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR);
    assert!(root.join("导入").is_dir(), "目录不重名时不该改名");

    let imported = read_instance(&root.join("导入"));
    assert_ne!(
        uuid_of(&imported),
        Some(old),
        "uuid 冲突时要给导入的实例换新"
    );
    assert_eq!(
        field(&imported, "Name"),
        Some("导入"),
        "没重名就不该改显示名"
    );

    let kept = read_instance(&root.join("原有"));
    assert_eq!(uuid_of(&kept), Some(old), "原有实例的 uuid 不该改");

    cleanup(&[&src, &dst]);
}

/// 移动模式同样保留同名实例
#[test]
fn move_keeps_same_named_instance() {
    ensure_boot();
    let src = temp_dir("move-inst-src");
    let dst = temp_dir("move-inst-dst");

    let kept_uuid = Uuid::new_v4();
    let moved_uuid = Uuid::new_v4();
    write_instance(&src, "共享", moved_uuid, "共享", None);
    write_instance(&dst, "共享", kept_uuid, "共享", None);

    migrate(None, &src, &dst, true).unwrap();

    let root = dst.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR);
    assert_eq!(uuid_of(&read_instance(&root.join("共享"))), Some(kept_uuid));
    let imported_dir = root.join("共享1");
    assert!(imported_dir.join(names::GAME_FILE).is_file());
    assert_eq!(uuid_of(&read_instance(&imported_dir)), Some(moved_uuid));
    assert!(!src.exists(), "搬空后源目录该被删掉");

    cleanup(&[&dst]);
}

/// ColorMC 的 GroupName 合并进 group_save.json：同名组复用，没有的新建
#[test]
fn group_names_merge_into_group_save() {
    ensure_boot();
    let src = temp_dir("group-src");
    let dst = temp_dir("group-dst");

    let uuid_a = Uuid::new_v4();
    let uuid_b = Uuid::new_v4();
    let uuid_c = Uuid::new_v4();
    write_instance(&src, "A", uuid_a, "A", Some(" 经典 "));
    write_instance(&src, "B", uuid_b, "B", Some("新组"));
    write_instance(&src, "C", uuid_c, "C", None);

    // 目标里已经有一组「经典」：导入的应复用它，而不是再建一个同名组
    let root = dst.join(names::MINECRAFT_DIR).join(names::INSTANCE_DIR);
    fs::create_dir_all(&root).unwrap();
    let exist = Uuid::new_v4();
    let default_key = Uuid::nil().to_string();
    let exist_key = exist.to_string();
    let mut names = serde_json::Map::new();
    names.insert(default_key.clone(), json!(""));
    names.insert(exist_key.clone(), json!("经典"));
    let mut groups = serde_json::Map::new();
    groups.insert(default_key.clone(), json!([]));
    groups.insert(exist_key.clone(), json!([]));
    let before = json!({
        "GroupOrder": [default_key.clone(), exist_key.clone()],
        "Names": Value::Object(names),
        "Groups": Value::Object(groups),
        "Order": {},
    });
    fs::write(
        root.join(names::GROUP_FILE),
        serde_json::to_string_pretty(&before).unwrap(),
    )
    .unwrap();

    migrate(None, &src, &dst, false).unwrap();

    let saved: Value =
        serde_json::from_str(&fs::read_to_string(root.join(names::GROUP_FILE)).unwrap()).unwrap();

    // 同名组复用
    assert_eq!(group_key(&saved, "经典"), exist_key, "同名组该复用");
    assert!(
        members(&saved, &exist_key).contains(&uuid_a.to_string()),
        "导入的实例该进「经典」"
    );
    // 新组按需创建
    let new_key = group_key(&saved, "新组");
    assert!(!new_key.is_empty(), "没有的组该新建");
    assert!(
        members(&saved, &new_key).contains(&uuid_b.to_string()),
        "导入的实例该进「新组」"
    );
    // 没有组名的实例不进任何非默认组
    for key in saved["Groups"].as_object().unwrap().keys() {
        if key != &default_key {
            assert!(
                !members(&saved, key).contains(&uuid_c.to_string()),
                "没有组名的实例不该被塞进「{}」",
                saved["Names"][key]
            );
        }
    }

    cleanup(&[&src, &dst]);
}
