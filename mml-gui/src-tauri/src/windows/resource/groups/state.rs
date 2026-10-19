//! 分组口径：名称、顺序与状态
//!
//! 分组存在实例的设置里，这里只解释「这条分组是什么、排第几」。

use std::collections::HashSet;

use mml_game::GameInstance;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

use crate::dtos::ModGroupDto;


//
// 分组**不是 mod.rs 的数据**：它属于实例的 GUI 设置，即 `gui_setting.json` 的 `Mod.Groups`，
// 分组 uuid → 分组对象，见 crate::gui_setting。
// 本模块只做"读-改-写 + 转 DTO"：
// - 每个命令都是 load → 改 → save：那份文件还存着日志设置、方块图标等，不能整份覆盖
// - 分组顺序 = 用户自己拖出来的，存在 `gui_setting.json` 的 `Mod.GroupOrder`；
//   没存过则用默认顺序：识别失败 → 已启用 → 已禁用 → 自建分组按名字
// - 成员用 SHA1 而不是 uuid：启用/禁用会改文件名，uuid 跟着变，SHA1 不变


/// 三个**状态分组**的固定 uuid
///
/// 它们不是用户数据，所以不进 `gui_setting.json` 的 `Mod.Groups`；但要参与"顺序"与
/// "收起状态"，也就是用户能拖、能折叠，所以需要**稳定的键**。用固定 uuid 而不是
/// 原先的 `$on` / `$off` / `$fail` 字符串：
/// - 与自建分组同一套键形状，都是 uuid，前端不必维护"两套键"的映射；
/// - 建组时不可能撞上，用户分组是 `Uuid::new_v4()`；
/// - 看起来就是分组，不再是一串带 `$` 的魔法字符串。
///
/// 取值刻意用"全 0 / 尾号 1 / 尾号 2"这种一眼能认出的形式，与内核里
/// `DEFAULT_GROUP_UUID = Uuid::nil()` 同一套思路：调试时看到
/// `00000000-…-000000000001` 就知道是状态分组，不用去查表。
///
/// **一旦发布就不能改** —— 改了等于所有用户的状态分组顺序与折叠状态重置。
/// 前端 `resource/composables/useModGroups.ts` 的 `STATE_GROUP_ID_*` 与此一一对应，
/// 改要一起改。
pub(super) const STATE_GROUP_FAIL: &str = "00000000-0000-0000-0000-000000000000";

pub(super) const STATE_GROUP_ON: &str = "00000000-0000-0000-0000-000000000001";

pub(super) const STATE_GROUP_OFF: &str = "00000000-0000-0000-0000-000000000002";

/// 默认顺序：识别失败 → 已启用 → 已禁用，自建分组排在后面
///
/// 「识别失败」放最前是**用户点名的**：坏包要第一时间看见；其余按"启用 → 禁用"。
///
/// # 返回值
///
/// 三个状态分组的固定 uuid，按上述顺序排列
pub(super) fn default_group_order() -> Vec<String> {
    vec![
        STATE_GROUP_FAIL.to_string(),
        STATE_GROUP_ON.to_string(),
        STATE_GROUP_OFF.to_string(),
    ]
}

/// 是不是三个状态分组之一
///
/// - `key`: 分组键，也就是分组 uuid
///
/// # 返回值
///
/// 是状态分组返回 `true`
pub(super) fn is_state_group(key: &str) -> bool {
    key == STATE_GROUP_ON || key == STATE_GROUP_OFF || key == STATE_GROUP_FAIL
}

/// 分组块的完整顺序：状态分组 + 自建分组，**与真实存在的分组对齐**
///
/// - `mods`: 模组设置本体，也就是 `GameGuiSettingObj::mods`，不是整份 `gui_setting.json`
///
/// # 返回值
///
/// 规范化后的分组键列表：丢掉不存在的、补上没记的，自建分组按名字排在后面。
/// 存下来的顺序可能过时，分组删了 / 换了台机器 / 手改过文件都会出现这种偏差
pub(super) fn group_order_of(mods: &crate::gui_setting::GameModSettingObj) -> Vec<String> {
    // 自建分组：按名字排序当兜底顺序，HashMap 无序，总要有个确定的补位规则
    let mut custom: Vec<(String, String)> = mods
        .groups
        .iter()
        .map(|(uuid, group)| (group.name.clone(), uuid.clone()))
        .collect();
    custom.sort();

    let mut order: Vec<String> = Vec::with_capacity(custom.len() + 3);
    let mut seen: HashSet<String> = HashSet::new();
    for key in mods.group_order.iter() {
        // 状态分组照收，它们不在 Groups 里；自建分组只认当前存在的那些
        let known = is_state_group(key) || mods.groups.contains_key(key);
        if known && seen.insert(key.clone()) {
            order.push(key.clone());
        }
    }
    for key in default_group_order() {
        if seen.insert(key.clone()) {
            order.push(key);
        }
    }
    for (_, uuid) in custom {
        if seen.insert(uuid.clone()) {
            order.push(uuid);
        }
    }
    order
}

/// 模组的自定义分组，**按用户拖出来的顺序**下发
///
/// - `instance`: 实例设置本体
///
/// # 返回值
///
/// 自建分组列表，每项带 uuid、名字与成员 SHA1，成员已排序，展示才稳定
///
/// 顺序取自 [`group_order_of`]：它同时管自建分组与状态分组的排列，所以这里按它遍历、
/// 跳过状态分组即可 —— 状态分组不是用户数据，前端自己按固定 uuid 拼。
pub(super) fn mod_groups_of(instance: &InstanceSettingObj) -> Vec<ModGroupDto> {
    let setting = crate::gui_setting::load(instance);
    let mut list: Vec<ModGroupDto> = group_order_of(&setting.mods)
        .into_iter()
        .filter(|key| !is_state_group(key))
        .filter_map(|uuid| {
            let group = setting.mods.groups.get(&uuid)?;
            let mut mods: Vec<String> = group.mods.iter().cloned().collect();
            // HashSet 迭代顺序不定，排一下让前端展示稳定
            mods.sort();
            Some(ModGroupDto {
                uuid,
                name: group.name.clone(),
                mods,
            })
        })
        .collect();
    list.shrink_to_fit();
    list
}

/// 读出实例设置 → 交给 `edit` 改 → 存回去
///
/// - `instance`: 实例句柄
/// - `edit`: 就地修改模组设置那一段的回调
///
/// `gui_setting.json` 里除分组外还存着备注、日志设置、方块图标等，
/// 所以每个命令都是 load → 改 → save，**不能整份覆盖**。
pub(super) fn edit_mod_setting(
    instance: &GameInstance,
    edit: impl FnOnce(&mut crate::gui_setting::GameModSettingObj),
) {
    // 用**写锁**：这是一次"读文件 → 改 → 写回整份"的读-改-写，
    // 读锁下并发执行会互相覆盖 —— 拖分组顺序 / 折叠 / 移组 / 改备注同时点就会丢更新。
    // 写锁把整段串起来，代价只是这几个命令之间短暂排队。
    let game = instance.write().unwrap();
    let mut setting = crate::gui_setting::load(&game);
    edit(&mut setting.mods);
    crate::gui_setting::save(&game, &setting);
}
