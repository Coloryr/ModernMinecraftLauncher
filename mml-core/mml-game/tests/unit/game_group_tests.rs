use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::*;

/// 文件四列：分组顺序 / 组名 / 归属 / 组内次序，都要能原样往返
///
/// 这几种顺序都只由本表的字段表达，丢哪一列都会让对应的那一种变成随机 ——
/// 而这是纯序列化行为，不用碰全局状态就能测。
#[test]
fn store_round_trip_keeps_all_columns() {
    let pack = Uuid::new_v4();
    let test = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let store = GroupStore {
        group_order: vec![pack.to_string(), test.to_string()],
        names: HashMap::from([
            (DEFAULT_GROUP_UUID.to_string(), String::new()),
            (pack.to_string(), "整合包".to_string()),
            (test.to_string(), "测试".to_string()),
        ]),
        groups: HashMap::from([
            (DEFAULT_GROUP_UUID.to_string(), vec![a, b]),
            (pack.to_string(), Vec::new()),
            (test.to_string(), Vec::new()),
        ]),
        order: HashMap::from([(a.to_string(), 0), (b.to_string(), 1)]),
    };

    let text = serde_json::to_string(&store).unwrap();
    let back: GroupStore = serde_json::from_str(&text).unwrap();

    assert_eq!(back.group_order, store.group_order, "分组顺序必须原样回来");
    assert_eq!(back.names, store.names, "组名必须原样回来");
    assert_eq!(back.groups, store.groups);
    assert_eq!(back.order, store.order, "组内次序必须原样回来");
}

/// 归一化：补默认分组、清悬空项、把组内次序压实成 0..n-1
#[test]
fn normalize_repairs_inconsistent_file() {
    let pack = Uuid::new_v4();
    let test = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let dead = Uuid::new_v4();

    let mut store = GroupStore {
        // "幽灵组"没有名字，只在 groups 里：视为损坏，连成员一起丢掉
        // "整合包"在顺序表里重复一次；"测试"存在但漏在顺序外
        group_order: vec!["幽灵组".to_string(), pack.to_string(), pack.to_string()],
        names: HashMap::from([
            (pack.to_string(), "整合包".to_string()),
            (test.to_string(), "测试".to_string()),
        ]),
        // b 有次序，a 缺次序且应排到 b 后面，dead 只挂在"幽灵组"上
        order: HashMap::from([(b.to_string(), 0), (dead.to_string(), 7)]),
        groups: HashMap::from([
            (pack.to_string(), vec![a, b]),
            (test.to_string(), Vec::new()),
            ("幽灵组".to_string(), vec![dead]),
        ]),
    };

    store.normalize();

    let keys = store.keys();
    assert_eq!(
        keys[0], DEFAULT_GROUP_UUID,
        "顺序表里没记默认分组时，补在最前（初始位置）"
    );
    assert_eq!(keys[1], pack);
    assert_eq!(keys[2], test);
    assert_eq!(
        store.groups[&pack.to_string()],
        vec![b, a],
        "组内数组应排成次序"
    );
    assert_eq!(store.order.get(&b.to_string()), Some(&0));
    assert_eq!(
        store.order.get(&a.to_string()),
        Some(&1),
        "缺次序的排到最后"
    );
    assert!(!store.groups.contains_key("幽灵组"), "没名字的分组应清掉");
    assert!(
        !store.order.contains_key(&dead.to_string()),
        "悬空次序应清掉"
    );
}

/// 重排：传入的顺序生效，没提到的组保持原相对顺序排在后面
///
/// 默认分组不在传入列表里时仍排在最前，即它的默认初始位置；
/// 一旦用户显式把它排到别处，就照用户的来
#[test]
fn reorder_groups_applies_order() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    let mut store = GroupStore::default();
    for (uuid, name) in [(a, "a"), (b, "b"), (c, "c")] {
        store.names.insert(uuid.to_string(), name.to_string());
        store.groups.insert(uuid.to_string(), Vec::new());
        store.group_order.push(uuid.to_string());
    }

    // 默认分组不在 store.group_order 里，老数据形态 → 补到最前
    store.reorder_groups(&[c, Uuid::new_v4(), a]);
    assert_eq!(store.keys(), vec![DEFAULT_GROUP_UUID, c, a, b]);

    // 一旦用户把它排到别处，就照用户的来；本次改动之前它会一直被钉在首位
    store.reorder_groups(&[c, DEFAULT_GROUP_UUID, a]);
    assert_eq!(store.keys(), vec![c, DEFAULT_GROUP_UUID, a, b]);
}

/// 新实例必须追加在目标组末尾，不能插到已有成员前面
#[test]
fn new_instance_appends_after_existing() {
    let mut store = GroupStore::default();
    store.normalize();

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    store.move_items(&[first], DEFAULT_GROUP_UUID);
    store.move_items(&[second], DEFAULT_GROUP_UUID);

    assert_eq!(store.position_of(&first), Some(0));
    assert_eq!(store.position_of(&second), Some(1));
    assert!(store.rank(second) > store.rank(first));
}

/// 跨组移动必须从原组摘掉，并在目标组末尾拿到次序
#[test]
fn move_items_moves_out_of_the_old_group() {
    let uuid = Uuid::new_v4();

    let mut store = GroupStore::default();
    store.normalize();
    // 先把它放进默认分组，再移到一个新组
    store
        .groups
        .get_mut(&DEFAULT_GROUP_UUID.to_string())
        .unwrap()
        .push(uuid);
    store.order.insert(uuid.to_string(), 0);
    let other = store.new_group("b").expect("建组失败");

    store.move_items(&[uuid], other);

    assert!(
        store.groups[&DEFAULT_GROUP_UUID.to_string()].is_empty(),
        "原分组必须摘干净"
    );
    assert_eq!(store.groups[&other.to_string()], vec![uuid]);
    assert_eq!(store.group_of(&uuid), other);
    assert_eq!(store.position_of(&uuid), Some(0));
}

/// 手改过文件造成有名字但少了成员数组的分组也要能收人
///
/// 只摘不放会让实例从界面上直接消失，所以落点必须能自己补出来。
#[test]
fn move_items_fills_missing_member_slot() {
    let pack = Uuid::new_v4();
    let uuid = Uuid::new_v4();

    let mut store = GroupStore::default();
    store.normalize();
    // 造出"有名字、没有成员数组"的组，实例先待在默认分组
    store.names.insert(pack.to_string(), "整合包".to_string());
    store.group_order.push(pack.to_string());
    store
        .groups
        .get_mut(&DEFAULT_GROUP_UUID.to_string())
        .unwrap()
        .push(uuid);

    store.move_items(&[uuid], pack);

    assert_eq!(store.members(pack), vec![uuid], "实例必须真的进了目标组");
    assert!(store.groups[&DEFAULT_GROUP_UUID.to_string()].is_empty());
    assert_eq!(store.position_of(&uuid), Some(0));
}

/// 旧版分组并入：新分组追加在已有分组之后，成员按旧文件顺序进组
#[test]
fn merge_legacy_appends_groups_and_keeps_member_order() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    let mut store = GroupStore::default();
    store.normalize();
    store.move_items(&[a, b, c], DEFAULT_GROUP_UUID);

    let legacy = LegacyGroups {
        groups: BTreeMap::from([(
            "旧组".to_string(),
            LegacyGroup {
                instances: vec!["b".to_string(), "c".to_string(), "已删除".to_string()],
            },
        )]),
    };
    let lookup = LegacyLookup::new(&[
        LegacyInstance {
            dir: "a".to_string(),
            name: String::from("A"),
            uuid: a,
        },
        LegacyInstance {
            dir: "b".to_string(),
            name: String::from("B"),
            uuid: b,
        },
        LegacyInstance {
            dir: "c".to_string(),
            name: String::from("C"),
            uuid: c,
        },
    ]);

    let result = store.merge_legacy(&legacy, &lookup);
    let group = store.keys()[1];

    assert_eq!(result.groups, 1);
    assert_eq!(result.instances, 2, "认不出目录名的条目应忽略");
    assert_eq!(
        store.keys(),
        vec![DEFAULT_GROUP_UUID, group],
        "旧分组追加在已有分组之后"
    );
    assert_eq!(store.members(group), vec![b, c], "组内顺序按旧文件");
    assert_eq!(
        store.members(DEFAULT_GROUP_UUID),
        vec![a],
        "移走的实例要从默认分组摘掉"
    );
}

/// 旧版分组与已有分组同名时复用，不建第二个同名组
#[test]
fn merge_legacy_reuses_group_with_same_name() {
    let uuid = Uuid::new_v4();

    let mut store = GroupStore::default();
    store.normalize();
    let group = store.new_group("整合包").expect("建组失败");
    store.move_items(&[uuid], DEFAULT_GROUP_UUID);

    let legacy = LegacyGroups {
        groups: BTreeMap::from([(
            String::from("整合包"),
            LegacyGroup {
                instances: vec![String::from("dir")],
            },
        )]),
    };
    let lookup = LegacyLookup::new(&[LegacyInstance {
        dir: String::from("dir"),
        name: String::new(),
        uuid,
    }]);

    let result = store.merge_legacy(&legacy, &lookup);

    assert_eq!(result.groups, 0, "同名分组应复用");
    assert_eq!(result.instances, 1);
    assert_eq!(store.keys(), vec![DEFAULT_GROUP_UUID, group]);
    assert_eq!(store.members(group), vec![uuid]);
    assert!(store.members(DEFAULT_GROUP_UUID).is_empty());
}

/// 落盘 / 读盘类用例的**串行锁**
///
/// `STORE` 与 `GROUP_FILE` 是进程级单例，而 cargo 默认并行跑用例 ——
/// 两个用例各 `init` 一次同一个目录，会互相把对方的分组表覆盖掉
/// 表现为随机失败，单独跑就过。凡是碰磁盘状态的用例都先拿这把锁。
static DISK_LOCK: Mutex<()> = Mutex::new(());

/// 建地基：起内核那几个进程级单例，见 [`crate::test_support::boot`]
///
/// **不要在这里自己再写一份**：`mml_log::STREAM` 是 `OnceLock`，第二次
/// `mml_log::start()` 会 panic，即 `STREAM.set(..).unwrap()` ——
/// 这正是本模块与 `game_mods::meta_tests` 曾经互相踩挂的原因。
fn boot() -> std::path::PathBuf {
    crate::test_support::boot()
}

/// 落盘 → 读回：分组顺序与组内次序都必须真的写进文件、重启后还在
///
/// 覆盖"拖完看着对了、重启就乱"的完整链路 —— 只断言内存里的顺序是抓不到这类问题的。
#[test]
fn orders_survive_save_and_reload() {
    let _guard = DISK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    boot();

    let dir = mml_testutil::temp_dir().join(format!("mml-group-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    init(dir.clone()).unwrap();
    let first = add_group("甲组").expect("建组失败");
    let second = add_group("乙组").expect("建组失败");
    // 乙组拖到默认分组之后，非默认组的第一位。
    // 顺序表**含默认分组**：把它一起传进去，它才不会被甩到末尾
    reorder_groups(&[DEFAULT_GROUP_UUID, second, first]);

    // 两个实例进乙组，再把 a 挪到 b 前面
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    move_group(vec![b, a], Some(second));
    place_in_group(&a, Some(second), 0);

    // 保存走异步线程，而且要等**内容**到位：文件可能先被前一次 save 建出来
    let file = dir.join(names::GROUP_FILE);
    let expect_order = vec![
        DEFAULT_GROUP_UUID.to_string(),
        second.to_string(),
        first.to_string(),
    ];
    let saved = || {
        serialize_tools::json_from_file::<GroupStore>(&file).is_ok_and(|back| {
            back.group_order == expect_order
                && back.names.get(&second.to_string()) == Some(&"乙组".to_string())
                && back.order.get(&a.to_string()) == Some(&0)
                && back.order.get(&b.to_string()) == Some(&1)
        })
    };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) && !saved() {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(saved(), "分组表未在限时内落盘: {}", file.display());

    // 模拟重启：重新 init，只从文件读回来
    init(dir).unwrap();
    let list = group_list();
    assert_eq!(
        list.iter().map(|g| g.uuid).collect::<Vec<_>>(),
        vec![DEFAULT_GROUP_UUID, second, first],
        "顺序应原样保留（默认分组第一，但它是被**记下来**的位置，不是被钉住的）"
    );
    assert_eq!(list[1].name, "乙组");
    assert_eq!(group_items(&second), vec![a, b], "组内次序应保持 a、b");
    assert_eq!(index_of(&a), Some(0));
    assert_eq!(index_of(&b), Some(1));
    assert_eq!(group_of(&a), second);
}

/// 默认分组只是**初始**排首位：拖到后面应生效，且重启后保持
///
/// 这是"默认分组拖不动"那个问题的回归测试 —— 它原先被写死在首位
/// 即 `keys()` 里无条件 push、`reorder_groups` 里跳过它。
///
/// **用自己的目录**，不复用上面那个用例的：落盘是异步的，两个用例共用一个目录时，
/// 先跑完的那个可能在另一个 `remove_dir_all` 之后才把文件写出来，
/// 于是后者的 `init` 读到了前者的数据，表现为随机失败、单独跑就过。
/// `DISK_LOCK` 已把两个用例串起来，各用各的目录就互不干扰了。
#[test]
fn default_group_can_be_reordered() {
    let _guard = DISK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    boot();

    let dir =
        mml_testutil::temp_dir().join(format!("mml-group-reorder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    init(dir.clone()).unwrap();

    let custom = add_group("甲组").expect("建组失败");

    // 把默认分组拖到甲组后面
    reorder_groups(&[custom, DEFAULT_GROUP_UUID]);
    assert_eq!(
        group_list().iter().map(|g| g.uuid).collect::<Vec<_>>(),
        vec![custom, DEFAULT_GROUP_UUID],
        "默认分组应能排到后面"
    );

    // 落盘是异步的：等文件里真的出现新顺序再重新 init，否则读到的还是旧的一份
    let file = dir.join(names::GROUP_FILE);
    let want = vec![custom.to_string(), DEFAULT_GROUP_UUID.to_string()];
    let saved = || {
        serialize_tools::json_from_file::<GroupStore>(&file)
            .is_ok_and(|back| back.group_order == want)
    };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) && !saved() {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(saved(), "分组表未在限时内落盘: {}", file.display());

    // 模拟重启：位置要能从文件读回来，说明存的是"用户排的位置"而不是每次重算
    init(dir).unwrap();
    assert_eq!(
        group_list().iter().map(|g| g.uuid).collect::<Vec<_>>(),
        vec![custom, DEFAULT_GROUP_UUID],
        "重启后仍应保持用户排的位置"
    );
}

/// 旧版分组文件在启动时并入分组表并删除，重启后分组仍在
#[test]
fn import_legacy_merges_and_removes_file() {
    let _guard = DISK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    boot();

    let dir = mml_testutil::temp_dir().join(format!("mml-group-legacy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let uuid = Uuid::new_v4();
    // 真实旧文件带 formatVersion 与 hidden，顺带验证多余字段会被忽略
    std::fs::write(
        dir.join(names::LEGACY_GROUP_FILE),
        r#"{"formatVersion":"1","groups":{"整合包":{"hidden":true,"instances":["old-dir"]}}}"#,
    )
    .unwrap();

    init(dir.clone()).unwrap();
    import_legacy(
        &dir,
        &[LegacyInstance {
            dir: String::from("old-dir"),
            name: String::from("旧实例"),
            uuid,
        }],
    );

    assert!(
        !dir.join(names::LEGACY_GROUP_FILE).exists(),
        "旧文件应被删除"
    );
    let list = group_list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].name, "整合包");
    assert_eq!(group_items(&list[1].uuid), vec![uuid]);
    assert_eq!(group_of(&uuid), list[1].uuid);

    // 重启：并进来的分组应已落盘，导入走的是同步写，不等异步队列
    init(dir.clone()).unwrap();
    let list = group_list();
    assert_eq!(list[1].name, "整合包");
    assert_eq!(group_items(&list[1].uuid), vec![uuid]);
}

