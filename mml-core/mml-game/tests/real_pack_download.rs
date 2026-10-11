//! 真实整合包下载安装测试：从网上下载真包，走完整安装链
//! 1. Modrinth 真包 Fabulously Optimized，Fabric，48 文件
//! 2. Modrinth 真包 Optimized FPS，NeoForge，17 文件
//! 3. CurseForge 真包 Fabulously Optimized，`#[ignore]` 手动测试，key 由环境
//!    变量 `MML_CF_API_KEY` 注入，与 GUI 运行时同源
//! 4. Modrinth 大体积真包 Fresh & Smooth，`#[ignore]` 手动测试，约 310MB，
//!    mods/resourcepacks/shaderpacks 多目录安装
//!
//! 无网时自动跳过。全局初始化每进程一次，互斥锁串行各用例。

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};

use mml_downloader::download_item::DownloadItem;
use mml_game::add_game::{self, PackType};
use mml_game::gui_hook::{AddModPackState, IAddModPackGui};
use mml_game::loader::LoaderType;
use mml_net::curseforge_api::{self, CurseForgeSortType};

mod common;

/// 测试运行目录：系统临时目录加进程号，避免多进程冲突
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join(format!("mml-real-pack-{}", std::process::id()))
}

/// 初始化链，与 mml_core::init 相同，每进程一次
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

        // 可选测试代理：设置 MML_TEST_PROXY=ip:port 时走显式代理
        // reqwest 的 Auto 模式在本环境不读取 HTTP_PROXY 等环境变量，须写进配置
        if let Ok(proxy) = std::env::var("MML_TEST_PROXY") {
            let (ip, port) = proxy
                .split_once(':')
                .expect("MML_TEST_PROXY 格式应为 ip:port");
            use mml_config::config_obj::ProxyState;
            {
                let mut config = mml_config::write_config();
                config.http.work_proxy = ProxyState::User;
                config.http.login_proxy = ProxyState::User;
                config.http.proxy_ip = ip.to_string();
                config.http.proxy_port = port.parse().expect("MML_TEST_PROXY 端口不合法");
            }
            println!("[代理] 走显式代理 {proxy}");
        }

        mml_config::config_save::start();
        mml_game::init(mml_base::get_base_dir()).unwrap();
        mml_net::init();

        // CurseForge API key：由环境变量 MML_CF_API_KEY 注入，与 GUI 运行时同源
        if let Ok(key) = std::env::var("MML_CF_API_KEY") {
            mml_net::curseforge_api::set_key(&key);
        }

        mml_downloader::init(&dir).unwrap();
        mml_downloader::set_gui_handel(Box::new(TestDownloader));
        mml_downloader::start();
    });
}

/// 全局状态共享同一进程，串行执行各用例
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 探测网络是否可用；无法访问外网时跳过用例
async fn network_available() -> bool {
    mml_net::get_work_client()
        .get_bytes("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .await
        .is_ok()
}

/// 下载真实 Modrinth 整合包到临时缓存，按 slug 加版本缓存，跨进程复用
///
/// 版本号硬编码：断言依赖该版本的具体字段，如 26.2 / fabric-loader 0.19.3，
/// 跟随最新版会随上游变动而失效。下载失败，如网络不可用，返回 `None`。
async fn download_modrinth_pack(slug: &str, version: &str) -> Option<PathBuf> {
    use mml_base::serialize_tools::json_from_bytes;
    use mml_net::modrinth_api::version_obj::ModrinthVersionObj;

    let cache = mml_testutil::temp_dir().join(format!("mml-real-pack-{slug}-{version}.mrpack"));
    if cache.exists() {
        return Some(cache);
    }

    let data = match mml_net::get_work_client()
        .get_bytes(&format!(
            "https://api.modrinth.com/v2/project/{slug}/version"
        ))
        .await
    {
        Ok(data) => data,
        Err(err) => {
            eprintln!("下载 {slug} 版本列表失败: {err}");
            return None;
        }
    };
    let versions: Vec<ModrinthVersionObj> = match json_from_bytes(&data) {
        Ok(versions) => versions,
        Err(err) => {
            eprintln!("解析 {slug} 版本列表失败: {err}");
            return None;
        }
    };
    let Some(target) = versions.into_iter().find(|v| v.version_number == version) else {
        eprintln!("{slug} 上不存在版本 {version}");
        return None;
    };
    let Some(file) = target
        .files
        .into_iter()
        .find(|f| f.primary || !f.url.is_empty())
    else {
        eprintln!("{slug} {version} 没有可下载文件");
        return None;
    };

    let data = match mml_net::get_work_client().get_bytes(&file.url).await {
        Ok(data) => data,
        Err(err) => {
            eprintln!("下载 {slug} 整合包失败: {err}");
            return None;
        }
    };
    if data.is_empty() {
        eprintln!("{slug} 整合包内容为空");
        return None;
    }
    std::fs::write(&cache, &data).ok()?;
    Some(cache)
}

/// 安装阶段记录器：供断言走完所有阶段
#[derive(Default, Clone)]
struct StateRecorder {
    /// 按发生顺序记录到的阶段 id，取值同 `state_id` 的输出
    states: Arc<Mutex<Vec<&'static str>>>,
}

impl StateRecorder {
    /// 该阶段是否已经记录到
    fn reached(&self, state: &str) -> bool {
        self.states.lock().unwrap().contains(&state)
    }
}

/// 把安装阶段映射成稳定的短 id，供断言比较
fn state_id(state: &AddModPackState) -> &'static str {
    match state {
        AddModPackState::DownloadPack => "downloadPack",
        AddModPackState::ReadInfo => "readInfo",
        AddModPackState::GetInfo => "getInfo",
        AddModPackState::DownloadFile => "downloadFile",
        AddModPackState::Extract => "extract",
        AddModPackState::Done => "done",
    }
}

impl IAddModPackGui for StateRecorder {
    /// 记录一个到达的安装阶段
    fn set_state(&self, state: AddModPackState) {
        self.states.lock().unwrap().push(state_id(&state));
    }

    /// 进度数值：本记录器不关心
    fn set_now(&self, _value: usize, _all: Option<usize>) {}

    /// 子进度文案：本记录器不关心
    fn set_sub_text(&self, _text: Option<String>) {}

    /// 子进度数值：本记录器不关心
    fn set_sub_now(&self, _value: usize, _all: Option<usize>) {}
}

/// 下载界面桩：只把进度打到测试输出
struct TestDownloader;

impl mml_downloader::IDownloadGui for TestDownloader {
    /// 每完成 25% 打一行进度，避免刷屏
    fn update(&self, thread: u32, file: &Arc<DownloadItem>) {
        let pro = file.progress() as u64;
        if pro > 0 && pro % 25 == 0 {
            println!("[下载] 线程 {thread} {} {}%", file.base.name, pro);
        }
    }

    /// 任务增删与进度变化各打一行
    fn update_task(&self, state: mml_downloader::DownloadTaskState) {
        match state {
            mml_downloader::DownloadTaskState::AddTask(id) => println!("[下载] 新任务 {id}"),
            mml_downloader::DownloadTaskState::RemoveTask(id) => println!("[下载] 任务结束 {id}"),
            mml_downloader::DownloadTaskState::UpdateTask(obj) => {
                println!("[下载] 任务 {} 进度 {:.1}%", obj.id, obj.progress)
            }
        }
    }
}

/// 真实 Modrinth 整合包完整安装，含全部 mod 文件下载，约 40+ 文件
/// Fabulously Optimized：Fabric 加载器。
#[tokio::test]
async fn install_real_modrinth_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    // 真包从 Modrinth 下载并缓存，无网跳过
    let Some(pack) = download_modrinth_pack("fabulously-optimized", "14.0.0-beta.3").await else {
        eprintln!("跳过: 网络不可用（无法下载 Modrinth 整合包）");
        return;
    };

    let recorder = StateRecorder::default();
    let uuid = add_game::install_archive_from_file(
        &pack,
        None,
        None,
        None,
        None,
        Some(Arc::new(recorder.clone())),
        None,
        PackType::Modrinth,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("真实 Modrinth 整合包安装失败");

    // 包元数据：minecraft 26.2 + fabric-loader 0.19.3
    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert!(!read.name.is_empty());
    assert_eq!(read.version, "26.2");
    assert!(matches!(read.loader, LoaderType::Fabric));
    assert_eq!(read.loader_version.as_deref(), Some("0.19.3"));
    assert!(read.is_modpack);
    drop(read);

    // 包内清单的全部 mod 都下载到了 mods 目录，约 48 个文件
    let game = mml_game::get_instance(&uuid).unwrap();
    let mods = game.read().unwrap().get_mods_path();
    let count = std::fs::read_dir(&mods)
        .expect("mods 目录应存在")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jar"))
        .count();
    assert!(count >= 40, "mods 目录应下载 40+ 个 jar，实际 {count}");

    for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// 真实 Modrinth 整合包完整安装：NeoForge 加载器。
/// Optimized FPS 4.3.0：17 个 mod 文件，约 12MB，纯 NeoForge 包
#[tokio::test]
async fn install_real_modrinth_neoforge_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = download_modrinth_pack("optimized-fps", "4.3.0").await else {
        eprintln!("跳过: 网络不可用（无法下载 Modrinth 整合包）");
        return;
    };

    let recorder = StateRecorder::default();
    let uuid = add_game::install_archive_from_file(
        &pack,
        None,
        None,
        None,
        None,
        Some(Arc::new(recorder.clone())),
        None,
        PackType::Modrinth,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("真实 NeoForge 整合包安装失败");

    // 包元数据：minecraft 26.2 + neoforge 26.2.0.75
    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.version, "26.2");
    assert!(matches!(read.loader, LoaderType::NeoForge));
    assert_eq!(read.loader_version.as_deref(), Some("26.2.0.75"));
    assert!(read.is_modpack);

    // 包内清单的 17 个 mod 都下载到了 mods 目录
    let mods = read.get_game_path().join("mods");
    let count = std::fs::read_dir(&mods)
        .expect("mods 目录应存在")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jar"))
        .count();
    assert!(count >= 15, "mods 目录应下载 15+ 个 jar，实际 {count}");
    drop(read);

    for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// 真实 CurseForge 整合包完整安装，手动测试
///
/// API key 由环境变量 `MML_CF_API_KEY` 注入，GUI 运行时也是同一个 key，
/// 先设置环境变量再手动运行：
///
/// ```bash
/// MML_CF_API_KEY='key' cargo test -p mml-game --test real_pack_download \
///     install_real_curseforge_pack -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn install_real_curseforge_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if mml_net::curseforge_api::get_key().is_err() {
        eprintln!("跳过: 未设置 MML_CF_API_KEY 环境变量（CurseForge API 需要 key）");
        return;
    }

    if !network_available().await {
        eprintln!("跳过: 网络不可用");
        return;
    }

    // 搜索整合包并取其最新文件，体积较小的 Fabulously Optimized
    let list =
        mml_net::curseforge_api::get_modpack_list(mml_net::curseforge_api::CurseFogreArg {
            filter: Some("Fabulously Optimized".to_string()),
            page_size: Some(5),
            ..Default::default()
        })
        .await
        .expect("搜索 CurseForge 整合包失败");
    let Some(item) = list.data.first() else {
        eprintln!("跳过: 搜索结果为空");
        return;
    };
    println!("选中整合包: {} (id={})", item.name, item.id);

    let mut files =
        mml_net::curseforge_api::get_files_page(mml_net::curseforge_api::CurseFogreArg {
            id: Some(item.id.to_string()),
            page_size: Some(1),
            ..Default::default()
        })
        .await
        .expect("获取 CurseForge 文件列表失败");
    let Some(mut file) = files.data.first_mut() else {
        eprintln!("跳过: 整合包没有文件");
        return;
    };
    println!("选中文件: {} (id={})", file.display_name, file.id);

    let recorder = StateRecorder::default();
    let uuid = add_game::install_curseforge(
        &mut file,
        None,
        None,
        None,
        Some(Arc::new(recorder.clone())),
        None,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("真实 CurseForge 整合包安装失败");

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert!(!read.name.is_empty());
    assert!(read.is_modpack);
    drop(read);

    for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// 按加载器在线搜索 CurseForge 整合包并下载安装，Fabric 与 NeoForge 共用
///
/// 搜索时按 modLoaderType 过滤，按下载量排序取前 10 个，
/// 依次找最新文件小于 150MB 的包安装，避免测试耗时过长。
async fn install_cf_pack_with_loader(loader: u32, loader_name: &str, expect_loader: LoaderType) {
    let list = curseforge_api::get_modpack_list(curseforge_api::CurseFogreArg {
        page_size: Some(10),
        sort: Some(CurseForgeSortType::TotalDownloads),
        loader: Some(loader),
        ..Default::default()
    })
    .await
    .expect("搜索 CurseForge 整合包失败");
    assert!(!list.data.is_empty(), "按 {loader_name} 搜索整合包结果为空");

    for item in &list.data {
        let mut files = curseforge_api::get_files_page(curseforge_api::CurseFogreArg {
            id: Some(item.id.to_string()),
            page_size: Some(1),
            ..Default::default()
        })
        .await
        .expect("获取 CurseForge 文件列表失败");
        let Some(file) = files.data.first_mut() else {
            continue;
        };
        if file.file_length > 150 * 1024 * 1024 {
            println!(
                "跳过大包: {} ({:.0}MB)",
                item.name,
                file.file_length as f64 / 1048576.0
            );
            continue;
        }
        println!(
            "选中 {} 整合包: {} (id={}) 文件 {} (id={}, {:.1}MB)",
            loader_name,
            item.name,
            item.id,
            file.display_name,
            file.id,
            file.file_length as f64 / 1048576.0
        );

        let recorder = StateRecorder::default();
        let uuid = add_game::install_curseforge(
            file,
            None,
            None,
            None,
            Some(Arc::new(recorder.clone())),
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("真实 CurseForge 整合包安装失败");

        let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
        let read = game.read().unwrap();
        assert!(!read.name.is_empty());
        assert!(read.is_modpack);
        assert!(
            std::mem::discriminant(&read.loader) == std::mem::discriminant(&expect_loader),
            "加载器类型应为 {loader_name}"
        );
        assert!(!read.version.is_empty(), "应从 manifest 取到游戏版本");
        drop(read);

        for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
            assert!(recorder.reached(state), "缺少安装阶段: {state}");
        }

        mml_game::delete_instance(&uuid).unwrap();
        return;
    }
    panic!("前 10 个 {loader_name} 整合包都没有小于 150MB 的最新文件");
}

/// CurseForge 真包在线安装：Fabric 加载器，手动测试，key 与网络要求同
/// [`install_real_curseforge_pack`]
#[tokio::test]
#[ignore]
async fn install_real_curseforge_fabric_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if mml_net::curseforge_api::get_key().is_err() {
        eprintln!("跳过: 未设置 MML_CF_API_KEY 环境变量（CurseForge API 需要 key）");
        return;
    }
    if !network_available().await {
        eprintln!("跳过: 网络不可用");
        return;
    }

    install_cf_pack_with_loader(
        mml_net::curseforge_api::MODLOADER_FABRIC,
        "Fabric",
        LoaderType::Fabric,
    )
    .await;
}

/// CurseForge 真包在线安装：NeoForge 加载器，手动测试，key 与网络要求同
/// [`install_real_curseforge_pack`]
#[tokio::test]
#[ignore]
async fn install_real_curseforge_neoforge_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if mml_net::curseforge_api::get_key().is_err() {
        eprintln!("跳过: 未设置 MML_CF_API_KEY 环境变量（CurseForge API 需要 key）");
        return;
    }
    if !network_available().await {
        eprintln!("跳过: 网络不可用");
        return;
    }

    install_cf_pack_with_loader(
        mml_net::curseforge_api::MODLOADER_NEOFORGE,
        "NeoForge",
        LoaderType::NeoForge,
    )
    .await;
}

/// 真实 Modrinth 大体积整合包，手动测试，约 310MB / 176 个文件
/// Fresh & Smooth：Fabric 加载器，文件分布在 mods / resourcepacks / shaderpacks
/// 多个子目录，验证多目录安装。
///
/// ```bash
/// cargo test -p mml-game --test real_pack_download install_real_modrinth_heavy_pack \
///     -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn install_real_modrinth_heavy_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = download_modrinth_pack("fresh-smooth", "2.9.5+26.2").await else {
        eprintln!("跳过: 网络不可用（无法下载 Modrinth 整合包）");
        return;
    };

    let recorder = StateRecorder::default();
    let uuid = add_game::install_archive_from_file(
        &pack,
        None,
        None,
        None,
        None,
        Some(Arc::new(recorder.clone())),
        None,
        PackType::Modrinth,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("真实大体积整合包安装失败");

    // 包元数据：minecraft 26.2 + fabric-loader 0.19.3
    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.version, "26.2");
    assert!(matches!(read.loader, LoaderType::Fabric));
    assert_eq!(read.loader_version.as_deref(), Some("0.19.3"));

    // 包内 112 个 mod + 60+ 资源包/光影包分布在多个子目录
    let game_path = read.get_game_path();
    let count_jars = |dir: &str| -> usize {
        std::fs::read_dir(game_path.join(dir))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".jar"))
                    .count()
            })
            .unwrap_or(0)
    };
    assert!(count_jars("mods") >= 100, "mods 目录应下载 100+ 个 jar");
    assert!(
        count_jars("resourcepacks") >= 20,
        "resourcepacks 应下载 20+ 个文件"
    );
    drop(read);

    for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}
