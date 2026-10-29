//! 分组的增删改与顺序命令
//!
//! 改动落 `gui_setting.json`，再回写窗口 model 并通知前端。

use std::collections::HashSet;

use uuid::Uuid;

use crate::dtos::ModGroupDto;
use crate::err_keys;

use crate::windows::resource::mods::mod_note_key;
use crate::windows::resource::parse_instance;

use super::state::{default_group_order, edit_mod_setting, group_order_of, mod_groups_of};

/// 取某个实例的模组分组
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 自建分组列表；实例不存在返回 `err.gameNotFound`
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_groups(uuid: String) -> Result<Vec<ModGroupDto>, String> {
    let instance = parse_instance(&uuid)?;
    let game = instance.read().unwrap();

    Ok(mod_groups_of(&game))
}

/// 取分组块的**完整顺序**，含三个状态分组的固定 uuid
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// [`group_order_of`] 规范化后的顺序：与真实存在的分组对齐、状态分组按固定顺序补齐，
/// 前端可以**直接当顺序用**，不用再拼一遍
///
/// **必须从后端读**：状态分组不是用户数据、不进 `Mod.Groups`，`resource_mod_groups`
/// 里也没有它们。前端自己拼的话，永远拼不出"用户把「已启用」拖到了某个自建分组前面"
/// 这件事 —— 表现就是拖完松手又弹回原位，也就是用户报的"模组分组无法移动顺序"。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_order(uuid: String) -> Result<Vec<String>, String> {
    let instance = parse_instance(&uuid)?;
    let mods = crate::gui_setting::load(&instance.read().unwrap()).mods;

    Ok(group_order_of(&mods))
}

/// 取**收起**的分组块键集合，与 `resource_mod_groups` 一起在进模组页时读
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 收起状态的分组键；已经删掉的分组不会出现在里面
///
/// 单独一条命令而不是塞进 `ModGroupDto`：那一份是"分组 → 成员"的数据结构，
/// 收起状态是窗口级的视图状态，混在一起会让 DTO 的语义变浑。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_groups_collapsed(uuid: String) -> Result<Vec<String>, String> {
    let instance = parse_instance(&uuid)?;
    let mods = crate::gui_setting::load(&instance.read().unwrap()).mods;

    // 按当前存在的分组过滤：分组删了之后，它残留的键不该再冒出来
    let known: HashSet<String> = group_order_of(&mods).into_iter().collect();
    Ok(collapsed_of(&mods)
        .into_iter()
        .filter(|key| known.contains(key))
        .collect())
}

/// 收起的分组键：没写过就用初值，写过就照用户存的来
///
/// 初值复用 [`default_group_order`]，不在这里另写一份"有哪些分组"。
/// 不把它写进 `GameModSettingObj::default` 是因为那个默认值只在**整块 `Mod` 字段缺失**
/// 时生效；老文件有 `Mod`、只是没有 `GroupCollapsed`，走的是字段级默认，空表等于全展开，
/// 用户会觉得"我明明收起过"。所以这里按"有没有写过"分情况。
///
/// **自建分组不在初值里**：它们是用户自己建的，建出来时展开更顺手，空组会显示
/// "把模组拖上来即可"的落点区，一眼知道能往里放。
///
/// - `mods`: 模组设置本体
///
/// # 返回值
///
/// 收起的分组键列表；`group_collapsed` 没写过时默认全部收起
pub(super) fn collapsed_of(mods: &crate::gui_setting::GameModSettingObj) -> Vec<String> {
    mods.group_collapsed
        .clone()
        .unwrap_or_else(default_group_order)
}

/// 新建模组分组，重名返回错误，前端据此提示
///
/// - `uuid`: 实例 uuid
/// - `name`: 分组名，前后空白会被去掉
///
/// # 返回值
///
/// **新建分组的 uuid**，前端拿它拼顺序表 / 折叠集合，也用它继续操作这个分组，
/// 不再像以前那样靠"组名"间接指代；名字为空返回 `err.groupEmpty`，
/// 重名返回 `err.groupExists`
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_add(uuid: String, name: String) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(String::from(err_keys::GROUP_EMPTY));
    }
    let instance = parse_instance(&uuid)?;
    if mod_groups_of(&instance.read().unwrap())
        .iter()
        .any(|item| item.name == name)
    {
        return Err(String::from(err_keys::GROUP_EXISTS));
    }

    // uuid v4：分组身份与名字解耦，改名不动键
    let group_uuid = Uuid::new_v4().to_string();
    let key = group_uuid.clone();
    edit_mod_setting(&instance, |mods| {
        mods.groups.insert(
            key.clone(),
            crate::gui_setting::GameModGroupObj {
                name: name.to_string(),
                mods: Default::default(),
            },
        );
    });

    Ok(group_uuid)
}

/// 删除模组分组，组内模组回到"未分组"，磁盘上的文件一个都不动
///
/// - `uuid`: 实例 uuid
/// - `group`: 分组 uuid
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_remove(uuid: String, group: String) {
    let Ok(instance) = parse_instance(&uuid) else {
        return;
    };
    edit_mod_setting(&instance, |mods| {
        mods.groups.remove(&group);
        // 顺序表 / 折叠集合里的这个 uuid 一起去掉：留着就是脏数据
        mods.group_order.retain(|key| key != &group);
        if let Some(collapsed) = mods.group_collapsed.as_mut() {
            collapsed.retain(|key| key != &group);
        }
    });
}

/// 重命名模组分组，**只改名字，键与成员都不动**
///
/// - `uuid`: 实例 uuid
/// - `group`: 分组 uuid
/// - `new_name`: 新分组名，前后空白会被去掉
///
/// # 返回值
///
/// 只表示成功 / 失败：名字为空返回 `err.groupEmpty`，与别的分组重名返回 `err.groupExists`
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_rename(
    uuid: String,
    group: String,
    new_name: String,
) -> Result<(), String> {
    let new_name = new_name.trim();

    if new_name.is_empty() {
        return Err(String::from(err_keys::GROUP_EMPTY));
    }
    let instance = parse_instance(&uuid)?;
    // 重名检查要排除它自己，改回原名 / 只改大小写不该报"已存在"
    if mod_groups_of(&instance.read().unwrap())
        .iter()
        .any(|item| item.uuid != group && item.name == new_name)
    {
        return Err(String::from(err_keys::GROUP_EXISTS));
    }

    edit_mod_setting(&instance, |mods| {
        if let Some(target) = mods.groups.get_mut(&group) {
            target.name = new_name.to_string();
        }
    });

    Ok(())
}

/// 保存分组块的显示顺序，由用户拖出来
///
/// - `uuid`: 实例 uuid
/// - `order`: **分组 uuid** 数组，与 [`group_order_of`] 同一套口径，状态分组用
///   `super::state::STATE_GROUP_ON` 那几个固定 uuid。接进来先规范化一次再落盘：前端可能因为
///   版本差异多传 / 少传了键，存脏数据的话下次读出来还得再纠一遍
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_order_set(uuid: String, order: Vec<String>) {
    let Ok(instance) = parse_instance(&uuid) else {
        return;
    };

    edit_mod_setting(&instance, |mods| {
        // 借当前的分组表跑一遍规范化再落盘，免得把过时的键存进去
        let probe = crate::gui_setting::GameModSettingObj {
            groups: mods.groups.clone(),
            mod_name: Default::default(),
            mod_source: Default::default(),
            group_order: order,
            group_collapsed: Default::default(),
        };
        mods.group_order = group_order_of(&probe);
    });
}

/// 保存**收起**的分组块键集合，由用户点分组头折叠出来
///
/// - `uuid`: 实例 uuid
/// - `collapsed`: 收起的分组 uuid 数组，与 [`resource_mod_group_order_set`] 同一套口径。
///   与顺序一样按当前真实存在的分组过滤一遍：分组删了以后，它的键不该留在文件里
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_collapsed_set(uuid: String, collapsed: Vec<String>) {
    let Ok(instance) = parse_instance(&uuid) else {
        return;
    };

    edit_mod_setting(&instance, |mods| {
        let known: HashSet<String> = group_order_of(mods).into_iter().collect();
        // 前端传来的顺序不保证相邻重复，按出现顺序去重而不是 `dedup` 只清相邻项
        let mut seen: HashSet<String> = HashSet::new();
        let kept: Vec<String> = collapsed
            .into_iter()
            .filter(|key| known.contains(key) && seen.insert(key.clone()))
            .collect();
        // 存 Some：哪怕是空数组也代表"用户明确展开了全部"，
        // 不能写回 None，那会让下次读出来又是"已启用默认收起"
        mods.group_collapsed = Some(kept);
    });
}

/// 把若干模组移到某个分组；`group` 为空 / null = 移出所有分组，回到"未分组"
///
/// - `uuid`: 实例 uuid
/// - `group`: 目标分组 uuid；`None` 或空白表示移出所有分组
/// - `keys`: 模组的 SHA1 列表
///
/// 移动语义：先从其它组里摘掉，再进目标组 —— 一个模组同时只属于一个组。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_group_set(uuid: String, group: Option<String>, keys: Vec<String>) {
    let Ok(instance) = parse_instance(&uuid) else {
        return;
    };
    let group = group
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string);

    // 目标组不存在就直接返回，别把它们从原组摘出来之后无处可去；
    // 先判再改，省掉一次没有改动的写盘
    if let Some(key) = &group {
        let exists = crate::gui_setting::load(&instance.read().unwrap())
            .mods
            .groups
            .contains_key(key);
        if !exists {
            return;
        }
    }

    edit_mod_setting(&instance, |mods| {
        for (key, target) in mods.groups.iter_mut() {
            if Some(key) == group.as_ref() {
                continue;
            }
            for sha1 in &keys {
                target.mods.remove(sha1);
            }
        }

        if let Some(key) = &group
            && let Some(target) = mods.groups.get_mut(key)
        {
            for sha1 in &keys {
                target.mods.insert(sha1.clone());
            }
        }
    });
}

/// 写某个模组的备注，传空串 = 删掉这条备注
///
/// - `uuid`: 实例 uuid
/// - `file`: 列表里的**原始文件名**，可能带 `.disabled`；落盘时按 [`mod_note_key`]
///   归一成"启用时的文件名"，这样启用 / 禁用来回切，备注都跟着走
/// - `note`: 备注文本，前后空白会被去掉；空串表示删除
///
/// 归一前后的键都写不到旧值时，顺手把原始文件名下的那条一起清掉 ——
/// 否则 ColorMC 在禁用状态下写的备注会和新写的并存，"看着改了其实没改"。
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub fn resource_mod_note_set(uuid: String, file: String, note: String) {
    let Ok(instance) = parse_instance(&uuid) else {
        return;
    };
    let note = note.trim().to_string();
    let key = mod_note_key(&file).to_string();
    if key.is_empty() {
        return;
    }

    edit_mod_setting(&instance, |mods| {
        mods.mod_name.remove(&file);
        if note.is_empty() {
            mods.mod_name.remove(&key);
        } else {
            mods.mod_name.insert(key, Some(note));
        }
    });
}
