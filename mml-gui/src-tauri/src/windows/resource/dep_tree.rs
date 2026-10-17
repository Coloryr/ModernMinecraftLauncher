//! 资源窗口的模组依赖图：从窗口模型那份扫描结果里整理出节点与连线
//!
//! 这张图原来是在前端建的（`mml-vue/.../composables/modDepGraph.ts`），按 AGENTS.md §12.1
//! 搬到后端：依赖关系属于"换个界面回来还要用、重新算一遍不便宜"的那一类，
//! 前端只留 dagre 布局与画，关系一个字都不留。
//!
//! 建图口径：
//! - **按 modid 去重**：同一个前置被多条路径共用、甚至绕回来成环时只画一个节点，
//!   同一对节点只留一条边；
//! - 只沿**直接依赖**往下铺，展开过的节点不再往下（否则菱形会展开成指数级）；
//! - **反向只铺一层**：谁直接声明了它是前置，再往上会牵出整张网，图就没法看了；
//! - 游戏 / 加载器提供的"虚拟前置"与内核 `provided_version` 同一份名单；
//! - 加载器 → minecraft 那条不是谁声明的，是运行环境的常识，两边都在图上时补一条。

use std::collections::{HashMap, HashSet};

use mml_game::game_mods::{
    DependantProblemKind, ModObj, ProvidedVersion, loader_modids, mods_dep_test, provided_version,
};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::loader::LoaderType;

use crate::dtos::{DepProblemKindDto, ModDepEdgeDto, ModDepNodeDto, ModDepTreeDto};
use crate::err_keys;

use super::model::window_model;
use super::mods::{ModDependencyRef, mod_dependencies};
use super::{block_on_instance, data_url, parse_instance};

/// 提供某个 modid 的一个条目
struct Provider<'a> {
    /// 提供它的条目
    item: &'a ModObj,
    /// 由内置 jar（jar-in-jar）提供
    nested: bool,
}

/// 建图过程中的节点草稿：比最终节点多几个只用于判定的字段
struct NodeDraft {
    /// 唯一键
    key: String,
    /// 原始大小写的 modid
    mod_id: String,
    /// 显示名
    label: String,
    /// 已装版本
    version: String,
    /// 图标 data URL
    icon: String,
    /// 是不是根
    root: bool,
    /// 由内置 jar 提供
    nested: bool,
    /// 由游戏 / 加载器 / 运行环境提供
    game_provided: bool,
    /// 只有禁用包提供它
    disabled: bool,
    /// 有模组提供它
    has_provider: bool,
    /// 指向它的边声明的版本要求
    required: Vec<String>,
    /// 至少有一条强制边指向它
    has_required: bool,
}

/// 建图的中间状态
struct Drafts {
    /// 节点草稿，按建立顺序
    nodes: Vec<NodeDraft>,
    /// 节点键 → 在 `nodes` 里的下标
    index: HashMap<String, usize>,
    /// 连线，按建立顺序
    edges: Vec<ModDepEdgeDto>,
    /// 已经连过的 `from>to`
    edge_seen: HashSet<String>,
    /// 已经铺开过前置的节点
    expanded: HashSet<String>,
    /// 当前遍历路径上的节点键，用来认环
    path: HashSet<String>,
}

/// 条目在元数据里的 modid，没有元数据时是空串
fn modid_of(item: &ModObj) -> String {
    item.info
        .first()
        .map(|info| info.mod_id.clone())
        .unwrap_or_default()
}

/// 条目的文件名，取不到时是空串
fn file_name_of(item: &ModObj) -> String {
    item.file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 条目声明的显示名，没有元数据时是空串
fn name_of(item: &ModObj) -> String {
    item.info
        .first()
        .map(|info| info.name.clone())
        .unwrap_or_default()
}

/// 条目声明的版本，没有元数据时是空串
fn version_of(item: &ModObj) -> String {
    item.info
        .first()
        .and_then(|info| info.version.clone())
        .unwrap_or_default()
}

/// 条目的图标 data URL，没有图标时是空串
fn icon_of(item: &ModObj) -> String {
    data_url(item.info.iter().find_map(|info| info.icon.as_ref()))
}

/// 节点键：modid 小写；没有 modid 时退回文件名
fn key_of(item: &ModObj) -> String {
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
fn virtual_version(game: &InstanceSettingObj, id: &str) -> Option<String> {
    match provided_version(game, id) {
        ProvidedVersion::Known(version) => Some(version.to_string()),
        ProvidedVersion::Unknown => Some(String::new()),
        ProvidedVersion::FromMods => None,
    }
}

/// 记下一个提供者
fn add_provider<'a>(
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
fn add_nested<'a>(providers: &mut HashMap<String, Vec<Provider<'a>>>, item: &'a ModObj) {
    for child in item.jar_in_jar.iter() {
        add_provider(providers, child, true);
        add_nested(providers, child);
    }
}

/// 这个 modid 用哪个条目展开：装了的那份优先，全禁用时取第一个
fn chosen_of<'a>(
    providers: &'a HashMap<String, Vec<Provider<'a>>>,
    id: &str,
) -> Option<&'a ModObj> {
    let list = providers.get(id)?;
    list.iter()
        .find(|provider| !provider.item.disable)
        .or_else(|| list.first())
        .map(|provider| provider.item)
}

/// 按 modid 取节点下标，没有就按它的提供者建一个
fn node_of(
    id: &str,
    providers: &HashMap<String, Vec<Provider>>,
    game: &InstanceSettingObj,
    drafts: &mut Drafts,
) -> usize {
    if let Some(index) = drafts.index.get(id) {
        return *index;
    }

    let empty: Vec<Provider> = Vec::new();
    let list = providers.get(id).unwrap_or(&empty);
    let chosen = list
        .iter()
        .find(|provider| !provider.item.disable)
        .or_else(|| list.first());
    // 有模组提供时就轮不到虚拟前置：装了的那份优先
    let virtual_version = if list.is_empty() {
        virtual_version(game, id)
    } else {
        None
    };
    let versions = list
        .iter()
        .map(|provider| version_of(provider.item))
        .filter(|version| !version.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let draft = NodeDraft {
        key: id.to_string(),
        mod_id: chosen
            .map(|provider| modid_of(provider.item))
            .unwrap_or_else(|| id.to_string()),
        label: chosen
            .map(|provider| name_of(provider.item))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| id.to_string()),
        version: if versions.is_empty() {
            virtual_version.clone().unwrap_or_default()
        } else {
            versions
        },
        icon: chosen
            .map(|provider| icon_of(provider.item))
            .unwrap_or_default(),
        root: false,
        nested: chosen.is_some_and(|provider| provider.nested),
        game_provided: virtual_version.is_some(),
        // 全是禁用包提供时才算"被禁用"；有启用的那份就正常
        disabled: !list.is_empty() && list.iter().all(|provider| provider.item.disable),
        has_provider: !list.is_empty(),
        required: Vec::new(),
        has_required: false,
    };
    drafts.index.insert(id.to_string(), drafts.nodes.len());
    drafts.nodes.push(draft);
    drafts.nodes.len() - 1
}

/// 按"具体的那个条目"建节点：反向上游那一层直接拿模组条目，不经提供者索引
fn node_of_item(item: &ModObj, drafts: &mut Drafts) -> usize {
    let id = key_of(item);
    if let Some(index) = drafts.index.get(&id) {
        return *index;
    }

    let name = name_of(item);
    let draft = NodeDraft {
        key: id.clone(),
        mod_id: modid_of(item),
        label: if name.is_empty() { id.clone() } else { name },
        version: version_of(item),
        icon: icon_of(item),
        root: false,
        nested: false,
        game_provided: false,
        disabled: item.disable,
        has_provider: !item.fail,
        required: Vec::new(),
        has_required: false,
    };
    drafts.index.insert(id, drafts.nodes.len());
    drafts.nodes.push(draft);
    drafts.nodes.len() - 1
}

/// 建根节点
fn set_root(item: &ModObj, drafts: &mut Drafts) -> usize {
    let key = key_of(item);
    let name = name_of(item);
    let file = file_name_of(item);
    let label = if !name.is_empty() {
        name
    } else if !file.is_empty() {
        file
    } else {
        modid_of(item)
    };

    let draft = NodeDraft {
        key: key.clone(),
        mod_id: modid_of(item),
        label,
        version: version_of(item),
        icon: icon_of(item),
        root: true,
        nested: false,
        game_provided: false,
        disabled: item.disable,
        has_provider: !item.fail,
        required: Vec::new(),
        has_required: false,
    };
    drafts.index.insert(key, drafts.nodes.len());
    drafts.nodes.push(draft);
    drafts.nodes.len() - 1
}

/// 深度优先铺开：同一对节点只连一条边，撞上路径上的节点就是环，不再往下走
fn walk(
    key: &str,
    deps: &[ModDependencyRef],
    providers: &HashMap<String, Vec<Provider>>,
    game: &InstanceSettingObj,
    drafts: &mut Drafts,
) {
    drafts.path.insert(key.to_string());
    for dep in deps {
        let id = dep.mod_id.to_lowercase();
        if id.is_empty() {
            continue;
        }
        let child = node_of(&id, providers, game, drafts);
        let edge_key = format!("{key}>{id}");
        if !drafts.edge_seen.contains(&edge_key) {
            drafts.edge_seen.insert(edge_key.clone());
            let backward = drafts.path.contains(&id);
            drafts.edges.push(ModDepEdgeDto {
                id: edge_key,
                from: key.to_string(),
                to: id.clone(),
                optional: dep.optional,
                required: dep.required.clone(),
                backward,
                loader: false,
            });
            if !dep.optional {
                drafts.nodes[child].has_required = true;
                let range = &dep.required;
                if !range.is_empty()
                    && range != "*"
                    && !drafts.nodes[child].required.contains(range)
                {
                    drafts.nodes[child].required.push(range.clone());
                }
            }
        }

        // 环上的节点不展开；展开过的也只连边，不重复铺开
        if drafts.path.contains(&id) || drafts.expanded.contains(&id) {
            continue;
        }
        let Some(chosen) = chosen_of(providers, &id) else {
            continue;
        };
        drafts.expanded.insert(id.clone());
        let children = mod_dependencies(chosen, game.loader);
        walk(&id, &children, providers, game, drafts);
    }
    drafts.path.remove(key);
}

/// 依赖检测的结论：modid 小写 → 问题种类
///
/// 与「依赖检测」同一个口径（内核 `mods_dep_test`），节点靠它决定打勾还是打感叹号。
fn problem_map(game: &InstanceSettingObj, list: &[ModObj]) -> HashMap<String, DepProblemKindDto> {
    let mut map = HashMap::new();
    let Ok(test) = mods_dep_test(game, list) else {
        return map;
    };
    for problem in test.problems {
        let kind = match problem.kind {
            DependantProblemKind::Missing => DepProblemKindDto::Missing,
            DependantProblemKind::Mismatch => DepProblemKindDto::Mismatch,
            DependantProblemKind::Disabled => DepProblemKindDto::Disabled,
        };
        map.insert(problem.modid.to_lowercase(), kind);
    }
    map
}

/// 在扫描结果里按 uuid 找条目，内置的（jar-in-jar）一并往下找
///
/// 图上双击可以把内置模组换成根，而内置条目的 uuid 不在顶层列表里 —— 只查顶层会报"找不到文件"。
///
/// - `list`: 扫描结果
/// - `uuid`: 目标条目 uuid
///
/// # 返回值
///
/// 找到的条目；没有时返回 `None`
fn find_by_uuid<'a>(list: &'a [ModObj], uuid: &str) -> Option<&'a ModObj> {
    for item in list {
        if item.uuid.to_string() == uuid {
            return Some(item);
        }
        if let Some(found) = find_by_uuid(&item.jar_in_jar, uuid) {
            return Some(found);
        }
    }
    None
}

/// 从一个模组出发建依赖图
///
/// - `game`: 实例设置，取游戏版本 / 加载器，并跑一遍依赖检测
/// - `list`: 窗口模型里那份扫描结果
/// - `root_uuid`: 图根那个模组的 uuid，即文件路径的 v5
///
/// # 返回值
///
/// 见 [`ModDepTreeDto`]；列表里没有这个 uuid 时返回 `err.fileNotFound`
pub(super) fn build_tree(
    game: &InstanceSettingObj,
    list: &[ModObj],
    root_uuid: &str,
) -> Result<ModDepTreeDto, String> {
    let root = find_by_uuid(list, root_uuid).ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;

    // modid 小写 → 提供它的条目：先铺顶层再铺内置，与列表里的先后一致
    let mut providers: HashMap<String, Vec<Provider>> = HashMap::new();
    for item in list {
        add_provider(&mut providers, item, false);
    }
    for item in list {
        add_nested(&mut providers, item);
    }

    // modid 小写 → 声明它是前置的条目与那份声明，反向那一层用
    let mut dependents: HashMap<String, Vec<(&ModObj, ModDependencyRef)>> = HashMap::new();
    for item in list {
        if item.fail || modid_of(item).is_empty() {
            continue;
        }
        for dep in mod_dependencies(item, game.loader) {
            dependents
                .entry(dep.mod_id.to_lowercase())
                .or_default()
                .push((item, dep));
        }
    }

    let mut drafts = Drafts {
        nodes: Vec::new(),
        index: HashMap::new(),
        edges: Vec::new(),
        edge_seen: HashSet::new(),
        expanded: HashSet::new(),
        path: HashSet::new(),
    };

    let root_key = key_of(root);
    set_root(root, &mut drafts);
    let root_deps = mod_dependencies(root, game.loader);
    walk(&root_key, &root_deps, &providers, game, &mut drafts);

    // 反向只铺**一层**：谁直接声明了它是前置
    if let Some(list) = dependents.get(&root_key) {
        for (item, dep) in list {
            let id = key_of(item);
            if id.is_empty() || id == root_key {
                continue;
            }
            let edge_key = format!("{id}>{root_key}");
            if drafts.edge_seen.contains(&edge_key) {
                continue;
            }
            drafts.edge_seen.insert(edge_key.clone());
            node_of_item(item, &mut drafts);
            drafts.edges.push(ModDepEdgeDto {
                id: edge_key,
                from: id,
                to: root_key.clone(),
                optional: dep.optional,
                required: dep.required.clone(),
                backward: false,
                loader: false,
            });
        }
    }

    // 加载器 → minecraft：两边都出现在图上时才补这一条
    if drafts.index.contains_key("minecraft") {
        for id in loader_modids(game.loader) {
            if !drafts.index.contains_key(*id) {
                continue;
            }
            let edge_key = format!("{id}>minecraft");
            if drafts.edge_seen.contains(&edge_key) {
                continue;
            }
            drafts.edge_seen.insert(edge_key.clone());
            drafts.edges.push(ModDepEdgeDto {
                id: edge_key,
                from: (*id).to_string(),
                to: "minecraft".to_string(),
                optional: false,
                required: "*".to_string(),
                backward: false,
                loader: true,
            });
        }
    }

    let problems = problem_map(game, list);
    let nodes = drafts
        .nodes
        .into_iter()
        .map(|draft| ModDepNodeDto {
            key: draft.key.clone(),
            label: draft.label,
            mod_id: draft.mod_id,
            version: draft.version,
            icon: draft.icon,
            root: draft.root,
            nested: draft.nested,
            r#virtual: draft.game_provided,
            installed: draft.has_provider || draft.game_provided,
            // 缺失只认"强制指过来、又没人提供、也不是虚拟前置"：可选前置没装不算缺失
            missing: !draft.has_provider && !draft.game_provided && draft.has_required,
            disabled: draft.disabled,
            mandatory: draft.has_required,
            required: draft.required,
            problem: problems.get(&draft.key).copied(),
        })
        .collect();

    Ok(ModDepTreeDto {
        nodes,
        edges: drafts.edges,
    })
}

/// 要和这几个模组一起改的那些条目
///
/// - `disable = true`：还有哪些**启用**的模组依赖它们 —— 少了前置轻则功能缺失、重则起不来
/// - `disable = false`：它们要求的强制前置里，现在被禁用着的那些
///
/// - `list`: 窗口模型里那份扫描结果
/// - `loader`: 实例的加载器
/// - `picked`: 这次要改的模组 uuid
/// - `disable`: true = 这次是禁用
///
/// # 返回值
///
/// 要一起改的条目 uuid，按列表顺序；不含 `picked` 自己
pub(super) fn related_mods(
    list: &[ModObj],
    loader: LoaderType,
    picked: &[String],
    disable: bool,
) -> Vec<String> {
    let picked_set: HashSet<String> = picked.iter().cloned().collect();
    // 这次动的这些模组自己的 modid（小写）：禁用时按它找依赖方
    let mut own_ids: HashSet<String> = HashSet::new();
    for item in list {
        if !picked_set.contains(&item.uuid.to_string()) {
            continue;
        }
        for info in item.info.iter() {
            if !info.mod_id.is_empty() {
                own_ids.insert(info.mod_id.to_lowercase());
            }
        }
    }

    if disable {
        return list
            .iter()
            .filter(|item| !item.disable && !item.fail)
            .filter(|item| !picked_set.contains(&item.uuid.to_string()))
            .filter(|item| {
                mod_dependencies(item, loader)
                    .iter()
                    .any(|dep| !dep.optional && own_ids.contains(&dep.mod_id.to_lowercase()))
            })
            .map(|item| item.uuid.to_string())
            .collect();
    }

    // 启用：把它们要求的强制前置收集起来，再挑出现在被禁用着的那些
    let mut wanted: HashSet<String> = HashSet::new();
    for item in list {
        if !picked_set.contains(&item.uuid.to_string()) {
            continue;
        }
        for dep in mod_dependencies(item, loader) {
            if !dep.optional {
                wanted.insert(dep.mod_id.to_lowercase());
            }
        }
    }
    if wanted.is_empty() {
        return Vec::new();
    }
    list.iter()
        .filter(|item| item.disable && !picked_set.contains(&item.uuid.to_string()))
        .filter(|item| wanted.contains(&modid_of(item).to_lowercase()))
        .map(|item| item.uuid.to_string())
        .collect()
}

/// 模组依赖图：从窗口模型那份扫描结果建图
///
/// **不重扫 mods 目录**，节点与连线都由后端算好，前端只负责布局与画。
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 图根那个模组的 uuid，由前端从列表项里取
///
/// # 返回值
///
/// 见 [`ModDepTreeDto`]；实例不存在、模组列表还没加载、或列表里没有这个模组时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_dep_tree(
    uuid: String,
    mod_uuid: String,
) -> Result<ModDepTreeDto, String> {
    let model = window_model(&uuid)?;
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, move |game| {
        model
            .with_cache(|cache| build_tree(&game, &cache.list, &mod_uuid))
            .ok_or_else(|| err_keys::MODS_NOT_LOADED.to_string())?
    })
    .await?
}

/// 要和这几个模组一起改的条目：给"一起禁用 / 一起启用"那个确认框用
///
/// 关系在后端算（见 [`related_mods`]），前端拿到 uuid 去自己的列表里对。
///
/// - `uuid`: 实例 uuid
/// - `mods`: 这次要改的模组 uuid
/// - `disable`: true = 这次是禁用，false = 启用
///
/// # 返回值
///
/// 要一起改的条目 uuid，按列表顺序；实例不存在或模组列表还没加载时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mods_related_mods(
    uuid: String,
    mods: Vec<String>,
    disable: bool,
) -> Result<Vec<String>, String> {
    let model = window_model(&uuid)?;
    let instance = parse_instance(&uuid)?;

    block_on_instance(instance, move |game| {
        model
            .with_cache(|cache| related_mods(&cache.list, game.loader, &mods, disable))
            .ok_or_else(|| err_keys::MODS_NOT_LOADED.to_string())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use mml_base::range_checker::RangeSyntax;
    use mml_game::game_mods::{DependantInfo, DependantType, ModItemObj};

    /// 造一条前置声明
    ///
    /// - `mod_id`: 前置的 modid
    /// - `optional`: 是不是推荐安装
    /// - `range`: 版本要求
    fn dep(mod_id: &str, optional: bool, range: &str) -> DependantType {
        let info = DependantInfo::parse_required_version(mod_id, range, RangeSyntax::Maven);
        if optional {
            DependantType::Recommend(info)
        } else {
            DependantType::Required(info)
        }
    }

    /// 造一个模组条目，只填建图用得到的字段
    ///
    /// - `mod_id`: modid，同时用作文件名
    /// - `deps`: 它声明的前置，(modid, 是否可选, 版本要求)
    fn item(mod_id: &str, deps: &[(&str, bool, &str)]) -> ModObj {
        let file = PathBuf::from(format!("{mod_id}.jar"));
        ModObj {
            uuid: mml_game::game_mods::gen_mod_uuid(&file),
            file,
            info: vec![ModItemObj {
                mod_id: mod_id.to_string(),
                name: mod_id.to_string(),
                version: Some("1.0".to_string()),
                loaders: LoaderType::Forge,
                dependants: deps
                    .iter()
                    .map(|(id, optional, range)| dep(id, *optional, range))
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// 造实例设置：只要游戏版本与加载器
    fn game(loader: LoaderType, version: &str, loader_version: &str) -> InstanceSettingObj {
        InstanceSettingObj {
            version: version.to_string(),
            loader,
            loader_version: Some(loader_version.to_string()),
            ..Default::default()
        }
    }

    /// 默认实例：Forge 1.20.4 / 47.2.0
    fn forge() -> InstanceSettingObj {
        game(LoaderType::Forge, "1.20.4", "47.2.0")
    }

    /// 建图，列表里的第一个条目当根
    fn tree_of(list: &[ModObj], game: &InstanceSettingObj) -> ModDepTreeDto {
        build_tree(game, list, &list[0].uuid.to_string()).unwrap()
    }

    /// 节点键集合，排序后方便断言"出现且只出现一次"
    fn keys(tree: &ModDepTreeDto) -> Vec<String> {
        let mut keys: Vec<String> = tree.nodes.iter().map(|node| node.key.clone()).collect();
        keys.sort();
        keys
    }

    /// 取一个节点
    fn node<'a>(tree: &'a ModDepTreeDto, key: &str) -> &'a ModDepNodeDto {
        tree.nodes
            .iter()
            .find(|node| node.key == key)
            .expect("节点应当存在")
    }

    /// 同一个前置被两条路径共用：只画一个节点，两条边汇过去
    #[test]
    fn shared_prerequisite_has_one_node_two_edges() {
        let c = item("C", &[]);
        let a = item("A", &[("C", false, "[1.0,)")]);
        let b = item("B", &[("C", false, "*")]);
        let root = item("Root", &[("A", false, "*"), ("B", false, "*")]);

        let tree = tree_of(&[root, a, b, c], &forge());

        assert_eq!(keys(&tree), ["a", "b", "c", "root"]);
        assert_eq!(tree.edges.iter().filter(|edge| edge.to == "c").count(), 2);
        assert_eq!(tree.edges.iter().filter(|edge| edge.backward).count(), 0);
        for key in ["a", "b", "c"] {
            assert!(node(&tree, key).installed, "{key} 应当算装了");
        }
    }

    /// 环：回边标出来且不再展开，节点不重复
    #[test]
    fn cycle_marks_backward_edge() {
        let a = item("A", &[("Root", false, "*")]);
        let root = item("Root", &[("A", false, "*")]);

        let tree = tree_of(&[root, a], &forge());

        assert_eq!(keys(&tree), ["a", "root"]);
        let backward: Vec<_> = tree.edges.iter().filter(|edge| edge.backward).collect();
        assert_eq!(backward.len(), 1);
        assert_eq!(backward[0].from, "a");
        assert_eq!(backward[0].to, "root");
    }

    /// 模组自己依赖自己：只留下一条回边
    #[test]
    fn self_dependency_leaves_one_backward_edge() {
        let root = item("Root", &[("Root", false, "*")]);

        let tree = tree_of(&[root], &forge());

        assert_eq!(keys(&tree), ["root"]);
        assert_eq!(tree.edges.len(), 1);
        assert!(tree.edges[0].backward);
    }

    /// 同一条声明写两遍：只连一条边
    #[test]
    fn duplicated_declaration_has_one_edge() {
        let c = item("C", &[]);
        let root = item("Root", &[("C", false, "*"), ("C", false, "*")]);

        let tree = tree_of(&[root, c], &forge());

        assert_eq!(tree.edges.iter().filter(|edge| edge.to == "c").count(), 1);
    }

    /// 缺失只认强制前置：可选前置没装不算缺失，虚拟前置也不算
    #[test]
    fn missing_only_counts_required() {
        let root = item(
            "Root",
            &[
                ("Missing", false, "*"),
                ("Optional", true, "*"),
                ("Minecraft", false, "*"),
            ],
        );

        let tree = tree_of(&[root], &forge());

        let missing = node(&tree, "missing");
        assert!(missing.missing);
        assert!(!missing.installed);
        assert!(missing.mandatory);

        let optional = node(&tree, "optional");
        assert!(!optional.mandatory);
        assert!(!optional.missing);

        let virtual_node = node(&tree, "minecraft");
        assert!(virtual_node.r#virtual);
        assert!(virtual_node.installed);
        assert!(!virtual_node.missing);
        assert_eq!(virtual_node.version, "1.20.4");

        assert!(
            tree.edges
                .iter()
                .find(|edge| edge.to == "optional")
                .is_some_and(|edge| edge.optional)
        );
    }

    /// 反着只画一层：谁直接依赖它，那一级自己的上游不再展开
    #[test]
    fn reverse_layer_is_one_level() {
        let root = item("Root", &[]);
        let parent = item("Parent", &[("Root", false, "*")]);
        let grand = item("Grand", &[("Parent", false, "*")]);

        let tree = tree_of(&[root, parent, grand], &forge());

        assert_eq!(keys(&tree), ["parent", "root"]);
        let edge = tree
            .edges
            .iter()
            .find(|edge| edge.from == "parent" && edge.to == "root")
            .expect("应当有一条 Parent → Root 的边");
        assert!(!edge.optional);
        assert!(node(&tree, "parent").installed);
    }

    /// 反过来那条边照样分可选 / 必装
    #[test]
    fn reverse_edge_keeps_optional() {
        let root = item("Root", &[]);
        let soft = item("Soft", &[("Root", true, "*")]);

        let tree = tree_of(&[root, soft], &forge());

        let edge = tree
            .edges
            .iter()
            .find(|edge| edge.from == "soft" && edge.to == "root")
            .expect("应当有一条 Soft → Root 的边");
        assert!(edge.optional);
    }

    /// 加载器 → minecraft：两边都在图上时补一条合成边
    #[test]
    fn loader_edge_is_added_when_both_sides_present() {
        let root = item(
            "Root",
            &[("Minecraft", false, "*"), ("MinecraftForge", false, "*")],
        );

        let tree = tree_of(&[root], &forge());

        let link = tree
            .edges
            .iter()
            .find(|edge| edge.from == "minecraftforge" && edge.to == "minecraft")
            .expect("应当补一条 加载器 → minecraft 的边");
        assert!(link.loader);
        // 模组自己声明的那条边不能带上这个标记
        let declared = tree
            .edges
            .iter()
            .find(|edge| edge.from == "root" && edge.to == "minecraft")
            .expect("模组声明的那条边还在");
        assert!(!declared.loader);
    }

    /// 只声明了 minecraft 时不凭空补出加载器
    #[test]
    fn loader_edge_needs_loader_node() {
        let root = item("Root", &[("Minecraft", false, "*")]);

        let tree = tree_of(&[root], &forge());

        assert_eq!(keys(&tree), ["minecraft", "root"]);
        assert!(!tree.edges.iter().any(|edge| edge.loader));
    }

    /// 图标与显示名跟着提供者走，内置 jar 提供的标 nested
    #[test]
    fn provider_details_come_from_the_item() {
        let mut nested = item("Bundled", &[]);
        nested.info[0].name = "Bundled Lib".to_string();
        let mut parent = item("Parent", &[]);
        parent.jar_in_jar = vec![nested];
        let root = item("Root", &[("Bundled", false, "*")]);

        let tree = tree_of(&[root, parent], &forge());

        let found = node(&tree, "bundled");
        assert_eq!(found.label, "Bundled Lib");
        assert!(found.nested);
    }

    /// 根可以是内置模组：图上双击换到它时，uuid 不在顶层列表里
    #[test]
    fn root_can_be_a_nested_item() {
        let nested = item("Bundled", &[("API", false, "*")]);
        let api = item("API", &[]);
        let mut parent = item("Parent", &[]);
        parent.jar_in_jar = vec![nested];
        let list = vec![parent, api];
        let root_uuid = list[0].jar_in_jar[0].uuid.to_string();

        let tree = build_tree(&forge(), &list, &root_uuid).unwrap();

        assert!(tree.nodes.iter().any(|node| node.key == "bundled"));
        assert!(tree.nodes.iter().any(|node| node.key == "api"));
    }

    /// 依赖方与前置：禁用方向找"谁还需要它"
    #[test]
    fn related_on_disable_finds_enabled_dependents() {
        let api = item("API", &[]);
        let user = item("User", &[("API", false, "*")]);
        let soft = item("Soft", &[("API", true, "*")]);
        let mut disabled_user = item("Off", &[("API", false, "*")]);
        disabled_user.disable = true;

        let api_uuid = api.uuid.to_string();
        let user_uuid = user.uuid.to_string();
        let list = vec![api, user, soft, disabled_user];
        let related = related_mods(&list, LoaderType::Forge, &[api_uuid], true);

        assert_eq!(related, vec![user_uuid]);
    }

    /// 前置与依赖方：启用方向找"它要的前置里还有哪些禁用着"
    #[test]
    fn related_on_enable_finds_disabled_prerequisites() {
        let mut api = item("API", &[]);
        api.disable = true;
        let mut lib = item("Lib", &[]);
        lib.disable = true;
        let user = item("User", &[("API", false, "*"), ("Lib", true, "*")]);

        let api_uuid = api.uuid.to_string();
        let user_uuid = user.uuid.to_string();
        let list = vec![api, lib, user];
        let related = related_mods(&list, LoaderType::Forge, &[user_uuid], false);

        // 只收强制前置：Lib 是推荐前置，不进来
        assert_eq!(related, vec![api_uuid]);
    }
}
