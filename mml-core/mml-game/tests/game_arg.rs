//! 启动参数构造测试：游戏参数、自动进入存档 / 服务器、代理、自定义参数与占位符替换。
//!
//! `make_game_arg` 依赖全局配置，即 `mml_config::read_config`，
//! `replace_arg` 依赖实例目录初始化，即 `mml_game::init`，
//! 因此本文件所有测试共用同一个临时运行目录，通过 `OnceLock` 保证初始化只执行一次。
//!
//! 初始化时会把全局窗口配置清空，使参数生成只由实例自身配置决定；
//! 需要“实例缺省 → 全局配置兜底”行为的测试通过 `CONFIG_LOCK` 独占执行，
//! 先改配置再还原，避免与并行测试相互干扰。

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use mml_config::config_obj::{GCType, RunArgObj, WindowSettingObj};
use mml_game::game_launch::AutoJoinType;
use mml_game::game_saves::SaveObj;
use mml_game::launcher::instance_setting_obj::{InstanceSettingObj, ProxyHostObj, ServerObj};
use mml_names::get_line_ending;

/// 初始化内核各系统并返回本次运行的临时目录，每次运行前先删旧目录
///
/// # 返回值
///
/// 本次测试专用的运行目录，进程内只初始化一次
fn init_all() -> PathBuf {
    static INIT: OnceLock<PathBuf> = OnceLock::new();
    INIT.get_or_init(|| {
        let run_dir = mml_testutil::temp_dir().join("mml-game-arg-test");
        let _ = std::fs::remove_dir_all(&run_dir);
        std::fs::create_dir_all(&run_dir).expect("创建测试运行目录失败");

        mml_base::init(&run_dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_game::init(mml_base::get_base_dir()).unwrap();

        // 清空全局窗口配置，让测试只受实例配置影响
        {
            let mut config = mml_config::write_config();
            config.window = WindowSettingObj::default();
        }
        run_dir
    })
    .clone()
}

/// 修改全局配置的测试需要独占执行，其他依赖空窗口配置的测试也持同一把锁
fn lock_config() -> MutexGuard<'static, ()> {
    static CONFIG_LOCK: Mutex<()> = Mutex::new(());
    CONFIG_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

/// 构造一个无附加配置的测试实例
///
/// - `version`: 游戏版本
///
/// # 返回值
///
/// 返回实例名与目录都是 `arg-test` 的实例
fn make_instance(version: &str) -> InstanceSettingObj {
    InstanceSettingObj {
        name: "arg-test".to_string(),
        dir: "arg-test".to_string(),
        version: version.to_string(),
        ..Default::default()
    }
}

/// 窗口参数：实例设置了宽高，应生成 `--width` / `--height`
#[test]
fn game_arg_window_size() {
    init_all();
    let _lock = lock_config();

    let instance = InstanceSettingObj {
        window: Some(WindowSettingObj {
            width: Some(1280),
            height: Some(720),
            ..Default::default()
        }),
        ..make_instance("1.20.4")
    };

    let args = instance.make_game_arg(&AutoJoinType::None);
    assert_eq!(args, vec!["--width", "1280", "--height", "720"]);
}

/// 窗口参数：全屏时只生成 `--fullscreen`，忽略宽高
#[test]
fn game_arg_full_screen() {
    init_all();
    let _lock = lock_config();

    let instance = InstanceSettingObj {
        window: Some(WindowSettingObj {
            full_screen: Some(true),
            width: Some(1280),
            height: Some(720),
            ..Default::default()
        }),
        ..make_instance("1.20.4")
    };

    let args = instance.make_game_arg(&AutoJoinType::None);
    assert_eq!(args, vec!["--fullscreen"]);
}

/// 窗口参数：实例与全局配置都没设置时不生成窗口参数
#[test]
fn game_arg_no_window() {
    init_all();
    let _lock = lock_config();

    let instance = make_instance("1.20.4");
    let args = instance.make_game_arg(&AutoJoinType::None);
    assert!(args.is_empty(), "不应生成窗口参数: {args:?}");
}

/// 窗口参数：实例未设置时回退到全局配置，即默认 1280x720
#[test]
fn game_arg_window_config_fallback() {
    init_all();
    let _lock = lock_config();

    // 临时设置全局窗口配置
    let backup = {
        let config = mml_config::read_config();
        config.window.clone()
    };
    {
        let mut config = mml_config::write_config();
        config.window = WindowSettingObj::new();
    }

    // 无论断言是否通过都要还原配置
    let restore = |backup: WindowSettingObj| {
        let mut config = mml_config::write_config();
        config.window = backup;
    };

    let instance = make_instance("1.20.4");
    let args = instance.make_game_arg(&AutoJoinType::None);
    let ok = args == vec!["--width", "1280", "--height", "720"];
    restore(backup);

    assert!(ok, "实例缺省时应使用全局窗口配置: {args:?}");
}

/// 自动进入存档：生成 `--quickPlaySingleplayer`
#[test]
fn game_arg_auto_join_save() {
    init_all();
    let _lock = lock_config();

    let instance = make_instance("1.20.4");
    let save = Arc::new(SaveObj {
        level_name: "显示名不影响参数".to_string(),
        path: std::path::PathBuf::from("/saves/my-world"),
        ..Default::default()
    });

    let args = instance.make_game_arg(&AutoJoinType::Save(save));
    // 传的是**存档目录名**（level id），不是 level.dat 里的显示名
    assert_eq!(args, vec!["--quickPlaySingleplayer", "my-world"]);
}

/// 自动进入存档：1.20 以下没有 quick play，不带这个参数
#[test]
fn game_arg_auto_join_save_before_120() {
    init_all();
    let _lock = lock_config();

    let instance = make_instance("1.19.4");
    let save = Arc::new(SaveObj {
        path: std::path::PathBuf::from("/saves/my-world"),
        ..Default::default()
    });

    let args = instance.make_game_arg(&AutoJoinType::Save(save));
    assert!(
        !args.iter().any(|arg| arg == "--quickPlaySingleplayer"),
        "1.20 以下不该带 quick play: {args:?}"
    );
}

/// 自动进入服务器：1.20 及以上用 quickPlayMultiplayer，端口缺省 25565
#[test]
fn game_arg_auto_join_server_quick_play() {
    init_all();
    let _lock = lock_config();

    let instance = make_instance("1.20.4");
    let server = Arc::new(ServerObj {
        enable: true,
        ip: Some("mc.example.com".to_string()),
        port: None,
    });

    let args = instance.make_game_arg(&AutoJoinType::Server(server));
    assert_eq!(args, vec!["--quickPlayMultiplayer", "mc.example.com:25565"]);
}

/// 自动进入服务器：1.20 以下用 `--server` / `--port`
#[test]
fn game_arg_auto_join_server_legacy() {
    init_all();
    let _lock = lock_config();

    let instance = make_instance("1.12.2");
    let server = Arc::new(ServerObj {
        enable: true,
        ip: Some("mc.example.com".to_string()),
        port: Some(25566),
    });

    let args = instance.make_game_arg(&AutoJoinType::Server(server));
    assert_eq!(args, vec!["--server", "mc.example.com", "--port", "25566"]);
}

/// 自动进入服务器：地址为空时不生成参数
#[test]
fn game_arg_auto_join_server_empty_ip() {
    init_all();
    let _lock = lock_config();

    let instance = make_instance("1.20.4");
    let server = Arc::new(ServerObj {
        enable: true,
        ip: Some(String::new()),
        port: None,
    });

    let args = instance.make_game_arg(&AutoJoinType::Server(server));
    assert!(args.is_empty(), "空地址不应生成参数: {args:?}");
}

/// 代理设置：生成 `--proxyHost` 等参数，未设置的项跳过
#[test]
fn game_arg_proxy() {
    init_all();
    let _lock = lock_config();

    let instance = InstanceSettingObj {
        proxy_host: Some(ProxyHostObj {
            ip: Some("127.0.0.1".to_string()),
            port: Some(7890),
            user: Some("user".to_string()),
            password: None,
        }),
        ..make_instance("1.20.4")
    };

    let args = instance.make_game_arg(&AutoJoinType::None);
    assert_eq!(
        args,
        vec![
            "--proxyHost",
            "127.0.0.1",
            "--proxyPort",
            "7890",
            "--proxyUser",
            "user"
        ]
    );
}

/// 自定义游戏参数：按行拆分追加到末尾
#[test]
fn game_arg_custom_game_args() {
    init_all();
    let _lock = lock_config();

    let instance = InstanceSettingObj {
        jvm_arg: Some(RunArgObj {
            game_args: Some("-doSomething\n-doAnother".to_string()),
            ..Default::default()
        }),
        ..make_instance("1.20.4")
    };

    let args = instance.make_game_arg(&AutoJoinType::None);
    assert_eq!(args, vec!["-doSomething", "-doAnother"]);
}

/// 自定义 GC 参数属于 JVM 参数，不参与游戏参数生成
#[test]
fn game_arg_ignores_gc_mode() {
    init_all();
    let _lock = lock_config();

    let instance = InstanceSettingObj {
        jvm_arg: Some(RunArgObj {
            gc_mode: Some(GCType::ZGC),
            ..Default::default()
        }),
        ..make_instance("1.20.4")
    };

    let args = instance.make_game_arg(&AutoJoinType::None);
    assert!(args.is_empty(), "GC 配置不应影响游戏参数: {args:?}");
}

/// 占位符替换：实例名、UUID、游戏目录、Java 路径与 JVM 参数
#[test]
fn replace_arg_placeholders() {
    init_all();

    let instance = make_instance("1.20.4");
    let jvm = PathBuf::from("java-bin");
    let jvm_args = vec!["-Xmx1024m".to_string(), "-Xms512m".to_string()];

    let item = "%GAME_NAME%|%GAME_UUID%|%GAME_DIR%|%GAME_BASE_DIR%|%JAVA_LOCAL%|%JAVA_ARG%";
    let out = instance.replace_arg(&jvm, &jvm_args, item);

    let uuid = instance.uuid.to_string();
    let game_path = instance.get_game_path().to_string_lossy().to_string();
    let java_arg = format!("-Xmx1024m{e}-Xms512m{e}", e = get_line_ending());

    let mut parts = out.split('|');
    assert_eq!(parts.next(), Some("arg-test"));
    assert_eq!(parts.next(), Some(uuid.as_str()));
    // %GAME_DIR% 与 %GAME_BASE_DIR% 都替换为游戏目录 .minecraft
    assert_eq!(parts.next(), Some(game_path.as_str()));
    assert_eq!(parts.next(), Some(game_path.as_str()));
    assert_eq!(parts.next(), Some("java-bin"));
    // %JAVA_ARG% 用换行符拼接
    assert_eq!(parts.next(), Some(java_arg.as_str()));
    assert_eq!(parts.next(), None);
}

/// 占位符替换：无占位符的参数原样返回
#[test]
fn replace_arg_no_placeholder() {
    init_all();

    let instance = make_instance("1.20.4");
    let out = instance.replace_arg(&PathBuf::from("java"), &Vec::new(), "-client");
    assert_eq!(out, "-client");
}
