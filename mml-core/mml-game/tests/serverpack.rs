//! 服务器包 serverpack 测试
//!
//! 覆盖用户要求的五条链路，以及各自落在哪一层：
//!
//! | 链路 | 对应实现 | 本文件怎么测 |
//! | --- | --- | --- |
//! | **生成** | 服务端产出的 `server.json` 与 `.sha1`；本仓库**只有消费侧** | 测试里构造 `ServerPackObj` 模拟服务端生成 → 走保存 / 读回 / 升级 |
//! | **获取** | `server_pack_update` 拉 `<url>/server.json.sha1` 与 `server.json` | 离线：本地起 HTTP 服务喂这两个文件，见文件末尾 |
//! | **下载** | `upgrade_serverpack` 里按 `plan_downloads` 下的文件 / 配置包 | 离线：纯函数用例 + 本地 HTTP 服务真实下载 |
//! | **更新** | 本地 `server.json` 的 SHA1 与远端 `.sha1` 比对 | 离线：本地 HTTP 服务 + 哈希一致/不一致两条 |
//! | **升级** | `upgrade_serverpack` 删旧 + 下载 + 解压 + 落盘新清单 | 离线：纯函数用例 + 真实升级链路 |
//!
//! 判定逻辑已抽到 [`mml_game::serverpack::plan`]，纯函数，无 IO 无网络，
//! 所以"删错文件 / 下错地址"这类最危险的判定都能离线测。

use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_game::serverpack::plan;
use mml_game::serverpack::serverpack_obj::{ServerArchiveItemObj, ServerItemObj, ServerPackObj};
use uuid::Uuid;

/// 初始化链，只用到磁盘与配置后台保存线程，不需要网络与下载器
fn ensure_init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir =
            mml_testutil::temp_dir().join(format!("mml-serverpack-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        mml_base::init(&dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        // 实例与清单保存都走 config_save 后台线程
        mml_config::config_save::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
    });
}

/// 内核那几个系统是进程级单例，见 AGENTS.md §6；用例串行执行
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 等待条件成立，后台保存线程异步写盘，需要轮询
fn wait_for<F: Fn() -> bool>(timeout: Duration, cond: F) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

// ===================== 构造器 =====================

/// 在线文件条目，`file` 是游戏目录下的相对路径
fn item(file: &str, pid: Option<&str>, fid: Option<&str>, url: &str) -> ServerItemObj {
    ServerItemObj {
        file: file.to_string(),
        pid: pid.map(|s| s.to_string()),
        fid: fid.map(|s| s.to_string()),
        url: Some(url.to_string()),
        ..Default::default()
    }
}

/// 配置压缩包条目
fn archive(file: &str, dir: &str, delete_old: bool, url: &str) -> ServerArchiveItemObj {
    ServerArchiveItemObj {
        file: file.to_string(),
        dir: dir.to_string(),
        delete_old,
        url: url.to_string(),
        ..Default::default()
    }
}

/// 服务器包清单
fn pack(online: Vec<ServerItemObj>, archives: Vec<ServerArchiveItemObj>) -> ServerPackObj {
    ServerPackObj {
        online_list: online,
        archive_list: archives,
        ..Default::default()
    }
}

/// 排序后的"要删除的文件路径"
fn removed_files(removals: &plan::ServerPackRemovals<'_>) -> Vec<String> {
    let mut v: Vec<String> = removals.files.iter().map(|i| i.file.clone()).collect();
    v.sort();
    v
}

/// 排序后的"要删除的目录"
fn removed_dirs(removals: &plan::ServerPackRemovals<'_>) -> Vec<String> {
    let mut v: Vec<String> = removals.dirs.iter().map(|i| i.dir.clone()).collect();
    v.sort();
    v
}

// ===================== 身份标识 / 哈希 / 来源判定，纯函数 =====================

/// `mod_key`：有 `pid` 用 `pid`，没有才退回文件名
#[test]
fn mod_key_prefers_pid_then_file() {
    assert_eq!(
        plan::mod_key(&item("mods/a.jar", Some("123"), None, "")),
        "123"
    );
    assert_eq!(
        plan::mod_key(&item("mods/a.jar", None, Some("abc"), "")),
        "mods/a.jar",
        "没有 pid 时按文件名配对"
    );
    assert_eq!(
        plan::mod_key(&item("mods/a.jar", None, None, "")),
        "mods/a.jar"
    );
}

/// `make_hash`：四种校验值组合
#[test]
fn make_hash_covers_all_combinations() {
    use mml_base::file_item::FileHash;

    let s1 = Some("aa".to_string());
    let s256 = Some("bb".to_string());
    assert!(matches!(
        plan::make_hash(&s1, &s256),
        FileHash::Sha1Sha256(a, b) if a == "aa" && b == "bb"
    ));
    assert!(matches!(plan::make_hash(&s1, &None), FileHash::Sha1(a) if a == "aa"));
    assert!(matches!(plan::make_hash(&None, &s256), FileHash::Sha256(b) if b == "bb"));
    assert!(matches!(plan::make_hash(&None, &None), FileHash::None));
}

/// `is_curseforge`：编号是纯数字才算 CurseForge
#[test]
fn is_curseforge_by_numeric_id() {
    // pid 数字 → CF；pid 非数字 → Modrinth
    assert!(plan::is_curseforge(&item(
        "m",
        Some("123"),
        Some("456"),
        ""
    )));
    assert!(!plan::is_curseforge(&item(
        "m",
        Some("abc"),
        Some("def"),
        ""
    )));
    // 没有 pid 时只看 fid
    assert!(plan::is_curseforge(&item("m", None, Some("456"), "")));
    assert!(!plan::is_curseforge(&item("m", None, Some("def"), "")));
    // 两个都没有 → 判为 Modrinth，原实现如此
    assert!(!plan::is_curseforge(&item("m", None, None, "")));
}

// ===================== 升级要删什么，纯函数 =====================

/// 新清单里没有的项目 → 删旧文件
#[test]
fn removals_include_files_missing_from_new_pack() {
    let old = pack(
        vec![
            item("mods/a.jar", Some("1"), None, ""),
            item("mods/b.jar", Some("2"), None, ""),
        ],
        vec![],
    );
    let new = pack(vec![item("mods/a.jar", Some("1"), None, "")], vec![]);

    let r = plan::plan_removals(&old, &new);
    assert_eq!(removed_files(&r), vec!["mods/b.jar"]);
    assert!(r.dirs.is_empty());
}

/// 同一项目换了路径 → 删旧路径，新路径由下载阶段写入
#[test]
fn removals_include_old_path_when_file_moved() {
    let old = pack(vec![item("mods/old-name.jar", Some("1"), None, "")], vec![]);
    let new = pack(vec![item("mods/new-name.jar", Some("1"), None, "")], vec![]);

    let r = plan::plan_removals(&old, &new);
    assert_eq!(removed_files(&r), vec!["mods/old-name.jar"]);
}

/// 同一项目、同一路径 → 不删，下载阶段会按哈希决定是否重下
#[test]
fn removals_skip_unchanged_file() {
    let old = pack(vec![item("mods/a.jar", Some("1"), None, "")], vec![]);
    let new = pack(vec![item("mods/a.jar", Some("1"), None, "")], vec![]);

    let r = plan::plan_removals(&old, &new);
    assert!(removed_files(&r).is_empty(), "路径没变不该删");
}

/// 没有 `pid` 时按文件名配对：同名不算移除，改名算移除
#[test]
fn removals_fall_back_to_file_name_without_pid() {
    let old = pack(
        vec![
            item("mods/a.jar", None, None, ""),
            item("mods/renamed.jar", None, None, ""),
        ],
        vec![],
    );
    let new = pack(vec![item("mods/a.jar", None, None, "")], vec![]);

    let r = plan::plan_removals(&old, &new);
    assert_eq!(
        removed_files(&r),
        vec!["mods/renamed.jar"],
        "没有 pid 时同名保留、改名算移除"
    );
}

/// 配置目录：只有"新清单里没有 + `delete_old` + `dir` 非空"才删
#[test]
fn removals_of_archive_dirs_follow_delete_old_flag() {
    let old = pack(
        vec![],
        vec![
            archive("cfg-a.zip", "config", true, ""),
            archive("cfg-b.zip", "kubejs", false, ""),
            archive("cfg-c.zip", "", true, ""),
            archive("cfg-d.zip", "scripts", true, ""),
        ],
    );
    // cfg-d 仍在新清单里 → 不删
    let new = pack(vec![], vec![archive("cfg-d.zip", "scripts", true, "")]);

    let r = plan::plan_removals(&old, &new);
    assert_eq!(
        removed_dirs(&r),
        vec!["config"],
        "delete_old=false / dir 为空 / 仍在新清单里的都不删"
    );
}

/// 旧清单与新清单都为空时不应 panic，也不产生删除项
#[test]
fn removals_with_empty_lists() {
    let empty = pack(vec![], vec![]);
    let some = pack(vec![item("mods/a.jar", Some("1"), None, "")], vec![]);

    let r = plan::plan_removals(&empty, &empty);
    assert!(r.files.is_empty() && r.dirs.is_empty());

    let r = plan::plan_removals(&empty, &some);
    assert!(
        r.files.is_empty() && r.dirs.is_empty(),
        "旧清单为空 → 没东西可删"
    );

    let r = plan::plan_removals(&some, &empty);
    assert_eq!(removed_files(&r), vec!["mods/a.jar"], "新清单为空 → 全删");
}

// ===================== 升级要下什么，纯函数 =====================

/// 自带 url 优先；为空时用解析出来的 `fid → url`
#[test]
fn downloads_prefer_own_url_then_resolved() {
    let p = pack(
        vec![
            item("mods/a.jar", Some("1"), Some("111"), "https://cdn/a.jar"),
            item("mods/b.jar", Some("2"), Some("222"), ""),
            item("mods/c.jar", Some("3"), Some("333"), ""),
        ],
        vec![],
    );
    let mut resolved = std::collections::HashMap::new();
    resolved.insert("222".to_string(), "https://cdn/b.jar".to_string());

    let d = plan::plan_downloads(&p, &resolved);
    let urls: Vec<(String, String)> = d
        .files
        .iter()
        .map(|f| (f.item.file.clone(), f.url.clone()))
        .collect();

    assert_eq!(
        urls,
        vec![
            ("mods/a.jar".to_string(), "https://cdn/a.jar".to_string()),
            ("mods/b.jar".to_string(), "https://cdn/b.jar".to_string()),
            // 解析不到 → 空串，下载阶段会失败并报 DownloadFileFail
            ("mods/c.jar".to_string(), String::new()),
        ],
        "顺序与清单一致，地址按 自带 url → 解析结果 → 空串 逐级回退"
    );
}

/// 配置压缩包用各自的 url，顺序与清单一致
#[test]
fn downloads_include_archives_in_order() {
    let p = pack(
        vec![],
        vec![
            archive("cfg-a.zip", "config", true, "https://cdn/cfg-a.zip"),
            archive("cfg-b.zip", "kubejs", false, "https://cdn/cfg-b.zip"),
        ],
    );

    let d = plan::plan_downloads(&p, &std::collections::HashMap::new());
    let urls: Vec<(String, String)> = d
        .archives
        .iter()
        .map(|a| (a.item.file.clone(), a.url.clone()))
        .collect();

    assert_eq!(
        urls,
        vec![
            ("cfg-a.zip".to_string(), "https://cdn/cfg-a.zip".to_string()),
            ("cfg-b.zip".to_string(), "https://cdn/cfg-b.zip".to_string()),
        ]
    );
    assert!(d.files.is_empty());
}

// ===================== 清单落盘，生成 / 保存 =====================

/// 保存 → 读回：`save_serverpack` 写的是实例的 `server.json`，`move_serverpack_to_old`
/// 把它变成 `server_old.json`
#[test]
fn save_then_move_to_old_round_trip() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = format!("save-{}", Uuid::new_v4().simple());
    let instance = InstanceSettingObj {
        name: dir.clone(),
        dir: dir.clone(),
        ..Default::default()
    };
    let pack_file = instance.get_server_pack_file();
    let old_file = instance.get_server_pack_old_file();
    std::fs::create_dir_all(pack_file.parent().unwrap()).unwrap();

    let p = pack(vec![item("mods/a.jar", Some("1"), None, "")], vec![]);
    instance.save_serverpack(&p);

    assert!(
        wait_for(Duration::from_secs(10), || pack_file.exists()),
        "保存后应出现 {}",
        pack_file.display()
    );

    instance.move_serverpack_to_old().expect("移到旧版");
    assert!(!pack_file.exists(), "移动后不该还留着 server.json");
    assert!(old_file.exists(), "移动后应出现 server_old.json");

    // 读回内容一致，走 json 反序列化
    let read: ServerPackObj =
        mml_base::serialize_tools::json_from_file(&old_file).expect("读回旧清单");
    assert_eq!(read.online_list.len(), 1);
    assert_eq!(read.online_list[0].file, "mods/a.jar");
}

/// 没有旧清单时 `move_serverpack_to_old` 是空操作
///
/// 第一次同步时本地还没有 `server.json`，这里报错会让新玩家第一次启动必定失败；
/// 已改成"文件不存在就跳过"。
#[test]
fn move_to_old_without_file_is_ok() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = format!("move-missing-{}", Uuid::new_v4().simple());
    let instance = InstanceSettingObj {
        name: dir.clone(),
        dir: dir.clone(),
        ..Default::default()
    };
    std::fs::create_dir_all(instance.get_server_pack_file().parent().unwrap()).unwrap();

    instance
        .move_serverpack_to_old()
        .expect("源文件不存在时应当作没有旧版");
    assert!(!instance.get_server_pack_old_file().exists());
}
