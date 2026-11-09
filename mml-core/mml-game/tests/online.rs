//! 在线版本接口测试，依赖网络；拉不到版本清单时跳过，不算失败。

use std::path::PathBuf;
use std::sync::OnceLock;

/// 初始化内核各系统并返回本次运行的临时目录，每次运行前先删旧目录
///
/// # 返回值
///
/// 本次测试专用的运行目录，进程内只初始化一次
fn init_all() -> PathBuf {
    static INIT: OnceLock<PathBuf> = OnceLock::new();
    INIT.get_or_init(|| {
        let run_dir = mml_testutil::temp_dir().join("mml-game-online-test");
        let _ = std::fs::remove_dir_all(&run_dir);
        std::fs::create_dir_all(&run_dir).expect("创建测试运行目录失败");

        mml_base::init(&run_dir);
        mml_names::init(mml_base::get_base_dir()).unwrap();
        mml_log::start(mml_base::get_base_dir()).unwrap();
        mml_config::init(mml_base::get_base_dir()).unwrap();
        mml_game::init(mml_base::get_base_dir()).unwrap();
        mml_net::init();
        run_dir
    })
    .clone()
}

/// 探测 Mojang 版本清单能否访问，离线时由调用方跳过
///
/// # 返回值
///
/// 清单非空时为 true
async fn network_available() -> bool {
    let ok = mml_net::get_work_client()
        .get_bytes("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .await
        .map(|data| !data.is_empty())
        .unwrap_or(false);
    if !ok {
        eprintln!("跳过: 网络不可用（无法获取 Mojang 版本清单）");
    }
    ok
}

/// 在线检查版本更新：能拉到指定版本的启动参数 JSON 并落盘缓存。
#[tokio::test]
async fn check_update_online() {
    init_all();

    if !network_available().await {
        return;
    }

    let obj = mml_game::launcher_path::version_path::check_update("1.20.4")
        .await
        .expect("在线检查版本失败");
    assert_eq!(obj.id, "1.20.4");
    assert!(!obj.main_class.is_empty());

    // 再次调用应命中本地缓存
    let cached =
        mml_game::launcher_path::version_path::get_version("1.20.4").expect("本地缓存版本失败");
    assert_eq!(cached.id, "1.20.4");
}

/// 在线检查不存在的版本号：应返回版本未找到错误而不是 panic。
#[tokio::test]
async fn check_update_unknown_version() {
    init_all();

    if !network_available().await {
        return;
    }

    let res = mml_game::launcher_path::version_path::check_update("0.0.0-not-exist").await;
    assert!(res.is_err(), "不存在的版本号应返回错误");
}
