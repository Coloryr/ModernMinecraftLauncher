use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::launcher::instance_setting_obj::InstanceSettingObj;

use crate::*;

pub fn get_instances() -> Vec<GameInstance> {
    let mut list = Vec::new();

    for (_, value) in INSTANCES.read().unwrap().iter() {
        list.push(value.clone());
    }

    list
}

/// 从uuid获取实例
///
/// - `uuid`: 实例 UUID
///
/// # 返回值
///
/// 返回实例；不存在返回 `None`
pub fn get_instance(uuid: &Uuid) -> Option<GameInstance> {
    let list = INSTANCES.read().unwrap();

    Some(list.get(uuid)?.clone())
}

/// 获取游戏版本类型列表，取 Mojang 版本清单的 type 字段
///
/// 返回独立 ID：release / snapshot / old_beta / old_alpha，
/// 显示名由前端 i18n 翻译。
///
/// # 返回值
///
/// 返回版本类型 ID 列表
pub fn get_version_types() -> Vec<&'static str> {
    vec!["release", "snapshot", "old_beta", "old_alpha"]
}

/// 从实例名字获取实例
///
/// - `name`: 实例名字
///
/// # 返回值
///
/// 返回实例；不存在返回 `None`
pub fn get_instance_by_name(name: &str) -> Option<GameInstance> {
    let list = INSTANCES.read().unwrap();
    let temp = list
        .iter()
        .filter(|(_, value)| value.read().unwrap().name.eq_ignore_ascii_case(name))
        .next()?;

    Some(temp.1.clone())
}

/// 是否存在这个名字的实例
///
/// - `name`: 实例名字
///
/// # 返回值
///
/// 存在返回 `true`
pub fn have_instance_name(name: &str) -> bool {
    let list = INSTANCES.read().unwrap();
    let temp = list
        .iter()
        .filter(|(_, value)| value.read().unwrap().name.eq_ignore_ascii_case(name))
        .next();
    !temp.is_none()
}

/// 是否存在这个 uuid 的实例
///
/// - `uuid`: 实例标识
///
/// # 返回值
///
/// 存在返回 `true`
pub fn have_instance_uuid(uuid: &Uuid) -> bool {
    let list = INSTANCES.read().unwrap();
    list.contains_key(uuid)
}

/// 将实例添加到分组中
///
/// UUID 为空或冲突时自动重新生成。
///
/// - `obj`: 实例配置，其中**没有**分组字段
/// - `group`: 要登记到哪个分组，给分组 uuid；`None` 表示默认分组
///
/// # 返回值
///
/// 返回加入列表后的实例句柄
pub(crate) fn add_to_group(obj: InstanceSettingObj, group: Option<Uuid>) -> GameInstance {
    let mut obj = obj;
    while obj.uuid.is_nil() || matches!(get_instance(&obj.uuid), Some(_)) {
        obj.uuid = Uuid::new_v4();
    }

    obj.save();
    let key = obj.uuid.clone();
    let name = obj.name.clone();
    let game: Arc<RwLock<InstanceSettingObj>> = Arc::new(RwLock::new(obj));

    INSTANCES.write().unwrap().insert(key, game.clone());
    // 名字先记进统计快照：就算这个实例马上又被删掉，统计窗口也还叫得出名字
    game_count::remember_name(&key, &name);

    // 登记到分组表；分组已独立存储，不存在的组会被自动创建
    game_group::register_instance(key, group);

    invoke_change(InstanceChange::AddInstance(key));
    game
}
