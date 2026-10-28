//! 收藏数据持久化集成测试
//!
//! 收藏、分组、清空都只改内存再异步落盘，这里走完整链路：
//! 增删改 → `config_save::stop()` 等落盘 → 读文件核对 → `load()` 读回内存核对。
//!
//! 收藏模块用的是进程级单例（`FILE` / `COLLECT`），所以全部断言集中在一条用例里顺序执行。
//! 运行目录在仓库 `target/temp` 下，按进程号区分，跑完清理。

use std::fs;
use std::path::PathBuf;

use mml_game::launcher::{FileType, ModPackType};
use mml_gui_lib::collect_utils::{self, CollectItemObj, CollectObj};

/// 本次运行的独立目录，避免和其它测试抢同一份收藏文件
fn run_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join(format!("mml-collect-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 起齐全局单例：运行目录、路径、日志、配置与异步保存线程
fn setup(dir: &PathBuf) {
    mml_base::init(dir);
    mml_names::init(mml_base::get_base_dir()).unwrap();
    mml_log::start(mml_base::get_base_dir()).unwrap();
    mml_config::init(mml_base::get_base_dir()).unwrap();
    mml_config::config_save::start();
    collect_utils::init(dir);
}

/// 造一条收藏
///
/// - `pid`: 项目 ID，同一路径下靠它判重
fn item(pid: &str, name: &str) -> CollectItemObj {
    CollectItemObj {
        source: ModPackType::Modrinth,
        file_type: FileType::Mod,
        name: name.to_string(),
        pid: pid.to_string(),
        icon: None,
        url: format!("https://modrinth.com/mod/{pid}"),
        loaders: vec!["fabric".to_string()],
        game_versions: vec!["1.21.1".to_string()],
        ..Default::default()
    }
}

/// 收藏 → 分组 → 移动 → 清空分组，每一步都对得上磁盘上的文件
#[test]
fn collect_lifecycle() {
    let dir = run_dir();
    setup(&dir);

    // ---------- 空库 ----------
    collect_utils::load().unwrap();
    assert!(collect_utils::get().items.is_empty(), "首次加载应为空");

    // ---------- 收藏两条，重复收藏不新增 ----------
    let sodium = collect_utils::add_item(item("sodium", "Sodium")).expect("首次收藏应成功");
    let iris = collect_utils::add_item(item("iris", "Iris")).expect("首次收藏应成功");
    assert!(
        collect_utils::add_item(item("sodium", "Sodium")).is_none(),
        "同一下载源 + 项目 ID 不应重复收藏"
    );
    assert_eq!(collect_utils::get().items.len(), 2);
    assert!(
        collect_utils::get().items[&sodium].fav_time > 0,
        "收藏时间应由模块落上"
    );

    // ---------- 分组：新建、放入、移动语义 ----------
    collect_utils::add_group("优化");
    collect_utils::add_group("光影");
    collect_utils::add_to_group("优化", &sodium);
    collect_utils::add_to_group("光影", &sodium);
    collect_utils::add_to_group("光影", &iris);

    let groups = collect_utils::get().groups;
    assert_eq!(groups["优化"].len(), 0, "一条只能属于一个具名分组");
    assert_eq!(groups["光影"].len(), 2);

    // ---------- 放进不存在的分组：什么都不做 ----------
    collect_utils::set_group_items("不存在", &[sodium.clone()]);
    assert!(!collect_utils::get().groups.contains_key("不存在"));

    // ---------- 清空分组：条目保留 ----------
    collect_utils::clear_group("光影");
    let obj = collect_utils::get();
    assert_eq!(obj.items.len(), 2, "清空分组不应删条目");
    assert_eq!(obj.groups["光影"].len(), 0);

    // ---------- 只清空一个分组：另一个组不受影响 ----------
    collect_utils::add_to_group("优化", &iris);
    collect_utils::add_to_group("光影", &sodium);
    collect_utils::clear_group("光影");
    let obj = collect_utils::get();
    assert_eq!(obj.groups["优化"].len(), 1);
    assert_eq!(obj.groups["光影"].len(), 0);

    // ---------- 等异步保存落盘，核对文件 ----------
    mml_config::config_save::stop();
    let file = dir.join(mml_names::names::COLLECT_FILE);
    assert!(file.exists(), "应写出收藏文件：{}", file.display());

    let saved: CollectObj =
        mml_base::serialize_tools::json_from_file(&file).expect("收藏文件应能解析");
    assert_eq!(saved.items.len(), 2);
    assert_eq!(saved.groups.len(), 2);

    // ---------- 重新加载：内存状态与文件一致 ----------
    collect_utils::load().unwrap();
    let reloaded = collect_utils::get();
    assert_eq!(reloaded.items.len(), 2);
    assert!(reloaded.groups["优化"].contains(&iris), "分组关系应读得回来");

    // ---------- 删分组：条目保留 ----------
    collect_utils::remove_group("优化");
    let obj = collect_utils::get();
    assert_eq!(obj.items.len(), 2);
    assert!(!obj.groups.contains_key("优化"));

    let _ = fs::remove_dir_all(&dir);
}
