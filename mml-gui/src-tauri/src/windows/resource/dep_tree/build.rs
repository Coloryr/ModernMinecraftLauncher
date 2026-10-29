//! 由草稿建树
//!
//! 选中项定根，前置递归展开成节点，重复出现的模组标成问题而不是再画一遍。

use std::collections::{HashMap, HashSet};

use mml_game::game_mods::{DependantProblemKind, ModObj, loader_modids, mods_dep_test};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

use crate::dtos::{DepProblemKindDto, ModDepEdgeDto, ModDepNodeDto, ModDepTreeDto};
use crate::err_keys;

use super::draft::{
    Drafts, NodeDraft, Provider, add_nested, add_provider, chosen_of, file_name_of, icon_of,
    key_of, modid_of, name_of, version_of, virtual_version,
};
use crate::windows::resource::mods::{ModDependencyRef, mod_dependencies};

/// 按 modid 取节点下标，没有就按它的提供者建一个
///
/// - `id`: modid，小写
/// - `providers`: modid 小写 → 提供它的条目
/// - `game`: 实例设置，用来判断虚拟前置与版本
/// - `drafts`: 建图中间状态
///
/// # 返回值
///
/// 节点在 `drafts.nodes` 里的下标；新建的追加在末尾
pub(super) fn node_of(
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
pub(super) fn node_of_item(item: &ModObj, drafts: &mut Drafts) -> usize {
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
pub(super) fn set_root(item: &ModObj, drafts: &mut Drafts) -> usize {
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
///
/// - `key`: 当前节点键
/// - `deps`: 当前节点声明的前置
/// - `providers`: modid 小写 → 提供它的条目
/// - `game`: 实例设置，取加载器并判断虚拟前置
/// - `drafts`: 建图中间状态
pub(super) fn walk(
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
///
/// - `game`: 实例设置
/// - `list`: 模组扫描结果
///
/// # 返回值
///
/// modid 小写 → 问题种类；依赖检测整体失败时是空表
pub(super) fn problem_map(
    game: &InstanceSettingObj,
    list: &[ModObj],
) -> HashMap<String, DepProblemKindDto> {
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
pub(super) fn find_by_uuid<'a>(list: &'a [ModObj], uuid: &str) -> Option<&'a ModObj> {
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
