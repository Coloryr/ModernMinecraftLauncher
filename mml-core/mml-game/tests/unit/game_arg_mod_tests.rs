//! 启动参数里的 JVM 部分：内存 `-Xms` / `-Xmx`、GC 档与自定义 JVM 参数。
//!
//! 放在 crate 内是因为 `make_jvm_arg` 是私有方法，`tests/game_arg.rs` 够不着；
//! 而公开的 `make_run_arg` 要先过版本 json 的联网校验，没法在测试里用。
//! 全程离线：账户取默认的离线账户，只有外置登录那几档才会联网。
//!
//! 全局配置里的 `jvm_arg` 是实例没给值时的兜底，所以这里先清成全 `None`，
//! 并让改配置的用例独占执行（同 `tests/game_arg.rs` 的 CONFIG_LOCK）。

use super::*;

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, Once};
use std::thread;
use std::time::{Duration, Instant};

use mml_auth::LoginObj;
use mml_config::config_obj::{GCType, RunArgObj};
use mml_names::names;
use uuid::Uuid;

use crate::game_launch::{AutoJoinType, GameLaunchArg, GameRunObj, InstanceHandle};
use crate::launcher::LogEncoding;
use crate::test_support::{boot, run_log_texts};

/// 取全局配置锁，改配置与读配置的用例串行
fn lock_config() -> MutexGuard<'static, ()> {
    static CONFIG_LOCK: Mutex<()> = Mutex::new(());
    CONFIG_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

/// 起内核各系统，并把全局 JVM 配置清成全 `None`
///
/// boot() 不含本 crate 的 init，而 -Dcoloryrasm.* 那段要读实例路径（BASE_DIR），
/// 所以这里补上；路径都是 `OnceLock::get_or_init`，重复初始化是幂等的。
///
/// # 返回值
///
/// 持有配置锁，用例结束才释放
fn setup() -> MutexGuard<'static, ()> {
    static INIT: Once = Once::new();
    let guard = lock_config();
    INIT.call_once(|| {
        let run = boot();
        crate::init(&run).unwrap();
        let mut config = mml_config::write_config();
        config.jvm_arg = RunArgObj::default();
    });
    guard
}

/// 造一个带指定 JVM 配置的实例
///
/// - `jvm`: 实例自己的 JVM 配置，其余字段取默认值
///
/// # 返回值
///
/// 返回实例
fn instance(jvm: RunArgObj) -> InstanceSettingObj {
    InstanceSettingObj {
        name: "launch-arg-test".to_string(),
        dir: "launch-arg-test".to_string(),
        version: "1.21.1".to_string(),
        jvm_arg: Some(jvm),
        ..Default::default()
    }
}

/// MMC 补丁的 `conflicts` 要能解析，且只有认得的组件才会剔掉对应运行库
///
/// GTNH 的 `org.lwjgl3` 就是靠这条把原版 LWJGL2 顶掉的；解析不到的话两条
/// `org.lwjgl.openal` 会一起进 classpath，游戏在 Computronics 初始化时崩。
#[test]
fn custom_conflicts_drop_component() {
    let json = br#"{"uid":"org.lwjgl3","conflicts":[{"uid":"org.lwjgl"}]}"#;
    let lwjgl3: CustomGameArgObj = mml_base::serialize_tools::json_from_bytes(json).unwrap();
    let lwjgl2 = CustomGameArgObj {
        uid: "org.lwjgl".to_string(),
        ..Default::default()
    };

    let mut custom = vec![
        ("org.lwjgl".to_string(), lwjgl2),
        ("org.lwjgl3".to_string(), lwjgl3),
    ];
    drop_conflicting_patches(&mut custom);
    let uids: Vec<&str> = custom.iter().map(|(_, item)| item.uid.as_str()).collect();
    assert_eq!(uids, vec!["org.lwjgl3"]);

    // 没有原版组件时，Mojang 那层照旧补
    assert!(!has_minecraft_component(&custom));
    custom.push((
        "net.minecraft".to_string(),
        CustomGameArgObj {
            uid: MINECRAFT_COMPONENT.to_string(),
            ..Default::default()
        },
    ));
    assert!(has_minecraft_component(&custom));
}

/// `--userProperties` 只能是空对象
///
/// 游戏按版本按不同形状读它：1.7.10 是 `Map<String, Collection<String>>`、
/// 1.8.x 是 authlib 的 `PropertyMap`。塞 `{"language":"zh_CN"}` 会让 GTNH 这类
/// 1.7.10 整合包直接崩在 `Main.main` 的 `JsonSyntaxException`；语言本来就由
/// options.txt 决定，与这个参数无关。
#[test]
fn user_properties_is_empty_object() {
    let _guard = setup();
    let obj = instance(RunArgObj::default());
    let mut args = vec![
        "--userProperties".to_string(),
        "${user_properties}".to_string(),
    ];
    obj.replace_all_value(&LoginObj::default(), &mut args, "", Path::new("."), &None);
    assert_eq!(args[1], "{}");
}

/// 隔离用户目录：HOME / APPDATA 指到实例目录，并补 `user.home`
#[tokio::test]
async fn isolate_home_paths() {
    let _guard = setup();
    let obj = instance(RunArgObj {
        isolate_home: Some(true),
        ..Default::default()
    });
    let home = obj.get_base_path().to_string_lossy().into_owned();
    let env = obj.make_env_arg();
    assert_eq!(env.get("HOME"), Some(&home));
    assert_eq!(env.get("USERPROFILE"), Some(&home));
    assert!(env["XDG_CONFIG_HOME"].ends_with(".config"));
    let args = obj
        .make_jvm_arg(&LoginObj::default(), 21, None, &[])
        .await
        .unwrap();
    assert!(args.iter().any(|a| a == &format!("-Duser.home={home}")));

    let off = instance(RunArgObj {
        isolate_home: Some(false),
        ..Default::default()
    });
    assert!(!off.make_env_arg().contains_key("HOME"));
    let args = off
        .make_jvm_arg(&LoginObj::default(), 21, None, &[])
        .await
        .unwrap();
    assert!(!args.iter().any(|a| a.starts_with("-Duser.home=")));
}

/// ColorASM 注入：端口、实例 ID 与验证密钥都要写进 JVM 参数
#[tokio::test]
async fn jvm_arg_colorasm() {
    let _guard = setup();
    let obj = instance(RunArgObj {
        colorasm: Some(true),
        ..Default::default()
    });
    let mixin = MixinArg {
        port: 12345,
        key: "test-key".to_string(),
    };

    let args = obj
        .make_jvm_arg(&LoginObj::default(), 21, Some(&mixin), &[])
        .await
        .unwrap();

    assert!(args.contains(&"-Dcoloryrasm.mixin.port=12345".to_string()));
    assert!(args.contains(&format!("-Dcoloryrasm.mixin.uuid={}", obj.uuid)));
    assert!(args.contains(&"-Dcoloryrasm.mixin.key=test-key".to_string()));

    // 下载检查会跳过没有下载地址的本地库，jar 必须由这一步自己写出来：
    // 文件不存在时 JVM 会直接以 "agent library failed to init" 退出
    let agent = args
        .iter()
        .find_map(|a| a.strip_prefix("-javaagent:"))
        .expect("启用 ColorASM 时必须注入 javaagent");
    let agent = std::path::Path::new(agent);
    assert!(agent.is_file(), "注入的 ColorASM jar 不存在：{}", agent.display());
    assert_eq!(
        std::fs::read(agent).unwrap().get(..2),
        Some(&b"PK"[..]),
        "注入的 ColorASM jar 不是有效的 zip"
    );
}

/// 内存：最小填 `-Xms`、最大填 `-Xmx`，且 `-Xms` 排在前面
#[tokio::test]
async fn jvm_arg_memory() {
    let _guard = setup();
    let obj = instance(RunArgObj {
        min_memory: Some(512),
        max_memory: Some(4096),
        // 关掉 GC，这条只盯内存
        gc_mode: Some(GCType::None),
        ..Default::default()
    });

    let args = obj
        .make_jvm_arg(&LoginObj::default(), 21, None, &[])
        .await
        .unwrap();

    let min = args
        .iter()
        .position(|a| a == "-Xms512m")
        .expect("应生成 -Xms512m");
    let max = args
        .iter()
        .position(|a| a == "-Xmx4096m")
        .expect("应生成 -Xmx4096m");
    assert!(min < max, "-Xms 应排在 -Xmx 之前");
}

/// 内存是 0 或没设：该档不生成，0 表示不限而不是 0 MB
#[tokio::test]
async fn jvm_arg_memory_zero_skips() {
    let _guard = setup();
    let obj = instance(RunArgObj {
        min_memory: Some(0),
        max_memory: None,
        gc_mode: Some(GCType::None),
        ..Default::default()
    });

    let args = obj
        .make_jvm_arg(&LoginObj::default(), 21, None, &[])
        .await
        .unwrap();

    assert!(
        !args.iter().any(|a| a.starts_with("-Xms")),
        "0 不该生成 -Xms"
    );
    assert!(
        !args.iter().any(|a| a.starts_with("-Xmx")),
        "没设不该生成 -Xmx"
    );
}

/// GC 档：自动档按 Java 版本挑，21 及以上走 ZGC、之下走 G1GC；显式指定时听指定的
#[tokio::test]
async fn jvm_arg_gc() {
    let _guard = setup();
    let auth = LoginObj::default();
    let auto = instance(RunArgObj {
        gc_mode: Some(GCType::Auto),
        ..Default::default()
    });

    let zgc = auto.make_jvm_arg(&auth, 21, None, &[]).await.unwrap();
    assert!(
        names::GCZGC
            .iter()
            .all(|item| zgc.contains(&item.to_string())),
        "Java 21 应走 ZGC"
    );
    assert!(
        !zgc.contains(&names::G1GC[0].to_string()),
        "自动档不该同时给 G1GC"
    );
    // 分代开关要等选好实际 Java 再按版本补（见 game_launch 的 fix_zgc_args），这里只出基础参数
    for java in [21u8, 22, 23, 26] {
        let args = auto.make_jvm_arg(&auth, java, None, &[]).await.unwrap();
        assert!(
            args.contains(&"-XX:+UseZGC".to_string()),
            "Java {java} 应走 ZGC"
        );
        assert!(
            !args.contains(&"-XX:+ZGenerational".to_string()),
            "Java {java} 的分代开关不该在这里加"
        );
    }

    let g1 = auto.make_jvm_arg(&auth, 17, None, &[]).await.unwrap();
    assert!(
        names::G1GC
            .iter()
            .all(|item| g1.contains(&item.to_string())),
        "Java 17 应走 G1GC"
    );
    assert!(
        !g1.contains(&names::GCZGC[0].to_string()),
        "低版本不该给 ZGC"
    );

    // 显式档与自动判定不一致时按显式的来
    let forced = instance(RunArgObj {
        gc_mode: Some(GCType::G1GC),
        ..Default::default()
    });
    let forced = forced.make_jvm_arg(&auth, 21, None, &[]).await.unwrap();
    assert!(
        names::G1GC
            .iter()
            .all(|item| forced.contains(&item.to_string())),
        "显式 G1GC 应压过自动档"
    );
}

/// 版本 JSON / 加载器已经带了 GC 时，自动档不再叠一套；显式档仍按用户选的追加
#[tokio::test]
async fn jvm_arg_gc_skips_when_existing_has_gc() {
    let _guard = setup();
    let auth = LoginObj::default();
    let existing = vec![
        "-XX:+UnlockExperimentalVMOptions".to_string(),
        "-XX:+UseZGC".to_string(),
    ];

    let auto = instance(RunArgObj {
        gc_mode: Some(GCType::Auto),
        ..Default::default()
    });
    let args = auto.make_jvm_arg(&auth, 21, None, &existing).await.unwrap();
    assert!(
        !args.iter().any(|arg| arg == "-XX:+UseZGC"
            || arg == "-XX:+UseG1GC"),
        "已有 GC 时自动档不应再追加: {args:?}"
    );

    // 自定义参数里已经写了 GC 时同样不叠加
    let custom = instance(RunArgObj {
        gc_mode: Some(GCType::Auto),
        jvm_args: Some("-XX:+UseZGC".to_string()),
        ..Default::default()
    });
    let args = custom.make_jvm_arg(&auth, 21, None, &[]).await.unwrap();
    assert_eq!(
        args.iter().filter(|arg| *arg == "-XX:+UseZGC").count(),
        1,
        "自定义参数里的 ZGC 不应再被自动档复制一份"
    );
    assert!(!args.contains(&"-XX:+UseG1GC".to_string()));

    // 显式选择是用户的主动覆盖，照旧追加在后面，JVM 取最后一条
    let forced = instance(RunArgObj {
        gc_mode: Some(GCType::G1GC),
        ..Default::default()
    });
    let args = forced.make_jvm_arg(&auth, 21, None, &existing).await.unwrap();
    assert!(args.contains(&"-XX:+UseG1GC".to_string()));
}

/// 自定义 JVM 参数：按行追加，首尾空白与空行都去掉
#[tokio::test]
async fn jvm_arg_custom_lines() {
    let _guard = setup();
    let obj = instance(RunArgObj {
        jvm_args: Some("  -Dfoo=1  \n\n-Dbar=2\n".to_string()),
        gc_mode: Some(GCType::None),
        ..Default::default()
    });

    let args = obj
        .make_jvm_arg(&LoginObj::default(), 17, None, &[])
        .await
        .unwrap();

    assert!(
        args.contains(&"-Dfoo=1".to_string()),
        "应保留自定义参数并去掉首尾空白"
    );
    assert!(args.contains(&"-Dbar=2".to_string()));
    assert!(!args.iter().any(|a| a.is_empty()), "空行不该留成空参数");
}

/// 本机能跑的 Java：先 `JAVA_HOME`，再 PATH 上的 `java`
///
/// 探针走源文件模式（`java 探针.java`），那是 JDK 的能力，纯 JRE 跑不了，
/// 所以这里用 `--list-modules` 认一下有没有 `jdk.compiler`。
///
/// # 返回值
///
/// 可用则返回 java 可执行文件路径，没有可用 JDK 时返回 `None`
fn find_jdk() -> Option<PathBuf> {
    let mut list = Vec::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let name = if cfg!(windows) { "java.exe" } else { "java" };
        list.push(PathBuf::from(home).join("bin").join(name));
    }
    list.push(PathBuf::from("java"));

    list.into_iter().find(|java| {
        Command::new(java)
            .arg("--list-modules")
            .output()
            .map(|out| {
                out.status.success()
                    && String::from_utf8_lossy(&out.stdout).contains("jdk.compiler")
            })
            .unwrap_or(false)
    })
}

/// 真实启动：命令行由启动器自己拼，进程是真的 Java、跑的是真代码
///
/// 探针用源文件模式跑，打印自定义 JVM 参数与游戏参数，于是"参数是否真的传进进程"
/// 也能一起断言。本机没有带 `jdk.compiler` 的 JDK 就跳过 —— 那种机器上启动器
/// 本身也起不了游戏。
#[tokio::test]
async fn real_launch_starts_java_process() {
    let _guard = setup();
    let Some(java) = find_jdk() else {
        eprintln!("跳过真实启动用例：本机没有可用的 JDK");
        return;
    };

    // 探针：把自定义 JVM 参数与游戏参数都打出来
    let dir = mml_testutil::temp_path(format!("mml-real-launch-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let probe = dir.join("MmlLaunchProbe.java");
    std::fs::write(
        &probe,
        r#"public class MmlLaunchProbe {
public static void main(String[] args) {
    System.out.println("mml-probe-ok prop=" + System.getProperty("mml.probe") + " args=" + String.join(",", args));
}
}
"#,
    )
    .unwrap();

    // 命令行：JVM 参数走 make_jvm_arg，整条走 make_run_arg，与真实启动同一入口
    let instance = instance(RunArgObj {
        min_memory: Some(256),
        max_memory: Some(512),
        // GC 档不选：本机 Java 版本不确定，GC 的两条用例已经各自断言过参数内容
        gc_mode: Some(GCType::None),
        jvm_args: Some("-Dmml.probe=1".to_string()),
        ..Default::default()
    });
    let jvm = instance
        .make_jvm_arg(&LoginObj::default(), 21, None, &[])
        .await
        .unwrap();

    let mut obj = GameLaunchObj {
        jvm_args: jvm,
        main_class: probe.to_string_lossy().to_string(),
        game_args: vec!["--probe-arg".to_string()],
        ..Default::default()
    };
    let launch_arg = GameLaunchArg {
        auth: LoginObj::default(),
        auto: AutoJoinType::None,
        admin: false,
        mixin: None,
        skip_server_pack: false,
    };
    let args = instance.make_run_arg(&launch_arg, &mut obj, false);
    assert!(
        args.contains(&"-Xms256m".to_string()),
        "命令行里应有内存参数"
    );
    assert!(
        args.contains(&probe.to_string_lossy().to_string()),
        "命令行里应有主类"
    );

    // 真起进程：句柄负责读流进运行日志、tick 判退出
    let uuid = Uuid::new_v4();
    let handle = InstanceHandle::new(GameRunObj {
        uuid,
        encoding: LogEncoding::UTF8,
        auth: LoginObj::default(),
        path: dir.join("game"),
        java,
        args,
        env: Default::default(),
        admin: false,
    })
    .expect("启动 Java 进程失败");

    // 先等输出：源文件模式要先编译，给足 60 秒
    let deadline = Instant::now() + Duration::from_secs(60);
    while run_log_texts(&uuid)
        .iter()
        .all(|line| !line.contains("mml-probe-ok"))
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(50));
    }

    let texts = run_log_texts(&uuid);
    let line = texts
        .iter()
        .find(|line| line.contains("mml-probe-ok"))
        .unwrap_or_else(|| panic!("运行日志里应出现探针输出，实际：{texts:?}"));

    // 三件事各证明一段链路：进程起来了、自定义 JVM 参数到了 JVM、游戏参数到了进程
    assert!(
        line.contains("prop=1"),
        "自定义 JVM 参数应真的传给 JVM：{line}"
    );
    assert!(
        line.contains("args=--probe-arg"),
        "游戏参数应真的传给进程：{line}"
    );

    // 再等进程退出
    let deadline = Instant::now() + Duration::from_secs(30);
    while !handle.is_exit() && Instant::now() < deadline {
        handle.tick();
        thread::sleep(Duration::from_millis(50));
    }
    assert!(handle.is_exit(), "Java 进程应在 30 秒内退出");

    let _ = std::fs::remove_dir_all(&dir);
}

/// 构件名带平台的原生库不能被 ARM 回退当成三段坐标
///
/// GTNH 的 lwjgl3 patch 写作 `org.lwjgl:lwjgl-natives-macos:3.4.3`：平台在构件名里，
/// 最后一段仍是版本。按最后一个 `:` 截断只剩两段，回退再取 `path[2]` 就越界 ——
/// 整条启动线程 panic，表现是 GTNH 点了启动直接"游戏启动线程异常退出"。
#[tokio::test]
async fn arm_native_fallback_skips_platform_in_artifact() {
    let _guard = setup();
    let json = r#"{
        "libraries": [
            {
                "name": "org.lwjgl:lwjgl-natives-macos:3.4.3",
                "rules": [{"action": "allow", "os": {"name": "osx"}}],
                "downloads": {"artifact": {
                    "path": "org/lwjgl/lwjgl-natives-macos/3.4.3/lwjgl-natives-macos-3.4.3.jar",
                    "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "url": "https://example.invalid/lwjgl-natives-macos.jar"
                }}
            },
            {
                "name": "org.lwjgl:lwjgl-natives-macos-arm64:3.4.3",
                "rules": [
                    {"action": "allow", "os": {"name": "osx"}},
                    {"action": "allow", "os": {"name": "osx-arm64"}}
                ],
                "downloads": {"artifact": {
                    "path": "org/lwjgl/lwjgl-natives-macos-arm64/3.4.3/lwjgl-natives-macos-arm64-3.4.3.jar",
                    "sha1": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "url": "https://example.invalid/lwjgl-natives-macos-arm64.jar"
                }}
            }
        ]
    }"#;
    let game: GameArgObj = mml_base::serialize_tools::json_from_str(json).unwrap();
    let dir = mml_testutil::temp_path(format!("mml-arm-native-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();

    let libs = game.build_game_libraries(&dir, None).await;

    // 其它系统被 osx 规则整批滤掉，只验证不 panic
    if cfg!(target_os = "macos") {
        assert!(
            libs.iter()
                .any(|item| item.name.ends_with("-natives-macos:3.4.3"))
        );
    }
    // Apple Silicon 上必须带上 arm64 那份：lwjgl3 的 dylib 只从 classpath 里取
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert!(
            libs.iter()
                .any(|item| item.name.contains("-natives-macos-arm64:")),
            "arm64 原生库被规则滤掉了：{:?}",
            libs.iter().map(|item| &item.name).collect::<Vec<_>>()
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// MMC 的 FirstThreadOnMacOS 要落成 -XstartOnFirstThread，且只在 macOS 加
///
/// GTNH 的 lwjgl3ify 用 RFB 的 MainStartOnFirstThread，没有这个 JVM 参数时
/// 事件循环跑不到主线程，SDL 初始化报 No available video device。
#[test]
fn custom_trait_first_thread_on_macos() {
    let patch: CustomGameArgObj = mml_base::serialize_tools::json_from_str(
        r#"{"uid":"me.eigenraven.lwjgl3ify.launchargs","+traits":["FirstThreadOnMacOS"]}"#,
    )
    .unwrap();
    let traits = patch.add_traits.unwrap();

    let mut args = Vec::new();
    super::custom::apply_traits(&mut args, &traits);
    if mml_sys::get_system_info().os == mml_sys::Os::MacOS {
        assert_eq!(args, vec!["-XstartOnFirstThread"]);
    } else {
        assert!(args.is_empty(), "非 macOS 不该加 -XstartOnFirstThread");
    }

    // 不认识的特性不产生参数；重复应用不会加第二遍
    let mut args = Vec::new();
    super::custom::apply_traits(&mut args, &["SomethingElse".to_string()]);
    assert!(args.is_empty());
    super::custom::apply_traits(&mut args, &traits);
    super::custom::apply_traits(&mut args, &traits);
    assert_eq!(
        args.len(),
        usize::from(mml_sys::get_system_info().os == mml_sys::Os::MacOS)
    );
}
