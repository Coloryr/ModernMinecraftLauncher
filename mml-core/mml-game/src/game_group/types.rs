//! 分组表的数据结构
//!
//! 分组条目、旧版分组的兼容结构与旧条目到实例 uuid 的映射。

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
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
pub(super) struct LegacyGroups {
    /// 组名 → 组内实例目录名
    #[serde(default)]
    pub(super) groups: BTreeMap<String, LegacyGroup>,
}

/// 旧版分组条目
#[derive(Debug, Deserialize)]
pub(super) struct LegacyGroup {
    /// 组内实例的目录名，按旧版排好的顺序
    #[serde(default)]
    pub(super) instances: Vec<String>,
}

/// 旧版分组的归并结果
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct LegacyImport {
    /// 新建的分组数；与已有分组同名时不算新建
    pub(super) groups: usize,
    /// 实际放进分组的实例数；认不出目录名的条目不算
    pub(super) instances: usize,
}

/// 旧版条目到实例 uuid 的映射
#[derive(Default)]
pub(super) struct LegacyLookup {
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
    pub(super) fn new(instances: &[LegacyInstance]) -> Self {
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
    pub(super) fn find(&self, entry: &str) -> Option<Uuid> {
        self.dirs
            .get(entry)
            .or_else(|| self.names.get(entry))
            .copied()
    }
}
