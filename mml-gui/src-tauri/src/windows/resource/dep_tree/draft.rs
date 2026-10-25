//! 依赖树的草稿层
//!
//! 先按「一个模组条目一份草稿」铺开，节点信息从模组元数据里取，
//! 合并与去重都在这一层完成，建图只读草稿。

use std::collections::{HashMap, HashSet};

use mml_game::game_mods::{ModObj, ProvidedVersion, provided_version};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

use crate::dtos::ModDepEdgeDto;

use crate::windows::resource::data_url;

/// 提供某个 modid 的一个条目
pub(super) struct Provider<'a> {
    /// 提供它的条目
    pub(super) item: &'a ModObj,
    /// 由内置 jar（jar-in-jar）提供
    pub(super) nested: bool,
}

/// 建图过程中的节点草稿：比最终节点多几个只用于判定的字段
pub(super) struct NodeDraft {
    /// 唯一键
    pub(super) key: String,
    /// 原始大小写的 modid
    pub(super) mod_id: String,
    /// 显示名
    pub(super) label: String,
    /// 已装版本
    pub(super) version: String,
    /// 图标 data URL
    pub(super) icon: String,
    /// 是不是根
    pub(super) root: bool,
    /// 由内置 jar 提供
    pub(super) nested: bool,
    /// 由游戏 / 加载器 / 运行环境提供
    pub(super) game_provided: bool,
    /// 只有禁用包提供它
    pub(super) disabled: bool,
    /// 有模组提供它
    pub(super) has_provider: bool,
    /// 指向它的边声明的版本要求
    pub(super) required: Vec<String>,
    /// 至少有一条强制边指向它
    pub(super) has_required: bool,
}

/// 建图的中间状态
pub(super) struct Drafts {
    /// 节点草稿，按建立顺序
    pub(super) nodes: Vec<NodeDraft>,
    /// 节点键 → 在 `nodes` 里的下标
    pub(super) index: HashMap<String, usize>,
    /// 连线，按建立顺序
    pub(super) edges: Vec<ModDepEdgeDto>,
    /// 已经连过的 `from>to`
    pub(super) edge_seen: HashSet<String>,
    /// 已经铺开过前置的节点
    pub(super) expanded: HashSet<String>,
    /// 当前遍历路径上的节点键，用来认环
    pub(super) path: HashSet<String>,
}

/// 条目在元数据里的 modid，没有元数据时是空串
pub(super) fn modid_of(item: &ModObj) -> String {
    item.info
        .first()
        .map(|info| info.mod_id.clone())
        .unwrap_or_default()
}

/// 条目的文件名，取不到时是空串
pub(super) fn file_name_of(item: &ModObj) -> String {
    item.file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 条目声明的显示名，没有元数据时是空串
pub(super) fn name_of(item: &ModObj) -> String {
    item.info
        .first()
        .map(|info| info.name.clone())
        .unwrap_or_default()
}

/// 条目声明的版本，没有元数据时是空串
pub(super) fn version_of(item: &ModObj) -> String {
    item.info
        .first()
        .and_then(|info| info.version.clone())
        .unwrap_or_default()
}

/// 条目的图标 data URL，没有图标时是空串
pub(super) fn icon_of(item: &ModObj) -> String {
    data_url(item.info.iter().find_map(|info| info.icon.as_ref()))
}

/// 节点键：modid 小写；没有 modid 时退回文件名
pub(super) fn key_of(item: &ModObj) -> String {
    let mod_id = modid_of(item);
    if mod_id.is_empty() {
        file_name_of(item).to_lowercase()
    } else {
        mod_id.to_lowercase()
    }
}

/// 这个前置是不是游戏 / 加载器提供的，以及它的版本
///
/// 与内核 `provided_version` 同一份名单：命中返回它的版本（版本未知时是空串），
/// 由模组包提供的返回 `None`。
pub(super) fn virtual_version(game: &InstanceSettingObj, id: &str) -> Option<String> {
    match provided_version(game, id) {
        ProvidedVersion::Known(version) => Some(version.to_string()),
        ProvidedVersion::Unknown => Some(String::new()),
        ProvidedVersion::FromMods => None,
    }
}

/// 记下一个提供者
pub(super) fn add_provider<'a>(
    providers: &mut HashMap<String, Vec<Provider<'a>>>,
    item: &'a ModObj,
    nested: bool,
) {
    let id = modid_of(item).to_lowercase();
    if id.is_empty() {
        return;
    }
    providers
        .entry(id)
        .or_default()
        .push(Provider { item, nested });
}

/// 递归记下内置 jar（jar-in-jar）里的提供者
pub(super) fn add_nested<'a>(providers: &mut HashMap<String, Vec<Provider<'a>>>, item: &'a ModObj) {
    for child in item.jar_in_jar.iter() {
        add_provider(providers, child, true);
        add_nested(providers, child);
    }
}

/// 这个 modid 用哪个条目展开：装了的那份优先，全禁用时取第一个
pub(super) fn chosen_of<'a>(
    providers: &'a HashMap<String, Vec<Provider<'a>>>,
    id: &str,
) -> Option<&'a ModObj> {
    let list = providers.get(id)?;
    list.iter()
        .find(|provider| !provider.item.disable)
        .or_else(|| list.first())
        .map(|provider| provider.item)
}
