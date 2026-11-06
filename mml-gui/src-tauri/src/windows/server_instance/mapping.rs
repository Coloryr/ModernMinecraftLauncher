//! 服务器实例自定义编号映射
//!
//! 项目编号按 modid / 配置路径稳定绑定，文件号按内容 SHA1 更新；映射存在实例根目录，
//! 下次生成时用来判断哪些条目变了。

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use mml_base::{
    hash_helper::{HashType, gen_hash},
    serialize_tools,
};
use mml_names::names;
use serde::{Deserialize, Serialize};

use crate::err_keys;

/// 实例根目录下的编号映射
#[derive(Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct IdMap {
    /// 上次生成时映射文件是否存在；用于把新增条目也标成变化
    #[serde(skip)]
    loaded: bool,
    /// 结构版本，当前为 1
    version: u32,
    /// 逻辑键 → 编号条目
    entries: BTreeMap<String, IdEntry>,
    /// 上次生成时填写的名字 / 版本 / 说明 / 网址等
    meta: Option<Meta>,
    /// 上次生成时的模组快照，键是 modid / 路径
    mods: BTreeMap<String, EntrySnapshot>,
    /// 上次生成时的资源包快照，键是 pack:路径
    packs: BTreeMap<String, EntrySnapshot>,
    /// 本次读取时修过的重复 / 空项目编号，生成时标记为变化
    #[serde(skip)]
    repaired: BTreeSet<String>,
}

/// 上次生成时的一个文件
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct EntrySnapshot {
    /// 相对游戏目录的路径
    pub(super) path: String,
    /// 文件内容 SHA1
    pub(super) sha1: String,
    /// 生成时是否标成可选模组；只对模组有意义
    pub(super) optional: bool,
}

/// 上次生成时保存的选项
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Meta {
    /// 服务器实例名
    pub(super) name: String,
    /// 包版本
    pub(super) version: String,
    /// 说明
    pub(super) text: String,
    /// 发布地址
    pub(super) url: String,
    /// 格式：mml / hmcl
    pub(super) format: String,
    /// 保存文件夹
    pub(super) folder: String,
    /// 勾选的基础文件，相对实例根目录
    pub(super) base_files: Vec<String>,
    /// 上次生成用的配置目录；老数据为 `None`
    pub(super) configs: Option<Vec<MetaConfig>>,
}

/// 上次生成时保存的一个配置目录
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct MetaConfig {
    /// 目录名，相对游戏目录
    pub(super) dir: String,
    /// 同步方式：archive / files
    pub(super) mode: String,
    /// 压缩包同步时是否先删旧目录
    pub(super) delete_old: bool,
}

/// 一条编号记录
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct IdEntry {
    /// 自定义项目编号
    project_id: String,
    /// 自定义文件编号，随内容变化
    file_id: String,
    /// 上次生成时的相对路径
    path: String,
}

/// 已保存的编号快照，按文件路径给前端回填
pub(super) struct IdSnapshot {
    /// 相对游戏目录的路径
    pub(super) path: String,
    /// 项目编号
    pub(super) project_id: String,
    /// 文件编号
    pub(super) file_id: String,
}

/// 一次编号分配的结果
pub(super) struct IdAssignment {
    /// 项目编号
    pub(super) project_id: String,
    /// 文件编号
    pub(super) file_id: String,
    /// 与上次相比是否变化
    pub(super) changed: bool,
}

impl IdMap {
    /// 从实例根目录读取映射；没有或读不了时返回空表
    ///
    /// - `base`: 实例根目录
    ///
    /// # 返回值
    ///
    /// 编号映射，版本至少为 1
    pub(super) fn load(base: &Path) -> Self {
        let path = base.join(names::SERVER_INSTANCE_ID_FILE);
        let loaded = path.is_file();
        let mut map: Self = serialize_tools::json_from_file(&path).unwrap_or_default();
        map.loaded = loaded;
        if map.version == 0 {
            map.version = 1;
        }
        map.normalize();
        if !map.repaired.is_empty() {
            // 映射文件里已有重复编号时立刻修盘，避免下一次读取又看到重复
            let _ = map.save(base);
        }
        map
    }

    /// 修复历史映射里的重复 / 空项目编号
    ///
    /// 按逻辑键顺序保留第一个编号，后面的改用唯一生成编号；生成的编号会标记为变化。
    fn normalize(&mut self) {
        let keys: Vec<String> = self.entries.keys().cloned().collect();
        let mut reserved: BTreeSet<String> = self
            .entries
            .values()
            .map(|entry| project_id_key(&entry.project_id).to_string())
            .filter(|project_id| !project_id.is_empty())
            .collect();
        let mut used = BTreeSet::new();
        for key in keys {
            let literal = self
                .entries
                .get(&key)
                .map(|entry| entry.project_id.trim().to_string())
                .unwrap_or_default();
            let current = project_id_key(&literal).to_string();
            let project_id = if !current.is_empty() && used.insert(current.clone()) {
                reserved.remove(&current);
                literal.clone()
            } else {
                let generated = self.unique_generated(&key, &used, &reserved);
                used.insert(project_id_key(&generated).to_string());
                self.repaired.insert(key.clone());
                generated
            };
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.project_id = project_id;
            }
        }
    }

    /// 生成一个当前映射里没被占用的自定义项目编号
    ///
    /// - `key`: 稳定逻辑键
    /// - `used`: 已经确定要使用的编号
    /// - `reserved`: 还要留给后续条目的编号
    ///
    /// # 返回值
    ///
    /// 基础编号被占用时追加 `-2`、`-3` 等后缀
    fn unique_generated(
        &self,
        key: &str,
        used: &BTreeSet<String>,
        reserved: &BTreeSet<String>,
    ) -> String {
        let base = generated_project_id(key);
        if !used.contains(project_id_key(&base)) && !reserved.contains(project_id_key(&base)) {
            return base;
        }
        for index in 2u32.. {
            let candidate = format!("{base}-{index}");
            let candidate_key = project_id_key(&candidate);
            if !used.contains(candidate_key) && !reserved.contains(candidate_key) {
                return candidate;
            }
        }
        unreachable!("项目编号后缀不会耗尽")
    }

    /// 项目编号是否已被其他逻辑键占用
    ///
    /// - `project_id`: 待检查的项目编号
    /// - `key`: 当前逻辑键，自身不算冲突
    ///
    /// # 返回值
    ///
    /// 已被其他条目使用时返回 `true`
    fn project_id_taken(&self, project_id: &str, key: &str) -> bool {
        let wanted = project_id_key(project_id);
        self.entries.iter().any(|(other, entry)| {
            other != key && project_id_key(&entry.project_id) == wanted
        })
    }

    /// 取一个逻辑键的项目编号与内容文件编号，并更新映射
    ///
    /// - `key`: 稳定逻辑键，如 `mod:jei`、`config:config/foo.toml`
    /// - `wire`: 相对游戏目录的正斜杠路径
    /// - `sha1`: 当前内容的 SHA1；为空时按路径算一个占位值
    ///
    /// # 返回值
    ///
    /// 见 [`IdAssignment`]
    pub(super) fn assign(&mut self, key: &str, wire: &str, sha1: &str) -> IdAssignment {
        let file_id = content_file_id(sha1, wire);
        let existing = self.entries.get(key).cloned();
        let project_id = match &existing {
            Some(entry) if !entry.project_id.is_empty() => {
                let project_id = entry.project_id.clone();
                if self.project_id_taken(&project_id, key) {
                    self.repaired.insert(key.to_string());
                    let used = self
                        .entries
                        .values()
                        .map(|entry| project_id_key(&entry.project_id).to_string())
                        .filter(|project_id| !project_id.is_empty())
                        .collect();
                    self.unique_generated(key, &used, &BTreeSet::new())
                } else {
                    project_id
                }
            }
            _ => {
                let used = self
                    .entries
                    .values()
                    .map(|entry| project_id_key(&entry.project_id).to_string())
                    .filter(|project_id| !project_id.is_empty())
                    .collect();
                self.unique_generated(key, &used, &BTreeSet::new())
            }
        };
        // 同路径换成了另一个逻辑键也算变化，例如同文件名换了 modid
        let reused_path = self
            .entries
            .iter()
            .any(|(other, entry)| other != key && entry.path == wire);
        let changed = self.repaired.contains(key)
            || existing.as_ref().is_some_and(|entry| {
                entry.project_id.is_empty() || entry.file_id != file_id || entry.path != wire
            })
            || (existing.is_none() && (self.loaded || reused_path));

        self.entries.insert(
            key.to_string(),
            IdEntry {
                project_id: project_id.clone(),
                file_id: file_id.clone(),
                path: wire.to_string(),
            },
        );
        IdAssignment {
            project_id,
            file_id,
            changed,
        }
    }

    /// 保存用户改过的自定义编号
    ///
    /// 已生成过的条目项目编号锁定，忽略传入的项目编号；新增条目才允许采用手填值。
    ///
    /// - `key`: 稳定逻辑键
    /// - `wire`: 相对游戏目录的正斜杠路径
    /// - `sha1`: 当前内容 SHA1
    /// - `pid`: 用户手填的项目编号
    /// - `fid`: 用户手填的文件编号
    ///
    /// # 返回值
    ///
    /// 见 [`IdAssignment`]
    pub(super) fn save_edited(
        &mut self,
        key: &str,
        wire: &str,
        sha1: &str,
        pid: &str,
        fid: &str,
    ) -> Result<IdAssignment, String> {
        let existing_key = if self.entries.contains_key(key) {
            Some(key.to_string())
        } else {
            self.entries
                .iter()
                .find(|(_, entry)| entry.path == wire)
                .map(|(other, _)| other.clone())
        };
        let existing = existing_key
            .as_ref()
            .and_then(|other| self.entries.get(other))
            .cloned();
        let pid = pid.trim();
        let project_id = match &existing {
            Some(entry) if !entry.project_id.trim().is_empty() => {
                let project_id = entry.project_id.trim().to_string();
                let taken = self.entries.iter().any(|(other, entry)| {
                    Some(other.as_str()) != existing_key.as_deref()
                        && other != key
                        && project_id_key(&entry.project_id) == project_id_key(&project_id)
                });
                if taken {
                    self.repaired.insert(key.to_string());
                    let used = self
                        .entries
                        .values()
                        .map(|entry| project_id_key(&entry.project_id).to_string())
                        .filter(|project_id| !project_id.is_empty())
                        .collect();
                    self.unique_generated(key, &used, &BTreeSet::new())
                } else {
                    project_id
                }
            }
            _ if !project_id_key(pid).is_empty() => {
                if self.project_id_taken(pid, key) {
                    return Err(err_keys::SERVER_INSTANCE_PROJECT_ID_DUPLICATE.to_string());
                }
                pid.to_string()
            }
            _ => {
                let used = self
                    .entries
                    .values()
                    .map(|entry| project_id_key(&entry.project_id).to_string())
                    .filter(|project_id| !project_id.is_empty())
                    .collect();
                self.unique_generated(key, &used, &BTreeSet::new())
            }
        };
        let fid = fid.trim();
        let file_id = if fid.is_empty() {
            content_file_id(sha1, wire)
        } else {
            fid.to_string()
        };
        let changed = self.repaired.contains(key)
            || existing_key.as_deref() != Some(key)
            || existing
                .as_ref()
                .is_some_and(|entry| entry.file_id != file_id || entry.path != wire);

        if let Some(other) = existing_key {
            if other != key {
                self.entries.remove(&other);
            }
        }
        self.entries.insert(
            key.to_string(),
            IdEntry {
                project_id: project_id.clone(),
                file_id: file_id.clone(),
                path: wire.to_string(),
            },
        );
        Ok(IdAssignment {
            project_id,
            file_id,
            changed,
        })
    }

    /// 保存这次生成填写的选项
    ///
    /// - `meta`: 名字 / 版本 / 说明 / 网址等
    pub(super) fn set_meta(&mut self, meta: Meta) {
        self.meta = Some(meta);
    }

    /// 取上次生成保存的选项
    ///
    /// # 返回值
    ///
    /// 没有生成过时返回 `None`
    pub(super) fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }

    /// 映射文件本次是否是从磁盘读到的
    ///
    /// # 返回值
    ///
    /// 文件存在并已读取时返回 `true`
    pub(super) fn loaded(&self) -> bool {
        self.loaded
    }

    /// 取上次生成时的模组快照
    ///
    /// # 返回值
    ///
    /// 逻辑键 → 路径与 SHA1
    pub(super) fn mods(&self) -> &BTreeMap<String, EntrySnapshot> {
        &self.mods
    }

    /// 更新本次生成后的模组快照
    ///
    /// - `mods`: 本次实际写进清单的模组，键是 modid / 路径
    pub(super) fn set_mods(&mut self, mods: BTreeMap<String, EntrySnapshot>) {
        self.mods = mods;
    }

    /// 取上次生成时的资源包快照
    ///
    /// # 返回值
    ///
    /// 逻辑键 → 路径与 SHA1
    pub(super) fn packs(&self) -> &BTreeMap<String, EntrySnapshot> {
        &self.packs
    }

    /// 更新本次生成后的资源包快照
    ///
    /// - `packs`: 本次实际写进清单的资源包，键是 pack:路径
    pub(super) fn set_packs(&mut self, packs: BTreeMap<String, EntrySnapshot>) {
        self.packs = packs;
    }

    /// 按路径导出编号，给前端表格回填
    ///
    /// # 返回值
    ///
    /// 每个已保存条目的路径与项目 / 文件编号
    pub(super) fn snapshot(&self) -> Vec<IdSnapshot> {
        self.entries
            .values()
            .map(|entry| IdSnapshot {
                path: entry.path.clone(),
                project_id: entry.project_id.clone(),
                file_id: entry.file_id.clone(),
            })
            .collect()
    }

    /// 写回实例根目录
    ///
    /// - `base`: 实例根目录
    ///
    /// # 返回值
    ///
    /// 写盘成功返回 `Ok(())`，否则返回错误文案
    pub(super) fn save(&self, base: &Path) -> Result<(), String> {
        serialize_tools::json_to_file(self, base.join(names::SERVER_INSTANCE_ID_FILE))
            .map_err(|error| error.to_string())
    }
}

/// 项目编号比较用的键：忽略显示层去掉的 custom- 前缀
///
/// - `project_id`: 项目编号
///
/// # 返回值
///
/// 去掉开头 `custom-` 后的编号
fn project_id_key(project_id: &str) -> &str {
    project_id
        .trim()
        .strip_prefix("custom-")
        .unwrap_or(project_id.trim())
}

/// 内容对应的自定义文件编号
///
/// - `sha1`: 内容 SHA1；为空时按路径兜底
/// - `wire`: 相对游戏目录的路径
///
/// # 返回值
///
/// 形如 `custom-f-...`
fn content_file_id(sha1: &str, wire: &str) -> String {
    let content = if sha1.is_empty() {
        gen_hash(HashType::Sha1, wire.as_bytes())
    } else {
        sha1.to_string()
    };
    format!("custom-f-{}", &content[..content.len().min(16)])
}

/// 按逻辑键生成稳定的项目编号
///
/// - `key`: 稳定逻辑键
///
/// # 返回值
///
/// 形如 `custom-p-...`
fn generated_project_id(key: &str) -> String {
    let hash = gen_hash(HashType::Sha1, key.as_bytes());
    format!("custom-p-{}", &hash[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_stays_and_file_id_changes() {
        let mut map = IdMap::default();
        map.version = 1;

        let first = map.assign("mod:jei", "mods/jei-1.jar", "aaa");
        assert!(!first.changed);
        let same = map.assign("mod:jei", "mods/jei-1.jar", "aaa");
        assert!(!same.changed);
        assert_eq!(first.project_id, same.project_id);
        assert_eq!(first.file_id, same.file_id);

        let changed = map.assign("mod:jei", "mods/jei-2.jar", "bbb");
        assert!(changed.changed);
        assert_eq!(first.project_id, changed.project_id);
        assert_ne!(first.file_id, changed.file_id);
    }

    #[test]
    fn edited_project_id_is_locked_after_first_save() {
        let mut map = IdMap::default();
        map.version = 1;

        let first = map
            .save_edited("mod:jei", "mods/jei.jar", "aaa", "my-project", "file-1")
            .unwrap();
        assert_eq!(first.project_id, "my-project");

        let second = map
            .save_edited("mod:jei", "mods/jei.jar", "bbb", "other-project", "file-2")
            .unwrap();
        assert_eq!(second.project_id, "my-project", "已生成的项目编号不能被改");
        assert_eq!(second.file_id, "file-2", "文件编号仍可改");

        let new = map
            .save_edited("mod:create", "mods/create.jar", "ccc", "create-project", "")
            .unwrap();
        assert_eq!(new.project_id, "create-project", "新增项目可以采用手填项目编号");

        let duplicate = map.save_edited(
            "mod:duplicate",
            "mods/duplicate.jar",
            "ddd",
            "my-project",
            "",
        );
        assert!(duplicate.is_err(), "不同条目不能共用一个项目编号");

        map.save_edited(
            "mod:prefixed",
            "mods/prefixed.jar",
            "eee",
            "custom-shared",
            "",
        )
        .unwrap();
        let duplicate_after_prefix = map.save_edited(
            "mod:plain",
            "mods/plain.jar",
            "fff",
            "shared",
            "",
        );
        assert!(
            duplicate_after_prefix.is_err(),
            "去掉 custom- 后相同也算重复"
        );
    }

    #[test]
    fn duplicate_project_ids_are_repaired_on_load_and_generated_unique() {
        let mut map = IdMap::default();
        map.version = 1;
        map.entries.insert(
            "mod:a".to_string(),
            IdEntry {
                project_id: "same".to_string(),
                file_id: "f1".to_string(),
                path: "mods/a.jar".to_string(),
            },
        );
        map.entries.insert(
            "mod:b".to_string(),
            IdEntry {
                project_id: "same".to_string(),
                file_id: "f2".to_string(),
                path: "mods/b.jar".to_string(),
            },
        );
        map.normalize();
        let ids: Vec<&str> = map
            .entries
            .values()
            .map(|entry| entry.project_id.as_str())
            .collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert!(map.repaired.contains("mod:b"));

        // 新条目生成时也必须避开已有编号
        let generated = generated_project_id("mod:new");
        map.entries.insert(
            "mod:occupied".to_string(),
            IdEntry {
                project_id: generated.clone(),
                file_id: "f1".to_string(),
                path: "mods/occupied.jar".to_string(),
            },
        );
        let assigned = map.assign("mod:new", "mods/new.jar", "f1");
        assert_ne!(assigned.project_id, generated);
    }
}
