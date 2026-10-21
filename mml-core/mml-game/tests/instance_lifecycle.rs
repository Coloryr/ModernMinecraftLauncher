//! 实例生命周期集成测试，离线运行：创建、重名自动改名、改名、分组、删除。
//!
//! 不依赖网络与版本下载：版本号只是字符串标记，不真正启动游戏。
//! 全局初始化每进程一次，互斥锁串行各用例。

use std::path::PathBuf;
use std::sync::{Mutex, Once};

use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use uuid::Uuid;

/// 测试运行目录：临时根目录加进程号，避免多进程冲突
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join(format!("mml-instance-lifecycle-{}", std::process::id()))
}

/// 初始化链，与 mml_core::init 相同；每进程只跑一次
fn ensure_init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = run_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_config::config_save::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
    });
}

/// 用例互斥锁：实例表是进程级共享，各用例必须串行
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 构造实例设置，只填名称与版本号
///
/// - `name`: 实例名，调用方保证带唯一后缀
///
/// 分组不在实例配置里，已迁到内核的 `group_save.json`，
/// 要指定分组得走 `create_instance_in_group`。
///
/// # 返回值
///
/// 可直接交给创建接口的实例配置
fn setting(name: &str) -> InstanceSettingObj {
    InstanceSettingObj {
        name: name.to_string(),
        version: "1.20.1".to_string(),
        ..Default::default()
    }
}

/// 创建：目录结构与全局表登记
#[tokio::test]
async fn create_instance_makes_dirs() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let suffix = Uuid::new_v4().simple().to_string();
    let name = format!("生命周期-{suffix}");
    let group = mml_game::add_group("测试组").expect("建组失败");
    let game = setting(&name)
        .create_instance_in_group(None, Some(group))
        .await
        .expect("创建实例失败");
    let uuid = game.read().unwrap().uuid;

    // 全局表登记
    assert!(mml_game::have_instance_name(&name));
    assert!(mml_game::get_instance_by_name(&name).is_some());
    // 分组归属在独立的分组表里，实例配置上没有这个字段，按 uuid 记录
    assert_eq!(mml_game::get_instance_group(&uuid), Some(group));
    assert!(
        mml_game::get_group_list().iter().any(|g| g.uuid == group),
        "分组表里应当有这个组"
    );

    // 目录结构齐全
    let game2 = mml_game::get_instance(&uuid).unwrap();
    let read = game2.read().unwrap();
    assert!(read.get_base_path().exists());
    assert!(read.get_game_path().exists());
    assert!(read.get_mods_path().exists());
    assert!(read.get_config_path().exists());
    assert!(read.get_logs_path().exists());
    assert!(read.get_saves_path().exists());
    assert!(read.get_resourcepacks_path().exists());
    assert_eq!(read.version, "1.20.1");
    drop(read);

    // 清理
    mml_game::delete_instance(&uuid).unwrap();
    assert!(mml_game::get_instance(&uuid).is_none());
    assert!(!mml_game::have_instance_name(&name));
}

/// 重名创建时自动改名 `{name}1`，两个实例共存
#[tokio::test]
async fn create_duplicate_name_auto_renames() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let suffix = Uuid::new_v4().simple().to_string();
    let name = format!("重名-{suffix}");

    let game1 = setting(&name).create_instance(None).await.unwrap();
    let uuid1 = game1.read().unwrap().uuid;

    let game2 = setting(&name).create_instance(None).await.unwrap();
    let uuid2 = game2.read().unwrap().uuid;
    let name2 = game2.read().unwrap().name.clone();

    assert_ne!(uuid1, uuid2);
    assert_eq!(name2, format!("{name}1"), "重名实例应自动追加序号");
    assert!(mml_game::have_instance_name(&name2));

    mml_game::delete_instance(&uuid1).unwrap();
    mml_game::delete_instance(&uuid2).unwrap();
}

/// 改名：目录跟随改名；新名重复被拒绝；空名被拒绝；改回原名允许
#[tokio::test]
async fn rename_instance_rules() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let suffix = Uuid::new_v4().simple().to_string();
    let name = format!("改名-{suffix}");

    let game = setting(&name).create_instance(None).await.unwrap();
    let uuid = game.read().unwrap().uuid;

    // 改名成功，目录跟随
    let new_name = format!("改名后-{suffix}");
    mml_game::rename_instance(&uuid, &new_name).expect("改名失败");
    let read = mml_game::get_instance(&uuid).unwrap();
    let read = read.read().unwrap();
    assert_eq!(read.name, new_name);
    assert!(read.get_base_path().exists());
    assert!(read.get_base_path().file_name().unwrap().to_string_lossy() != name);
    drop(read);
    assert!(!mml_game::have_instance_name(&name));
    assert!(mml_game::have_instance_name(&new_name));

    // 改成别人占用的名字 → 拒绝
    let other = format!("改名占位-{suffix}");
    let game2 = setting(&other).create_instance(None).await.unwrap();
    let uuid2 = game2.read().unwrap().uuid;
    assert!(mml_game::rename_instance(&uuid, &other).is_err());

    // 空名 → 拒绝
    assert!(mml_game::rename_instance(&uuid, "   ").is_err());

    // 保持原名 → 允许，自己不算重名
    mml_game::rename_instance(&uuid, &new_name).expect("保持原名不应报错");

    mml_game::delete_instance(&uuid).unwrap();
    mml_game::delete_instance(&uuid2).unwrap();
}

/// 删除：从全局表与分组中移除，目录清理
#[tokio::test]
async fn delete_instance_removes_everywhere() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let suffix = Uuid::new_v4().simple().to_string();
    let group = mml_game::add_group(&format!("删除组-{suffix}")).expect("建组失败");
    let game = setting(&format!("待删-{suffix}"))
        .create_instance_in_group(None, Some(group))
        .await
        .unwrap();
    let uuid = game.read().unwrap().uuid;
    let base = game.read().unwrap().get_base_path();

    assert!(mml_game::get_group_list().iter().any(|g| g.uuid == group));

    mml_game::delete_instance(&uuid).unwrap();

    assert!(mml_game::get_instance(&uuid).is_none());
    // 实例从组里摘掉，但**空分组保留** —— 这正是分组独立存储的目的：
    // 归属挂在实例上时，删掉最后一个成员就再没有任何实例承载这个组，只能跟着消失
    assert!(
        mml_game::get_group(&group).is_empty(),
        "被删实例不应还在组里"
    );
    assert!(
        mml_game::get_group_list().iter().any(|g| g.uuid == group),
        "空分组应当保留"
    );
    assert!(!base.exists(), "实例目录应被清理（回收站）");

    // 重复删除 → 报错，uuid 已不存在
    assert!(mml_game::delete_instance(&uuid).is_err());
}

/// 分组归属与组内次序：内核一次完成"落组 + 定位"，前端拿到的 `order` 就是组内位置
#[tokio::test]
async fn move_instance_sets_group_and_order() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let suffix = Uuid::new_v4().simple().to_string();
    let group = mml_game::add_group(&format!("排序组-{suffix}")).expect("建组失败");

    let a = setting(&format!("排序A-{suffix}"))
        .create_instance(None)
        .await
        .unwrap();
    let b = setting(&format!("排序B-{suffix}"))
        .create_instance(None)
        .await
        .unwrap();
    let ua = a.read().unwrap().uuid;
    let ub = b.read().unwrap().uuid;

    // 先后移进同一个组，每次插入到第 0 位，最终顺序应为 B、A
    mml_game::move_instance(&ua, Some(group), 0);
    mml_game::move_instance(&ub, Some(group), 0);

    assert_eq!(mml_game::get_instance_group(&ua), Some(group));
    assert_eq!(
        mml_game::get_instance_order(&ub),
        Some(0),
        "B 应排在第 0 位"
    );
    assert_eq!(
        mml_game::get_instance_order(&ua),
        Some(1),
        "A 应被挤到第 1 位"
    );

    // 分组顺序可调：默认分组恒在首位，被提到的组紧跟其后
    mml_game::reorder_groups(&[group]);
    let keys: Vec<Uuid> = mml_game::get_group_list().iter().map(|g| g.uuid).collect();
    assert_eq!(
        keys.first(),
        Some(&mml_game::game_group::DEFAULT_GROUP_UUID),
        "默认分组恒在首位"
    );
    assert_eq!(keys.get(1), Some(&group), "重排后本组应紧跟默认分组");

    mml_game::delete_instance(&ua).unwrap();
    mml_game::delete_instance(&ub).unwrap();
}
