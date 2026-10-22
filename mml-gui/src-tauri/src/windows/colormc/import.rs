//! 实例目录的导入计划：重名改名、uuid 冲突换新、组名并入分组表
//!
//! 迁移对普通条目是"原样覆盖"，实例却不行：M²L 以 uuid 为身份、以目录名为地址。
//! 同名覆盖会让两份只剩一份；uuid 重复则会让后登记的那个在启动时被换掉。
//! 所以 `minecraft/instances` 单独走这里：搬运前按目标现状算好计划，
//! 搬运后把 `UUID` / `Name` / `DirName` 写回 `game.json`，
//! 最后把 ColorMC 的 `GroupName` 合并进 `group_save.json`。
//!
//! 计划**只读**目标目录：改名与换 uuid 的具体值在搬运前定下来，
//! 搬运时照着落，搬完再写回。这样移动模式下源目录后来没了也不影响。

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use uuid::Uuid;

use mml_base::serialize_tools;
use mml_game::game_group::{self, ImportedGroup};
use mml_names::names;
use mml_sys::path_helper;

use super::detect::list_instance_dirs;

/// 一个待导入实例的落地计划
pub(super) struct InstancePlan {
    /// 源实例目录名，即 `minecraft/instances/<source>`
    pub(super) source: String,
    /// 目标实例目录名；与已有实例重名时改过
    pub(super) dir: String,
    /// 目标实例名，写回 `game.json` 的 `Name`
    pub(super) name: String,
    /// 目标实例 uuid，写回 `game.json` 的 `UUID`；与已有实例冲突或源里为空时是新生成的
    pub(super) uuid: Uuid,
    /// ColorMC 的组名，已去首尾空白；`None` 表示默认分组
    pub(super) group: Option<String>,
}

/// 目标实例根目录里已经占用的身份
#[derive(Default)]
struct Taken {
    /// 已存在的条目名，文件也算：同名目录写不进去
    dirs: Vec<String>,
    /// 已存在的显示名，来自可解析的 `game.json`
    names: Vec<String>,
    /// 已存在的实例 uuid
    uuids: Vec<Uuid>,
}

/// 按源实例目录与目标现状算出每个实例的目标目录 / 名字 / uuid / 分组
///
/// 只读，不落盘。重名与 uuid 冲突都在这里定下来，搬运时照着落。
///
/// - `src_root`: ColorMC 的 `minecraft/instances`
/// - `dst_root`: M²L 的 `minecraft/instances`
///
/// # 返回值
///
/// 按源目录名排序的计划表；源目录读不到时返回空表
pub(super) fn plan_instances(src_root: &Path, dst_root: &Path) -> Vec<InstancePlan> {
    let mut taken = scan_taken(dst_root);
    let sources = list_instance_dirs(src_root);

    let mut plans = Vec::with_capacity(sources.len());
    for source in sources {
        let game = read_game(&src_root.join(&source));

        let base = game
            .as_ref()
            .and_then(|value| text(value, "Name"))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(source.as_str())
            .to_string();
        let group = game
            .as_ref()
            .and_then(|value| text(value, "GroupName"))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);

        let (name, dir) = free_name(&base, &source, &taken);
        taken.names.push(name.clone());
        taken.dirs.push(dir.clone());

        // 源 uuid 只在既非空、又没被目标里的实例占着时保留：换掉的只能是导入的这个
        let old = game.as_ref().and_then(game_uuid);
        let mut uuid = match old {
            Some(uuid) if !uuid.is_nil() && !taken.uuids.contains(&uuid) => uuid,
            _ => Uuid::new_v4(),
        };
        while taken.uuids.contains(&uuid) {
            uuid = Uuid::new_v4();
        }
        taken.uuids.push(uuid);

        plans.push(InstancePlan {
            source,
            dir,
            name,
            uuid,
            group,
        });
    }

    plans
}

/// 把计划写回实例的 `game.json`，内容已经一致时不重写
///
/// - `dir`: 目标实例目录
/// - `plan`: 该实例的计划
///
/// # 返回值
///
/// 写成功返回 `true`；没有 `game.json` 或读不出来返回 `false`，内容没变也不动文件；
/// 写盘失败返回 `Err`，内容为错误原文
pub(super) fn write_back(dir: &Path, plan: &InstancePlan) -> Result<bool, String> {
    let file = dir.join(names::GAME_FILE);
    let Ok(mut game) = serialize_tools::json_from_file::<Value>(&file) else {
        return Ok(false);
    };
    let Some(obj) = game.as_object_mut() else {
        return Ok(false);
    };

    let same = obj
        .get("UUID")
        .and_then(Value::as_str)
        .and_then(|text| Uuid::parse_str(text).ok())
        == Some(plan.uuid)
        && obj.get("Name").and_then(Value::as_str) == Some(plan.name.as_str())
        && obj.get("DirName").and_then(Value::as_str) == Some(plan.dir.as_str());
    if same {
        return Ok(true);
    }

    obj.insert(String::from("UUID"), json!(plan.uuid.to_string()));
    obj.insert(String::from("Name"), json!(plan.name.as_str()));
    obj.insert(String::from("DirName"), json!(plan.dir.as_str()));
    serialize_tools::json_to_file(&game, &file)
        .map(|()| true)
        .map_err(|err| err.to_string())
}

/// 把导入实例的组名合并进目标分组表
///
/// - `dst_root`: M²L 的 `minecraft/instances`
/// - `items`: 已成功落地的实例与其组名
///
/// # 返回值
///
/// 合并失败返回 `Err`，内容为错误原文；没有带名字的组时什么都不做
pub(super) fn merge_groups(dst_root: &Path, items: &[ImportedGroup]) -> Result<(), String> {
    if !items.iter().any(|item| {
        item.group
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty())
    }) {
        return Ok(());
    }

    game_group::merge_imported(&dst_root.join(names::GROUP_FILE), items)
        .map_err(|err| err.to_string())
}

/// 扫目标实例根目录，收集已经占用的目录名 / 显示名 / uuid
///
/// - `root`: M²L 的 `minecraft/instances`
///
/// # 返回值
///
/// 目标里已被占用的身份；目录读不到时返回空表
fn scan_taken(root: &Path) -> Taken {
    let mut taken = Taken::default();
    let Ok(reader) = fs::read_dir(root) else {
        return taken;
    };

    for entry in reader.flatten() {
        let path = entry.path();
        let dir = entry.file_name().to_string_lossy().to_string();
        if path.is_dir()
            && let Some(game) = read_game(&path)
        {
            if let Some(name) = text(&game, "Name") {
                taken.names.push(name.to_string());
            }
            if let Some(uuid) = game_uuid(&game) {
                taken.uuids.push(uuid);
            }
        }
        taken.dirs.push(dir);
    }

    taken
}

/// 读实例目录里的 `game.json`
///
/// - `dir`: 实例目录
///
/// # 返回值
///
/// 解析出的 JSON；文件不存在或不是合法 JSON 时返回 `None`
fn read_game(dir: &Path) -> Option<Value> {
    serialize_tools::json_from_file::<Value>(dir.join(names::GAME_FILE)).ok()
}

/// 取 `game.json` 里的字符串字段
///
/// - `value`: `game.json` 的内容
/// - `key`: 字段名，如 `Name` / `GroupName`
///
/// # 返回值
///
/// 字段存在且是字符串时返回它，否则返回 `None`
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// 取 `game.json` 里的实例 uuid
///
/// - `game`: `game.json` 的内容
///
/// # 返回值
///
/// 字段缺失、不是字符串或解析失败时返回 `None`
fn game_uuid(game: &Value) -> Option<Uuid> {
    text(game, "UUID").and_then(|text| Uuid::parse_str(text).ok())
}

/// 给导入实例挑一个不重名的显示名与目录名
///
/// 目录或显示名任一被占用时一起加序号；两者都空着就保持原目录名，
/// 没冲突的实例搬过来还是原来的目录。
///
/// - `base`: 源实例的显示名，空白时调用方已回落到目录名
/// - `source`: 源实例目录名
/// - `taken`: 目标里已占用的目录名 / 显示名
///
/// # 返回值
///
/// 目标显示名与目录名；序号被占满时退化为带 uuid 后缀的名字
fn free_name(base: &str, source: &str, taken: &Taken) -> (String, String) {
    let source_taken = taken
        .dirs
        .iter()
        .any(|dir| dir.eq_ignore_ascii_case(source));

    for index in 0..=9999u32 {
        let name = if index == 0 {
            base.to_string()
        } else {
            format!("{base}{index}")
        };
        // 目录没被占时保持源目录名；一旦冲突就跟着名字走，别让两份挤进同一个目录
        let dir = if !source_taken && index == 0 {
            source.to_string()
        } else {
            path_helper::replace_path_name(&name)
        };

        let name_taken = taken
            .names
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&name));
        let dir_taken = taken
            .dirs
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&dir));
        if !name_taken && !dir_taken {
            return (name, dir);
        }
    }

    let suffix: String = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect();
    (format!("{base}-{suffix}"), format!("{source}-{suffix}"))
}
