//! 依赖树相关单元测试

use super::build::*;
use super::related::*;


use mml_game::game_mods::{ModObj};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::loader::LoaderType;

use crate::dtos::{ModDepNodeDto, ModDepTreeDto};


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
