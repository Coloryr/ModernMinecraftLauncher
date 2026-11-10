//! 分组表
//!
//! 内存与磁盘共用一个结构：分组顺序、组名、组内实例与组内次序。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{DEFAULT_GROUP_UUID, LegacyGroups, LegacyImport, LegacyLookup};

/// 分组表：内存与磁盘共用同一个结构
///
/// 单独再定义一个"落盘用"的镜像结构没有意义 —— 字段完全一样，只会多一份要同步的东西，
/// 所以 `Default` 的空表与 `Serialize` / `Deserialize` 的文件形态都落在本类型上。
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct GroupStore {
    /// 分组的显示顺序，元素是分组 uuid，**含默认分组**
    ///
    /// 默认分组只是**初始排在首位**，见 `normalize`：顺序表里没有它时补在最前，
    /// 之后用户可以把它拖到任意位置 —— 它的特殊之处是"未分组"这个身份
    /// 即固定 uuid 加空白名字，不是"位置固定"。
    #[serde(default)]
    pub(super) group_order: Vec<String>,
    /// 分组 uuid → 组名
    #[serde(default)]
    pub(super) names: HashMap<String, String>,
    /// 分组 uuid → 组内实例，只表达归属，先后看 `order`
    #[serde(default)]
    pub(super) groups: HashMap<String, Vec<Uuid>>,
    /// 实例 uuid → 组内次序
    #[serde(default)]
    pub(super) order: HashMap<String, i32>,
}

impl GroupStore {
    /// 归一化：补默认分组、清悬空项、把每个组的组内次序压实成 `0..n-1`
    ///
    /// 读盘后、以及手改过文件后，必须跑一次：文件里的次序可能缺项、并列、是负数，
    /// 也可能指向早就删掉的实例。压实之后"次序"才是一个确定的全序。
    pub(super) fn normalize(&mut self) {
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
    pub(super) fn rank(&self, uuid: Uuid) -> i32 {
        self.order
            .get(&uuid.to_string())
            .copied()
            .unwrap_or(i32::MAX)
    }

    /// 全部分组 uuid，顺序同 `group_order`，默认分组只是初始排在首位
    pub(super) fn keys(&self) -> Vec<Uuid> {
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
    pub(super) fn name_of(&self, uuid: Uuid) -> Option<String> {
        self.names.get(&uuid.to_string()).cloned()
    }

    /// 是否存在这个分组
    pub(super) fn exists(&self, uuid: Uuid) -> bool {
        self.names.contains_key(&uuid.to_string())
    }

    /// 是否已经有同名的分组，组名保持唯一，免得界面上分不清
    pub(super) fn has_name(&self, name: &str) -> bool {
        self.names.values().any(|n| n == name)
    }

    /// 按组名找分组
    ///
    /// - `name`: 分组名，调用方已去掉首尾空白
    ///
    /// # 返回值
    ///
    /// 找到返回分组 uuid；组名不存在返回 `None`
    pub(super) fn group_by_name(&self, name: &str) -> Option<Uuid> {
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
    pub(super) fn merge_legacy(
        &mut self,
        legacy: &LegacyGroups,
        lookup: &LegacyLookup,
    ) -> LegacyImport {
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
    pub(super) fn members(&self, uuid: Uuid) -> Vec<Uuid> {
        let mut items = self
            .groups
            .get(&uuid.to_string())
            .cloned()
            .unwrap_or_default();
        items.sort_by_key(|instance| (self.rank(*instance), instance.to_string()));
        items
    }

    /// 某个实例所属的分组 uuid；不在任何组里时回落到默认分组
    pub(super) fn group_of(&self, uuid: &Uuid) -> Uuid {
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
    pub(super) fn position_of(&self, uuid: &Uuid) -> Option<usize> {
        let group = self.group_of(uuid);
        self.members(group).iter().position(|u| u == uuid)
    }

    /// 是否有任何分组已经收了这个实例
    pub(super) fn contains(&self, uuid: &Uuid) -> bool {
        self.groups.values().any(|items| items.contains(uuid))
    }

    /// 新建分组，返回新分组的 uuid；名字为空或重名返回 `None`
    pub(super) fn new_group(&mut self, name: &str) -> Option<Uuid> {
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
    pub(super) fn move_items(&mut self, list: &[Uuid], target: Uuid) -> Uuid {
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
    pub(super) fn reposition(&mut self, group: Uuid, uuid: &Uuid, index: usize) {
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
    pub(super) fn reorder_groups(&mut self, order: &[Uuid]) {
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
    pub(super) fn forget(&mut self, uuid: &Uuid) -> bool {
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
