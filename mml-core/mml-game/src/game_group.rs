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
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{LazyLock, RwLock},
};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_names::{
    i18_items::{error_type::CoreResult, info_type::InfoType},
    names, uuids,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 默认分组的固定 uuid，**全 0**
///
/// 全 0 就是"没有分组"本身：对外 `Option<Uuid>` 的 `None` 即指向它，
/// 重启后也必须还是同一个身份。
///
/// **不放进 `mml_names::uuids`**：那张表登记的是配置文件保存任务的 id，
/// 这里是数据身份；混在一起会让"uuid 有没有重复"的检查失去意义。
pub const DEFAULT_GROUP_UUID: Uuid = Uuid::nil();

/// 分组条目：uuid 加显示名
pub struct GroupInfoObj {
    /// 分组 uuid
    pub uuid: Uuid,
    /// 分组显示名；默认分组是空白，由界面翻译成"默认分组"
    pub name: String,
}

/// 从别处导入的一个实例的分组归属
///
/// 迁移时实例目录刚搬过来、核心还没登记，只能先把归属写进分组表；
/// 重启加载实例时按 uuid 对上，实例就落在这一组里。
pub struct ImportedGroup {
    /// 导入的实例 uuid
    pub uuid: Uuid,
    /// 原来的组名，调用方已去首尾空白；`None` 或空白表示默认分组
    pub group: Option<String>,
}

/// 旧版分组文件里的一个实例条目
///
/// 旧文件只记实例目录名，迁进分组表要先换成 uuid，所以由调用方补上 uuid。
pub struct LegacyInstance {
    /// 实例目录名，即 `game.json` 的 `DirName`
    pub dir: String,
    /// 实例显示名，目录名对不上时用它兜底
    pub name: String,
    /// 实例 uuid
    pub uuid: Uuid,
}

/// 旧版分组文件的内容
///
/// 只取 `groups`：`formatVersion` 与每组的 `hidden` 在新表里没有对应字段，
/// 反序列化时忽略。
#[derive(Debug, Deserialize)]
struct LegacyGroups {
    /// 组名 → 组内实例目录名
    #[serde(default)]
    groups: BTreeMap<String, LegacyGroup>,
}

/// 旧版分组条目
#[derive(Debug, Deserialize)]
struct LegacyGroup {
    /// 组内实例的目录名，按旧版排好的顺序
    #[serde(default)]
    instances: Vec<String>,
}

/// 旧版分组的归并结果
#[derive(Debug, Default, Clone, Copy)]
struct LegacyImport {
    /// 新建的分组数；与已有分组同名时不算新建
    groups: usize,
    /// 实际放进分组的实例数；认不出目录名的条目不算
    instances: usize,
}

/// 旧版条目到实例 uuid 的映射
#[derive(Default)]
struct LegacyLookup {
    /// 目录名 → 实例 uuid
    dirs: HashMap<String, Uuid>,
    /// 实例显示名 → 实例 uuid，重名时保留第一个
    names: HashMap<String, Uuid>,
}

impl LegacyLookup {
    /// 按当前加载到的实例建映射
    ///
    /// - `instances`: 实例列表
    ///
    /// # 返回值
    ///
    /// 见 [`LegacyLookup`]
    fn new(instances: &[LegacyInstance]) -> Self {
        let mut lookup = Self::default();
        for item in instances {
            lookup.dirs.entry(item.dir.clone()).or_insert(item.uuid);
            lookup.names.entry(item.name.clone()).or_insert(item.uuid);
        }
        lookup
    }

    /// 旧文件里的一项对应哪个实例
    ///
    /// - `entry`: 旧文件记的实例目录名，可能是显示名
    ///
    /// # 返回值
    ///
    /// 对得上返回实例 uuid；实例已删时返回 `None`
    fn find(&self, entry: &str) -> Option<Uuid> {
        self.dirs
            .get(entry)
            .or_else(|| self.names.get(entry))
            .copied()
    }
}

/// 分组表：内存与磁盘共用同一个结构
///
/// 单独再定义一个"落盘用"的镜像结构没有意义 —— 字段完全一样，只会多一份要同步的东西，
/// 所以 `Default` 的空表与 `Serialize` / `Deserialize` 的文件形态都落在本类型上。
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GroupStore {
    /// 分组的显示顺序，元素是分组 uuid，**含默认分组**
    ///
    /// 默认分组只是**初始排在首位**，见 `normalize`：顺序表里没有它时补在最前，
    /// 之后用户可以把它拖到任意位置 —— 它的特殊之处是"未分组"这个身份
    /// 即固定 uuid 加空白名字，不是"位置固定"。
    #[serde(default)]
    group_order: Vec<String>,
    /// 分组 uuid → 组名
    #[serde(default)]
    names: HashMap<String, String>,
    /// 分组 uuid → 组内实例，只表达归属，先后看 `order`
    #[serde(default)]
    groups: HashMap<String, Vec<Uuid>>,
    /// 实例 uuid → 组内次序
    #[serde(default)]
    order: HashMap<String, i32>,
}

impl GroupStore {
    /// 归一化：补默认分组、清悬空项、把每个组的组内次序压实成 `0..n-1`
    ///
    /// 读盘后、以及手改过文件后，必须跑一次：文件里的次序可能缺项、并列、是负数，
    /// 也可能指向早就删掉的实例。压实之后"次序"才是一个确定的全序。
    fn normalize(&mut self) {
        let default = DEFAULT_GROUP_UUID.to_string();

        // 默认分组恒存在：身份是固定 uuid，名字留白，界面自行翻译
        self.names.entry(default.clone()).or_default();
        self.groups.entry(default.clone()).or_default();

        // 没有名字的分组视为损坏：只有 add_group 会建组，一定带名字
        let named: HashSet<String> = self.names.keys().cloned().collect();
        self.groups.retain(|key, _| named.contains(key));
        // 反向也要补齐：手改过文件时会出现有名字但少了成员数组的组，补一个空数组。
        // 缺了它，实例会被从原组摘下来却推不进目标组 —— 直接从界面上消失
        let keys: Vec<String> = self.names.keys().cloned().collect();
        for key in keys {
            self.groups.entry(key).or_default();
        }

        // 分组顺序：默认分组**初始排首位**，顺序表里没有它时补在最前，已有则尊重用户排的位置；
        // 不存在的名字丢掉，存在但漏掉的补在末尾
        let mut next: Vec<String> = Vec::with_capacity(self.names.len() + 1);
        let mut has_default = false;
        for key in &self.group_order {
            if !self.names.contains_key(key) || next.contains(key) {
                continue;
            }
            if key == &default {
                has_default = true;
            }
            next.push(key.clone());
        }
        if !has_default {
            // 老文件的默认分组不在顺序表里：补到最前 —— 这正是"默认初始位置"，
            // 用户之后拖到哪儿就存哪儿，下次读出来 has_default 为真、不再挪
            next.insert(0, default.clone());
        }
        for key in self.names.keys() {
            if !next.contains(key) {
                next.push(key.clone());
            }
        }
        self.group_order = next;

        // 一个实例只能属于一个组：按分组顺序保留第一次出现的那个
        let order: Vec<String> = self.group_order.clone();
        let mut seen: HashSet<Uuid> = HashSet::new();
        for key in std::iter::once(default.clone()).chain(order) {
            if let Some(items) = self.groups.get_mut(&key) {
                items.retain(|uuid| seen.insert(*uuid));
            }
        }

        // 每个组按 (次序, uuid) 排一遍再重新编号：次序并列/缺失的旧数据也能排出确定顺序
        let groups = std::mem::take(&mut self.groups);
        let mut fixed: HashMap<String, Vec<Uuid>> = HashMap::with_capacity(groups.len());
        for (key, mut items) in groups {
            items.sort_by_key(|uuid| (self.rank(*uuid), uuid.to_string()));
            for (i, uuid) in items.iter().enumerate() {
                self.order.insert(uuid.to_string(), i as i32);
            }
            fixed.insert(key, items);
        }
        self.groups = fixed;

        // 悬空次序，即实例已不在任何组里的那些，清掉
        let alive: HashSet<String> = self
            .groups
            .values()
            .flatten()
            .map(|uuid| uuid.to_string())
            .collect();
        self.order.retain(|key, _| alive.contains(key));
    }

    /// 实例的组内次序；表里缺这一项时排到最后
    fn rank(&self, uuid: Uuid) -> i32 {
        self.order
            .get(&uuid.to_string())
            .copied()
            .unwrap_or(i32::MAX)
    }

    /// 全部分组 uuid，顺序同 `group_order`，默认分组只是初始排在首位
    fn keys(&self) -> Vec<Uuid> {
        let mut list = Vec::with_capacity(self.group_order.len() + 1);
        for key in &self.group_order {
            if let Ok(uuid) = Uuid::parse_str(key)
                && !list.contains(&uuid)
            {
                list.push(uuid);
            }
        }
        // 兜底一：默认分组必须始终可见，老文件与手改过的文件可能没记它
        if !list.contains(&DEFAULT_GROUP_UUID) {
            list.insert(0, DEFAULT_GROUP_UUID);
        }
        // 兜底二：表里有、顺序表里没有的分组也得能被看到。
        // 正常路径，即 init 的 normalize 与 new_group，不会出现这种状态，这里只是别让它悄悄消失
        for key in self.names.keys() {
            if let Ok(uuid) = Uuid::parse_str(key)
                && !list.contains(&uuid)
            {
                list.push(uuid);
            }
        }
        list
    }

    /// 分组名；默认分组是空白
    fn name_of(&self, uuid: Uuid) -> Option<String> {
        self.names.get(&uuid.to_string()).cloned()
    }

    /// 是否存在这个分组
    fn exists(&self, uuid: Uuid) -> bool {
        self.names.contains_key(&uuid.to_string())
    }

    /// 是否已经有同名的分组，组名保持唯一，免得界面上分不清
    fn has_name(&self, name: &str) -> bool {
        self.names.values().any(|n| n == name)
    }

    /// 按组名找分组
    ///
    /// - `name`: 分组名，调用方已去掉首尾空白
    ///
    /// # 返回值
    ///
    /// 找到返回分组 uuid；组名不存在返回 `None`
    fn group_by_name(&self, name: &str) -> Option<Uuid> {
        self.names
            .iter()
            .find(|(_, value)| value.as_str() == name)
            .and_then(|(key, _)| Uuid::parse_str(key).ok())
    }

    /// 把旧版分组并进来：组名已存在就复用，成员按旧文件顺序追加到组末尾
    ///
    /// - `legacy`: 旧版分组文件
    /// - `lookup`: 旧版实例名到 uuid 的映射
    ///
    /// # 返回值
    ///
    /// 见 [`LegacyImport`]，调用方据此决定要不要落盘
    fn merge_legacy(&mut self, legacy: &LegacyGroups, lookup: &LegacyLookup) -> LegacyImport {
        let mut result = LegacyImport::default();

        for (name, group) in &legacy.groups {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let target = match self.group_by_name(name) {
                Some(uuid) => uuid,
                None => match self.new_group(name) {
                    Some(uuid) => {
                        result.groups += 1;
                        uuid
                    }
                    None => continue,
                },
            };

            for entry in &group.instances {
                let Some(uuid) = lookup.find(entry) else {
                    continue;
                };
                // 已经在这一组里就不再动：旧文件删不掉时下次启动会再并一次，
                // 重复追加会把组内顺序越拖越乱
                if self.group_of(&uuid) == target {
                    continue;
                }
                self.move_items(&[uuid], target);
                result.instances += 1;
            }
        }

        result
    }

    /// 某分组的成员，按组内次序排好
    fn members(&self, uuid: Uuid) -> Vec<Uuid> {
        let mut items = self
            .groups
            .get(&uuid.to_string())
            .cloned()
            .unwrap_or_default();
        items.sort_by_key(|instance| (self.rank(*instance), instance.to_string()));
        items
    }

    /// 某个实例所属的分组 uuid；不在任何组里时回落到默认分组
    fn group_of(&self, uuid: &Uuid) -> Uuid {
        for key in self.keys() {
            if self
                .groups
                .get(&key.to_string())
                .is_some_and(|items| items.contains(uuid))
            {
                return key;
            }
        }
        DEFAULT_GROUP_UUID
    }

    /// 实例在它所属分组里的位置，0 起：由次序编号算出来，不是直接读编号
    ///
    /// 直接读编号是不行的 —— 文件被手改过后编号可能并列、跳号或为负。
    fn position_of(&self, uuid: &Uuid) -> Option<usize> {
        let group = self.group_of(uuid);
        self.members(group).iter().position(|u| u == uuid)
    }

    /// 是否有任何分组已经收了这个实例
    fn contains(&self, uuid: &Uuid) -> bool {
        self.groups.values().any(|items| items.contains(uuid))
    }

    /// 新建分组，返回新分组的 uuid；名字为空或重名返回 `None`
    fn new_group(&mut self, name: &str) -> Option<Uuid> {
        let name = name.trim();
        if name.is_empty() || self.has_name(name) {
            return None;
        }

        let uuid = Uuid::new_v4();
        let key = uuid.to_string();
        self.names.insert(key.clone(), name.to_string());
        self.groups.insert(key.clone(), Vec::new());
        self.group_order.push(key);

        Some(uuid)
    }

    /// 把一批实例移入目标分组：从所有旧分组里摘掉，按次序追加到目标组末尾
    ///
    /// 目标分组不存在时落到默认分组，界面上拿到过期数据、表还没 init 都属于这种情况，
    /// 不凭空造一个没名字的组。
    ///
    /// # 返回值
    ///
    /// 返回实际落到的分组 uuid；调用方接着排序时要用它，别拿原来那个
    fn move_items(&mut self, list: &[Uuid], target: Uuid) -> Uuid {
        let target = if self.exists(target) {
            target
        } else {
            DEFAULT_GROUP_UUID
        };
        let key = target.to_string();

        // 落点必须真的存在，名字与成员数组都要有：只摘不放会让实例从界面上消失。
        // 补出来的名字是空串，正是默认分组"留白"的那个口径
        self.names.entry(key.clone()).or_default();
        self.groups.entry(key.clone()).or_default();

        // 追加位置：接在目标组现有最大次序之后
        let mut next = self
            .groups
            .get(&key)
            .and_then(|items| items.iter().map(|uuid| self.rank(*uuid)).max())
            .map_or(0, |max| max.saturating_add(1));

        for uuid in list {
            for items in self.groups.values_mut() {
                items.retain(|u| u != uuid);
            }
            if let Some(items) = self.groups.get_mut(&key) {
                items.push(*uuid);
            }
            self.order.insert(uuid.to_string(), next);
            next = next.saturating_add(1);
        }

        target
    }

    /// 把已在组内的实例挪到第 `index` 位，整组重新编号 `0..n-1`
    ///
    /// 重新编号而不是只改这一个值：旧数据里次序全是默认 0、互相并列，只改一个值排不出顺序。
    fn reposition(&mut self, group: Uuid, uuid: &Uuid, index: usize) {
        // 组不存在就什么都不做 —— 别在这里 insert 出一个没有名字的组
        if !self.exists(group) {
            return;
        }

        let mut items = self.members(group);
        items.retain(|u| u != uuid);
        let at = index.min(items.len());
        items.insert(at, *uuid);

        for (i, u) in items.iter().enumerate() {
            self.order.insert(u.to_string(), i as i32);
        }
        self.groups.insert(group.to_string(), items);
    }

    /// 按给定顺序重排分组
    ///
    /// 默认分组**不特殊对待**：它只是初始排在首位，之后可以和别的分组一样换位置。
    /// 传进来的 uuid 里不存在的忽略；表里存在但没提到的保持原相对顺序、排在后面。
    fn reorder_groups(&mut self, order: &[Uuid]) {
        let mut next: Vec<String> = Vec::with_capacity(self.group_order.len());
        let default = DEFAULT_GROUP_UUID.to_string();

        for uuid in order {
            let key = uuid.to_string();
            // 默认分组恒是合法成员：names 里一定有它，但别依赖这一点，直接放行，
            // 否则一个没跑过 normalize 的 store 会把它悄悄丢掉
            if (self.names.contains_key(&key) || key == default) && !next.contains(&key) {
                next.push(key);
            }
        }
        for key in &self.group_order {
            if !next.contains(key) {
                next.push(key.clone());
            }
        }

        self.group_order = next;
    }

    /// 把某个实例从所有分组里摘掉，连同它的组内次序
    ///
    /// # 返回值
    ///
    /// 有改动返回 `true`，调用方据此决定要不要落盘
    fn forget(&mut self, uuid: &Uuid) -> bool {
        self.order.remove(&uuid.to_string());

        let mut changed = false;
        for items in self.groups.values_mut() {
            let before = items.len();
            items.retain(|u| u != uuid);
            changed |= items.len() != before;
        }
        changed
    }
}

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
    {
        let store = STORE.read().unwrap();
        if store.contains(&uuid) {
            return;
        }
    }
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
#[path = "../tests/unit/game_group_tests.rs"]
mod tests;
