//! 分组表：`group_save.json`
//!
//! 分组以 **uuid 为身份**，组名只是可改的显示数据：
//! - 一个分组可以没有任何实例，空分组也合法 —— 归属信息若存在实例里，空分组就没有
//!   任何实例可以承载它，这正是之前"空分组留不住"的根因；
//! - 组内次序、分组自身的顺序也都由本表表达，不再散落在各实例的 `gui_setting.json` 里；
//! - 组名不进任何"身份"位置，如键与顺序表，改名不会牵连归属与顺序。
//!
//! 文件放**实例根目录**下、与各实例目录并列，下面用默认分组举例：
//! ```json
//! {
//!   "GroupOrder": [],
//!   "Names": { "00000000-0000-0000-0000-000000000000": "" },
//!   "Groups": { "00000000-0000-0000-0000-000000000000": ["8bb5fca2-...", "6a6af958-..."] },
//!   "Order": { "8bb5fca2-...": 0, "6a6af958-...": 1 }
//! }
//! ```
//! 四列各管一件事：
//! - `GroupOrder` — 分组 uuid 的显示顺序，**含默认分组**：它只是初始排首位，
//!   之后可以和别的分组一样换位置
//! - `Names` — 分组 uuid → 组名，**显示数据**；空白名字就是默认分组
//! - `Groups` — 分组 uuid → 组内实例，**只表达归属**，先后一律看 `Order`
//! - `Order` — 实例 uuid → 组内次序
//!
//! 实例配置 `game.json` 里**没有** `group` 字段；判断某个实例属于哪一组，
//! 靠在本表里查它的 uuid，见 [`group_of`]。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{LazyLock, RwLock},
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{
    i18_items::{error_type::CoreResult, info_type::InfoType},
    names, uuids,
};
use uuid::Uuid;

mod store;
mod types;

use self::store::GroupStore;
pub use self::types::{DEFAULT_GROUP_UUID, GroupInfoObj, ImportedGroup, LegacyInstance};
use self::types::{LegacyGroups, LegacyLookup};

/// 分组表：内存中的唯一数据源
static STORE: LazyLock<RwLock<GroupStore>> = LazyLock::new(|| RwLock::new(GroupStore::default()));

/// 分组文件路径，`init` 时确定
static GROUP_FILE: LazyLock<RwLock<Option<PathBuf>>> = LazyLock::new(|| RwLock::new(None));

/// 初始化分组存储，在实例目录确定后调用
///
/// - `dir`: 实例根目录，取自 `launcher_path::get_instance_dir()`
///
/// # 返回值
///
/// 读取失败返回对应错误；文件不存在时按空表开始，即首次运行
pub fn init(dir: PathBuf) -> CoreResult<()> {
    let file = dir.join(names::GROUP_FILE);
    *GROUP_FILE.write().unwrap() = Some(file.clone());

    // 读失败不致命：分组是可重建的辅助数据，宁可空着也不让实例加载整个失败
    let mut store: GroupStore = if file.exists() && file.is_file() {
        serialize_tools::json_from_file(&file).unwrap_or_default()
    } else {
        GroupStore::default()
    };
    store.normalize();

    *STORE.write().unwrap() = store;

    Ok(())
}

/// 把旧版分组文件 `instgroups.json` 并入分组表，并删掉旧文件
///
/// 旧版按"组名 → 实例目录名"存分组，本启动器改用 uuid 为身份的
/// [`names::GROUP_FILE`]。不迁移的话旧分组永远看不见，用户会以为分组丢了。
///
/// - `dir`: 实例根目录，旧文件与分组表都在这里
/// - `instances`: 当前加载到的实例，用来把旧文件里的目录名换成 uuid
pub fn import_legacy(dir: &Path, instances: &[LegacyInstance]) {
    let file = dir.join(names::LEGACY_GROUP_FILE);
    if !file.is_file() {
        return;
    }

    // 读不出来就留着旧文件下次再试：分组是辅助数据，不能挡住实例加载
    let Ok(legacy) = serialize_tools::json_from_file::<LegacyGroups>(&file) else {
        return;
    };
    let lookup = LegacyLookup::new(instances);

    // 先取路径再锁表：save 里是先读 GROUP_FILE 再读 STORE，反过来会死锁
    let path = GROUP_FILE.read().unwrap().clone();
    let Some(path) = path else {
        return;
    };

    let result = {
        let mut store = STORE.write().unwrap();
        let result = store.merge_legacy(&legacy, &lookup);
        if result.groups > 0 || result.instances > 0 {
            store.normalize();
            // 同步落盘：旧文件马上就要删，交给异步队列的话进程在写盘前退出就丢了
            if serialize_tools::json_to_file(&*store, &path).is_err() {
                return;
            }
        }
        result
    };

    // 删不掉就留着，下次启动会再并一次；成员已在本组时 merge_legacy 会跳过
    if std::fs::remove_file(&file).is_ok() && (result.groups > 0 || result.instances > 0) {
        mml_log::info_type(InfoType::ImportLegacyGroups {
            groups: result.groups,
            instances: result.instances,
        });
    }
}

/// 把迁移导入的分组归属合并进指定的分组表文件
///
/// 迁移发生在核心加载之后，导入的实例还没登记进内存表，所以这里只改表。
/// 若 `file` 正是本进程在用的那份表，直接在内存表上合并 —— 否则之后任何分组操作
/// 都会拿旧表把刚合并的组覆盖掉。导入的实例在下次启动按 uuid 对上归属。
///
/// - `file`: `group_save.json` 的路径
/// - `items`: 每条含实例 uuid 与组名；组名为 `None` 或空白表示默认分组，不写进表
///
/// # 返回值
///
/// 读写失败时返回对应错误；没有任何带名字的组时不写盘
pub fn merge_imported(file: &Path, items: &[ImportedGroup]) -> CoreResult<()> {
    let live = GROUP_FILE.read().unwrap().clone();

    if live.as_deref() == Some(file) {
        let merged = {
            let mut store = STORE.write().unwrap();
            let merged = merge_store(&mut store, items);
            if merged {
                store.normalize();
                serialize_tools::json_to_file(&*store, file)?;
            }
            merged
        };

        // 同步写盘之外再走一次异步保存：config_save 按 uuid 去重，
        // 这样队列里更早那份旧表落盘任务会被替换掉
        if merged {
            save();
        }
        return Ok(());
    }

    let mut store: GroupStore = if file.is_file() {
        serialize_tools::json_from_file(file).unwrap_or_default()
    } else {
        GroupStore::default()
    };
    store.normalize();

    if merge_store(&mut store, items) {
        store.normalize();
        serialize_tools::json_to_file(&store, file)?;
    }

    Ok(())
}

/// 把一批导入归属放进分组表：同名组复用，没有才新建
///
/// - `store`: 目标分组表
/// - `items`: 每条含实例 uuid 与组名；空白组名跳过
///
/// # 返回值
///
/// 至少落了一条归属时返回 `true`
fn merge_store(store: &mut GroupStore, items: &[ImportedGroup]) -> bool {
    let mut merged = false;
    for item in items {
        let Some(name) = item
            .group
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };

        // ColorMC 的组名就是 M²L 这边的组名：同名组直接复用，别建出两个一样的组
        let target = store.group_by_name(name).or_else(|| store.new_group(name));
        let Some(target) = target else {
            continue;
        };
        store.move_items(&[item.uuid], target);
        merged = true;
    }

    merged
}

/// 保存分组表到磁盘，异步落盘
pub fn save() {
    let file = GROUP_FILE.read().unwrap().clone();
    let Some(file) = file else {
        return;
    };

    // config_save 入队前就把对象序列化好了，见 `ConfigSaveObj::new`，
    // 所以直接交引用即可，不必再拷一份整表
    let store = STORE.read().unwrap();
    config_save::save(uuids::GROUP_UUID, &*store, &file);
}

/// 全部分组，含空分组
///
/// # 返回值
///
/// 返回分组条目列表；默认分组排在最前，其余按用户排定的顺序
pub fn group_list() -> Vec<GroupInfoObj> {
    let store = STORE.read().unwrap();
    store
        .keys()
        .into_iter()
        .map(|uuid| GroupInfoObj {
            uuid,
            name: store.name_of(uuid).unwrap_or_default(),
        })
        .collect()
}

/// 某个分组的显示名
///
/// - `uuid`: 分组 uuid
///
/// # 返回值
///
/// 返回分组名；分组不存在返回 `None`，默认分组的名字是空白
pub fn group_name(uuid: &Uuid) -> Option<String> {
    STORE.read().unwrap().name_of(*uuid)
}

/// 某分组内的实例 uuid，按组内次序
///
/// - `uuid`: 分组 uuid
///
/// # 返回值
///
/// 返回该组的 uuid 列表；分组不存在返回空列表
pub fn group_items(uuid: &Uuid) -> Vec<Uuid> {
    STORE.read().unwrap().members(*uuid)
}

/// 某个实例所属的分组 uuid
///
/// 在表里反查实例；查不到时回落到默认分组，实例刚建好还没登记就是这种情况
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 返回分组 uuid，可能是 [`DEFAULT_GROUP_UUID`]
pub fn group_of(uuid: &Uuid) -> Uuid {
    STORE.read().unwrap().group_of(uuid)
}

/// 某个实例在它所属分组里的显示次序，0 起
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 返回组内位置；该实例不在任何分组里时返回 `None`
pub fn index_of(uuid: &Uuid) -> Option<i32> {
    STORE.read().unwrap().position_of(uuid).map(|i| i as i32)
}

/// 新建分组
///
/// - `name`: 分组名，首尾空白会被去掉
///
/// # 返回值
///
/// 新建成功返回新分组的 uuid；已存在或名字为空返回 `None`
pub fn add_group(name: &str) -> Option<Uuid> {
    let mut store = STORE.write().unwrap();
    let uuid = store.new_group(name)?;
    drop(store);

    save();
    Some(uuid)
}

/// 删除分组，组内实例移入默认分组
///
/// 空分组也能删 —— 这正是把分组独立存的目的
///
/// - `uuid`: 分组 uuid
///
/// # 返回值
///
/// 删除成功返回被移入默认分组的实例 uuid；分组不存在或试图删除默认分组返回 `None`
pub fn remove_group(uuid: &Uuid) -> Option<Vec<Uuid>> {
    if *uuid == DEFAULT_GROUP_UUID {
        return None;
    }
    let key = uuid.to_string();

    let mut store = STORE.write().unwrap();
    store.names.remove(&key)?;
    let items = store.groups.remove(&key).unwrap_or_default();
    store.group_order.retain(|k| k != &key);
    // 组内实例移入默认分组，接在默认分组末尾，而不是凭空丢失归属
    store.move_items(&items, DEFAULT_GROUP_UUID);
    drop(store);

    save();
    Some(items)
}

/// 把一批实例移入某分组，追加到目标组末尾
///
/// - `list`: 要移动的实例 uuid
/// - `new`: 目标分组 uuid；`None` 表示默认分组
pub fn move_group(list: Vec<Uuid>, new: Option<Uuid>) {
    let target = new.unwrap_or(DEFAULT_GROUP_UUID);

    let mut store = STORE.write().unwrap();
    store.move_items(&list, target);
    drop(store);

    save();
}

/// 把一个实例放到目标分组的第 `index` 位
///
/// 跨组移动与组内排序在这里是**同一件事**：先确保目标分组存在，不存在则落到默认分组，
/// 再把它从原分组摘掉、插到目标下标。分两步调，即先 [`move_group`] 再排序，会落两次盘，
/// 调用方也容易漏掉后一半。
///
/// - `uuid`: 实例 uuid
/// - `new`: 目标分组 uuid；`None` 表示默认分组
/// - `index`: 目标下标；超出范围则排到末尾
pub fn place_in_group(uuid: &Uuid, new: Option<Uuid>, index: usize) {
    let target = new.unwrap_or(DEFAULT_GROUP_UUID);

    let mut store = STORE.write().unwrap();
    // 落组与定位必须连着做，且排序要针对**实际落到**的那个组：
    // move_items 在目标组不存在时会把实例放进默认分组
    let target = store.move_items(&[*uuid], target);
    store.reposition(target, uuid, index);
    drop(store);

    save();
}

/// 调整分组本身的顺序，按给定的完整列表重排
///
/// - `order`: 期望的分组 uuid 顺序；不在表里的忽略，表里没提到的组保持在后
pub fn reorder_groups(order: &[Uuid]) {
    let mut store = STORE.write().unwrap();
    store.reorder_groups(order);
    drop(store);

    save();
}

/// 删除实例后的清理：把它从所有分组里摘掉
///
/// 不删空分组 —— 空分组是合法状态，用户可以手工建一个空组
///
/// - `uuid`: 实例 uuid
pub(crate) fn forget_instance(uuid: &Uuid) {
    let mut store = STORE.write().unwrap();
    let changed = store.forget(uuid);
    drop(store);

    if changed {
        save();
    }
}

/// 登记一个新实例到某分组，创建实例后调用
///
/// **已经在表里的实例保持原分组** —— 加载流程会对每个实例都调一次，
/// 无条件移动会把用户分好的组全部冲回默认分组。
///
/// - `uuid`: 实例 uuid
/// - `group`: 目标分组 uuid；`None` 表示默认分组
pub fn register_instance(uuid: Uuid, group: Option<Uuid>) {
    let store = STORE.read().unwrap();
    if store.contains(&uuid) {
        return;
    }

    drop(store);
    move_group(vec![uuid], group);
}

/// 剔除表里指向已不存在实例的悬空项，归属与组内次序一起清
///
/// 实例可能在别处被删，如外部改动目录、别的进程，那时不会有删除事件；
/// 加载完成后调一次即可保持表与实际数据一致。
///
/// - `alive`: 当前实际存在的实例 uuid 集合
pub fn prune(alive: &[Uuid]) {
    let live: HashSet<Uuid> = alive.iter().copied().collect();

    let mut store = STORE.write().unwrap();

    let mut changed = false;
    for items in store.groups.values_mut() {
        let before = items.len();
        items.retain(|u| live.contains(u));
        changed |= items.len() != before;
    }

    let before = store.order.len();
    store
        .order
        .retain(|key, _| Uuid::parse_str(key).is_ok_and(|uuid| live.contains(&uuid)));
    changed |= store.order.len() != before;

    drop(store);

    if changed {
        save();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/game_group_tests.rs"]
mod tests;
