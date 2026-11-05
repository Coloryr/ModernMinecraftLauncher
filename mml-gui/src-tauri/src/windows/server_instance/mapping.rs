//! 服务器实例自定义编号映射
//!
//! 项目编号按 modid / 配置路径稳定绑定，文件号按内容 SHA1 更新；映射存在实例根目录，
//! 下次生成时用来判断哪些条目变了。

use std::{collections::BTreeMap, path::Path};

use mml_base::{
    hash_helper::{HashType, gen_hash},
    serialize_tools,
};
use mml_names::names;
use serde::{Deserialize, Serialize};

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
        map
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
        let existing = self.entries.get(key);
        let project_id = existing
            .map(|entry| entry.project_id.clone())
            .unwrap_or_else(|| generated_project_id(key));
        // 同路径换成了另一个逻辑键也算变化，例如同文件名换了 modid
        let reused_path = self
            .entries
            .iter()
            .any(|(other, entry)| other != key && entry.path == wire);
        let changed = existing
            .is_some_and(|entry| entry.file_id != file_id || entry.path != wire)
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
    ) -> IdAssignment {
        let existing = self.entries.get(key).cloned().or_else(|| {
            self.entries
                .values()
                .find(|entry| entry.path == wire)
                .cloned()
        });
        let pid = pid.trim();
        let project_id = existing
            .as_ref()
            .map(|entry| entry.project_id.clone())
            .unwrap_or_else(|| {
                if pid.is_empty() {
                    generated_project_id(key)
                } else {
                    pid.to_string()
                }
            });
        let fid = fid.trim();
        let file_id = if fid.is_empty() {
            content_file_id(sha1, wire)
        } else {
            fid.to_string()
        };
        let changed = existing
            .as_ref()
            .is_some_and(|entry| entry.file_id != file_id || entry.path != wire);

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

        let first = map.save_edited("mod:jei", "mods/jei.jar", "aaa", "my-project", "file-1");
        assert_eq!(first.project_id, "my-project");

        let second = map.save_edited("mod:jei", "mods/jei.jar", "bbb", "other-project", "file-2");
        assert_eq!(second.project_id, "my-project", "已生成的项目编号不能被改");
        assert_eq!(second.file_id, "file-2", "文件编号仍可改");

        let new = map.save_edited("mod:create", "mods/create.jar", "ccc", "create-project", "");
        assert_eq!(new.project_id, "create-project", "新增项目可以采用手填项目编号");
    }
}
