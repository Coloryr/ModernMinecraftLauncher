//! Modrinth API 的集成测试，走实时网络请求。
//!
//! 与 mml-game 的测试约定一致：网络不可用时打印提示并跳过，不视为失败。
//! 请求通过 `mml_net` 的全局客户端进行，需先初始化配置与 HTTP 客户端。

use mml_net::modrinth_api::{
    ModrinthSearchArg, ModrinthSortType, get_game_versions, get_modpack_list, get_project,
    get_versions,
};

/// 一次性初始化：配置与 HTTP 客户端。
fn init() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let dir = mml_testutil::temp_dir().join(format!("mml-net-api-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("创建临时配置目录失败");
        mml_config::init(&dir);
        mml_net::init();
    });
}

/// 查询 Fabulously Optimized 项目元数据。
#[tokio::test]
async fn modrinth_project_metadata() {
    init();

    let Ok(project) = get_project("fabulously-optimized").await else {
        eprintln!("[跳过] 无法获取项目 fabulously-optimized（网络不可用？）");
        return;
    };

    assert!(!project.id.is_empty(), "项目 id 不应为空");
    assert_eq!(project.title, "Fabulously Optimized");
    assert_eq!(project.project_type, "modpack");
    assert!(project.downloads > 0, "下载量应大于 0");
}

/// 获取指定版本号内容。
#[tokio::test]
async fn modrinth_version_lookup() {
    init();

    match get_versions(vec![String::from("Sq7Ilgdn")]).await {
        Ok(versions) => {
            assert_eq!(versions.len(), 1, "应按 id 精确返回一个版本");
            let version = &versions[0];
            assert_eq!(version.project_id, "1KVo5zza");
            assert_eq!(version.version_number, "14.0.0-beta.3");
            assert!(!version.files.is_empty(), "版本应包含至少一个文件");
        }
        Err(err) => {
            eprintln!("{err}");
            eprintln!("[跳过] 无法获取版本 Sq7Ilgdn（网络不可用？）");
        }
    }
}

/// 获取全部游戏版本列表。
#[tokio::test]
async fn modrinth_game_versions() {
    init();

    let Ok(list) = get_game_versions().await else {
        eprintln!("[跳过] 无法获取游戏版本列表（网络不可用？）");
        return;
    };

    assert!(!list.is_empty(), "游戏版本列表不应为空");
    assert!(
        list.iter().any(|v| v.version.contains('.')),
        "版本号应形如 x.y.z"
    );
}

/// 搜索整合包。
#[tokio::test]
async fn modrinth_search() {
    init();

    let arg = ModrinthSearchArg {
        version: None,
        query: Some(String::from("fabulously optimized")),
        sort: ModrinthSortType::Relevance,
        page: Some(0),
        page_size: Some(5),
        category: None,
        loader: None,
    };

    let Ok(result) = get_modpack_list(arg).await else {
        eprintln!("[跳过] 搜索整合包失败（网络不可用？）");
        return;
    };

    assert!(result.total_hits > 0, "搜索应有结果");
    assert!(!result.hits.is_empty(), "搜索结果不应为空");
    assert!(
        result
            .hits
            .iter()
            .any(|hit| hit.title.contains("Fabulously")),
        "结果中应包含 Fabulously Optimized"
    );
}
