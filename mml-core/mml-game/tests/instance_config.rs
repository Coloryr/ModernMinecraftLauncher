//! 实例配置与 game_options 配置文件的磁盘读写测试。
//!
//! 需要全局初始化：实例目录 / 运行库目录 / 配置后台保存线程，
//! 全部测试共用同一个临时运行目录，通过 `OnceLock` 保证初始化只执行一次。
//! 运行目录由 mml_testutil 提供，运行前先清理旧目录。

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use mml_game::game_options::read_options_from_file;
use mml_game::launcher::instance_setting_obj::{InstanceSettingObj, ProxyHostObj, ServerObj};
use mml_game::launcher::mod_update_obj::ModUpdateRecordObj;
use mml_game::launcher::{LogEncoding, ModPackType};
use mml_game::loader::LoaderType;
use mml_game::{get_instance_by_name, init as game_init, load as game_load};
use uuid::Uuid;

/// 初始化内核各系统，返回运行目录；每次运行前先删除旧目录
///
/// # 返回值
///
/// 本次运行的临时运行目录
fn init_all() -> PathBuf {
    static INIT: OnceLock<PathBuf> = OnceLock::new();
    INIT.get_or_init(|| {
        let run_dir = mml_testutil::temp_dir().join("mml-instance-config-test");
        let _ = std::fs::remove_dir_all(&run_dir);
        std::fs::create_dir_all(&run_dir).expect("创建测试运行目录失败");

        // 与 mml_core::init 相同的初始化链
        mml_base::init(&run_dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        // 实例保存走 config_save 后台线程，需要先启动
        mml_config::config_save::start();
        game_init(mml_base::get_base_dir()).unwrap();
        run_dir
    })
    .clone()
}

/// 轮询等待条件成立，后台保存线程是异步写盘
///
/// - `timeout`: 最长等待时间
/// - `cond`: 判定条件，返回 true 即结束等待
///
/// # 返回值
///
/// 超时前成立返回 true，否则返回最后一次判断结果
fn wait_for<F: Fn() -> bool>(timeout: Duration, cond: F) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

/// 构造一个测试实例配置
///
/// - `dir`: 实例目录名
///
/// # 返回值
///
/// 填好加载器、服务器、代理等字段的配置，其余取默认值
fn make_instance(dir: &str) -> InstanceSettingObj {
    InstanceSettingObj {
        uuid: Uuid::new_v4(),
        name: format!("inst-{dir}"),
        dir: dir.to_string(),
        version: "1.20.4".to_string(),
        loader: LoaderType::Fabric,
        loader_version: Some("0.16.9".to_string()),
        start_server: Some(ServerObj {
            enable: true,
            ip: Some("mc.example.com".to_string()),
            port: Some(25565),
        }),
        proxy_host: Some(ProxyHostObj {
            ip: Some("127.0.0.1".to_string()),
            port: Some(7890),
            user: None,
            password: None,
        }),
        is_modpack: true,
        modpack_type: ModPackType::Modrinth,
        encoding: LogEncoding::GBK,
        ..Default::default()
    }
}

/// 实例配置保存到磁盘：写入 `{实例目录}/game.json`，字段为 PascalCase
#[test]
fn instance_save_to_disk() {
    init_all();

    let dir = format!("save-test-{}", Uuid::new_v4().simple());
    let instance = make_instance(&dir);
    instance.save();

    let json_file = instance.get_json_file();
    assert!(
        wait_for(Duration::from_secs(10), || json_file.exists()),
        "game.json 未在限时内落盘: {}",
        json_file.display()
    );

    let text = std::fs::read_to_string(&json_file).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();

    // 文件名与字段命名规范
    assert_eq!(
        json_file.file_name().unwrap().to_string_lossy(),
        "game.json"
    );
    assert_eq!(value["Name"], format!("inst-{dir}"));
    assert_eq!(value["Version"], "1.20.4");
    assert_eq!(value["DirName"], dir);
    assert_eq!(value["Loader"], 2); // LoaderType::Fabric
    assert_eq!(value["ModPackType"], 1); // ModPackType::Modrinth
    assert_eq!(value["Encoding"], 1); // LogEncoding::GBK
    // 分组已迁到独立的 group_save.json，实例配置里不该再出现 GroupName
    assert!(value.get("GroupName").is_none());

    // 从磁盘还原
    let restored: InstanceSettingObj = serde_json::from_value(value).unwrap();
    assert_eq!(restored.uuid, instance.uuid);
    assert_eq!(restored.loader_version.as_deref(), Some("0.16.9"));
    assert_eq!(restored.start_server.as_ref().unwrap().port, Some(25565));
    assert_eq!(
        restored.proxy_host.as_ref().unwrap().ip.as_deref(),
        Some("127.0.0.1")
    );
}

/// 实例配置写盘后能被加载回来：load 之后按名字能找到
#[test]
fn instance_load_round_trip() {
    init_all();

    let dir = format!("load-test-{}", Uuid::new_v4().simple());
    let instance = make_instance(&dir);
    instance.save();

    let json_file = instance.get_json_file();
    assert!(
        wait_for(Duration::from_secs(10), || json_file.exists()),
        "game.json 未在限时内落盘"
    );

    game_load().expect("加载实例失败");

    let loaded = get_instance_by_name(&format!("inst-{dir}")).expect("加载后应能按名字找到实例");
    let loaded = loaded.read().unwrap();
    assert_eq!(loaded.uuid, instance.uuid);
    assert_eq!(loaded.version, "1.20.4");
    assert_eq!(loaded.loader, LoaderType::Fabric);
    assert_eq!(loaded.encoding, LogEncoding::GBK);
    // 目录名与 DirName 不一致时以目录名为准，本测试两者一致
    assert_eq!(loaded.dir, dir);
}

/// game_options 往返：save_options 写盘后 read_options_from_file 读回一致
#[test]
fn game_options_save_and_read_round_trip() {
    init_all();

    let mut instance = make_instance(&format!("options-test-{}", Uuid::new_v4().simple()));
    // options.txt 是 OptiFine 配置，用 OptiFine 加载器拿到独立的配置路径
    instance.loader = LoaderType::OptiFine;
    instance.loader_version = Some(format!("9.9.{}", Uuid::new_v4().simple()));

    let options: std::collections::HashMap<String, String> = [
        ("fov".to_string(), "0.5".to_string()),
        ("renderDistance".to_string(), "12".to_string()),
        ("music".to_string(), "0.0".to_string()),
    ]
    .into_iter()
    .collect();

    instance
        .save_options(&options, None)
        .expect("写 options 失败");

    let file = instance.get_optifine_file();
    assert!(file.exists(), "options 文件应已写盘: {}", file.display());

    let data = read_options_from_file(&file, None).expect("读 options 失败");
    assert_eq!(data.len(), 3);
    for (key, value) in &options {
        assert_eq!(data.get(key), Some(value), "key {key} 值不一致");
    }

    // 实例上的读取入口 get_options 应读到同样内容
    let data = instance.get_options().unwrap();
    assert_eq!(data.get("fov").map(String::as_str), Some("0.5"));
    assert_eq!(data.get("renderDistance").map(String::as_str), Some("12"));
}

/// game_options 换行符不敏感，CRLF 与 LF 都能解析
#[test]
fn game_options_line_endings() {
    init_all();

    let mut instance = make_instance(&format!("line-test-{}", Uuid::new_v4().simple()));
    instance.loader = LoaderType::OptiFine;
    instance.loader_version = Some(format!("9.8.{}", Uuid::new_v4().simple()));

    // Windows 写 CRLF，Unix 写 LF
    let mut options: std::collections::HashMap<String, String> =
        [("fov".to_string(), "1.0".to_string())]
            .into_iter()
            .collect();
    instance.save_options(&options, None).unwrap();

    // 手工用另一种换行符写盘再读，键值分隔符仍是 :
    let file = instance.get_optifine_file();
    let sep = if cfg!(windows) { "\n" } else { "\r\n" };
    std::fs::write(&file, format!("gamma:0.8{sep}fov:1.0{sep}")).unwrap();

    let data = read_options_from_file(&file, None).unwrap();
    assert_eq!(data.get("gamma").map(String::as_str), Some("0.8"));
    assert_eq!(data.get("fov").map(String::as_str), Some("1.0"));

    // 自定义分隔符写入与读回
    options.insert("lang".to_string(), "zh_cn".to_string());
    instance.save_options(&options, Some('=')).unwrap();
    let data = read_options_from_file(&file, Some('=')).unwrap();
    assert_eq!(data.get("lang").map(String::as_str), Some("zh_cn"));
    assert_eq!(data.get("fov").map(String::as_str), Some("1.0"));
}

/// game_options：配置文件不存在时 get_options 返回空表
#[test]
fn game_options_missing_file_returns_empty() {
    init_all();

    // 用唯一的版本号保证对应的 OptiFine 配置文件一定不存在
    let instance = InstanceSettingObj {
        version: format!("9.9.{}", Uuid::new_v4().simple()),
        loader: LoaderType::OptiFine,
        loader_version: Some(format!("opt-{}", Uuid::new_v4().simple())),
        ..Default::default()
    };

    // 不预先写盘，get_options 应返回空表
    let data = instance.get_options().unwrap();
    assert!(data.is_empty());
}

/// 模组更新记录：追加两条，读回来两条且保持追加顺序
///
/// 关键是**追加不能冲掉前一条**：这条链路是"读旧表 + 追加 + 写回"，
/// 所以它走同步写盘而不是 config_save 队列。
#[test]
fn mod_update_appends_and_reads_back() {
    init_all();

    let dir = format!("mod-update-{}", Uuid::new_v4().simple());
    let instance = make_instance(&dir);
    // 记录落在实例目录里，目录得先存在（真实流程里实例建出来时就有了）
    std::fs::create_dir_all(instance.get_base_path()).unwrap();

    // 文件还不存在时视为空表，不是报错
    assert!(instance.read_mod_update().is_empty());

    instance
        .append_mod_update(
            "5123456",
            "Sodium",
            ModUpdateRecordObj {
                file: "sodium-0.6.jar".to_string(),
                old_version: "0.5.8".to_string(),
                version: "0.6.9".to_string(),
                old_file_id: "100".to_string(),
                file_id: "200".to_string(),
                time: chrono::Local::now().fixed_offset(),
                sha1: "sha1-aaa".to_string(),
            },
        )
        .unwrap();
    // 同一个项目再追一条：名字传空，不该把已有的名字冲掉
    instance
        .append_mod_update(
            "5123456",
            "",
            ModUpdateRecordObj {
                file: "sodium-0.7.jar".to_string(),
                time: chrono::Local::now().fixed_offset(),
                sha1: "sha1-bbb".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
    // 另一个项目是另一项
    instance
        .append_mod_update(
            "AANobbMI",
            "Lithium",
            ModUpdateRecordObj {
                file: "lithium-0.12.jar".to_string(),
                time: chrono::Local::now().fixed_offset(),
                ..Default::default()
            },
        )
        .unwrap();

    let path = instance.get_mod_update_file();
    assert!(path.ends_with(mml_names::names::MOD_UPDATE_FILE));

    let list = instance.read_mod_update();
    assert_eq!(list.len(), 2, "两个项目应是两项");
    let sodium = list.get("5123456").expect("应有 sodium 那一项");
    assert_eq!(sodium.name, "Sodium", "名字传空时沿用原来记的");
    assert_eq!(sodium.update.len(), 2, "两次追加应留下两条记录");
    assert_eq!(sodium.update[0].file, "sodium-0.6.jar");
    assert_eq!(sodium.update[0].sha1, "sha1-aaa");
    assert_eq!(sodium.update[0].old_version, "0.5.8");
    assert_eq!(sodium.update[0].version, "0.6.9");
    assert_eq!(sodium.update[0].old_file_id, "100");
    assert_eq!(sodium.update[0].file_id, "200");
    assert_eq!(sodium.update[1].file, "sodium-0.7.jar");
    // 时间由追加时打上，非零即说明确实写进去了
    assert!(sodium.update[0].time.timestamp() > 0);
    assert_eq!(
        list.get("AANobbMI").expect("应有 lithium 那一项").name,
        "Lithium"
    );

    // 删除走"整份覆盖写回"：留下哪几条由调用方给
    let mut trimmed = list.clone();
    trimmed.get_mut("5123456").unwrap().update.remove(0);
    instance.save_mod_update(&trimmed).unwrap();
    let left = instance.read_mod_update();
    let left = left.get("5123456").expect("删除后另一项还在");
    assert_eq!(left.update.len(), 1);
    assert_eq!(left.update[0].file, "sodium-0.7.jar");
    // 名字跟着项目项留着，删掉一条记录不影响它
    assert_eq!(left.name, "Sodium");
}
