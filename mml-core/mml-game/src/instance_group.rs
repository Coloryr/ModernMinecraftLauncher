//! 实例分组 API
//!
//! 分组表在 `game_group`（独立 `group_save.json`），这里只做对外的查询 / 移动 / 增删包装，
//! 并在移动后按实际归属发 `MoveGroup` 事件。旧路径由 `lib.rs` 再导出。

use uuid::Uuid;

use crate::game_group;
use crate::instance_event::{InstanceChange, invoke_change};
use crate::{GameInstance, get_instance, have_instance_uuid};

/// 获取所有分组，含空分组
///
/// # 返回值
///
/// 返回分组条目列表，每项含 uuid 与显示名；默认分组置顶，其余按用户排定的顺序
pub fn get_group_list() -> Vec<game_group::GroupInfoObj> {
    game_group::group_list()
}

/// 取某个实例所属的分组 uuid
///
/// 分组归属存在独立的 `group_save.json`，实例配置里没有这个字段，
/// 所以想显示"这个实例在哪个组"必须来这里查。
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 返回分组 uuid；属于默认分组时返回 `None`，即"没有分组"就是默认分组的表示
pub fn get_instance_group(uuid: &Uuid) -> Option<Uuid> {
    let group = game_group::group_of(uuid);
    if group == game_group::DEFAULT_GROUP_UUID {
        None
    } else {
        Some(group)
    }
}

/// 取某个实例在它所属分组里的显示次序，从 0 起
///
/// 组内次序存在独立的 `group_save.json` 里，键是实例 uuid 到次序的映射，
/// 不再写各实例的 `gui_setting.json`。
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 返回组内位置；实例不在任何分组里时返回 `None`
pub fn get_instance_order(uuid: &Uuid) -> Option<i32> {
    game_group::index_of(uuid)
}

/// 按给定顺序重排分组，默认分组只是初始排首位，同样可以换位置
///
/// - `order`: 期望的分组 uuid 顺序；不存在的忽略，没提到的组保持在后
pub fn reorder_groups(order: &[Uuid]) {
    game_group::reorder_groups(order);
}

/// 把某个实例移到分组的第 `index` 位，跨组移动与组内排序一次到位
///
/// 归属与组内次序都在 `group_save.json`：拖拽落点既可能换组、也可能只是同组内换位置，
/// 统一由这一个入口处理；移动完成后发 `MoveGroup` 事件。
///
/// - `uuid`: 实例 UUID；不存在时什么都不做，也不发事件
/// - `group`: 目标分组 uuid；`None` 表示默认分组
/// - `index`: 目标下标；超出范围则排到末尾
pub fn move_instance(uuid: &Uuid, group: Option<Uuid>, index: usize) {
    if !have_instance_uuid(uuid) {
        return;
    }

    game_group::place_in_group(uuid, group, index);

    // 用回读到的实际分组发事件：目标组不存在时内核会落到默认分组，
    // 拿传进来的那个 uuid 发就跟实际状态不符了
    invoke_change(InstanceChange::MoveGroup(*uuid, get_instance_group(uuid)));
}

/// 从分组 uuid 获取对应的实例
///
/// - `uuid`: 分组 uuid
///
/// # 返回值
///
/// 返回该分组下的实例列表，组内按用户排定的次序
pub fn get_group(uuid: &Uuid) -> Vec<GameInstance> {
    let mut list = Vec::new();

    for item in game_group::group_items(uuid) {
        if let Some(instance) = get_instance(&item) {
            list.push(instance.clone());
        }
    }

    list
}

/// 添加分组
///
/// - `name`: 分组名
///
/// # 返回值
///
/// 添加成功返回新分组的 uuid；分组已存在或名字为空返回 `None`
pub fn add_group(name: &str) -> Option<Uuid> {
    game_group::add_group(name)
}

/// 删除分组
///
/// - `uuid`: 分组 uuid
///
/// # 返回值
///
/// 删除成功返回 `true`，组内实例移入默认分组；分组不存在或为默认分组返回 `false`
pub fn remove_group(uuid: &Uuid) -> bool {
    // 只通知**被移入默认分组的那些**实例，不是整个默认分组
    let Some(moved) = game_group::remove_group(uuid) else {
        return false;
    };
    for instance in moved {
        invoke_change(InstanceChange::MoveGroup(instance, None));
    }
    true
}

/// 移动实例到分组
///
/// - `list`: 需要移动的实例列表
/// - `new`: 目标分组 uuid；`None` 表示默认分组
///
/// 移动完成后为每个实例发一次 `MoveGroup` 事件。
///
/// 分组表是独立存储的 `group_save.json`，实例配置里不再有 group 字段，
/// 所以这里只需改表与通知，不必逐个 `save()` 实例。
pub fn move_group(list: Vec<Uuid>, new: Option<Uuid>) {
    // 只处理真实存在的实例，避免把不存在的 uuid 写进分组表
    let moved: Vec<Uuid> = list.into_iter().filter(|u| have_instance_uuid(u)).collect();
    if moved.is_empty() {
        return;
    }

    game_group::move_group(moved.clone(), new);

    // 回读实际分组发事件，目标组不存在时内核会把实例落到默认分组
    for uuid in moved {
        invoke_change(InstanceChange::MoveGroup(uuid, get_instance_group(&uuid)));
    }
}
