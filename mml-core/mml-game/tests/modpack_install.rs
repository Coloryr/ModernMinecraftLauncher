//! 整合包安装链路集成测试：程序化构造 CurseForge / Modrinth 整合包压缩包，
//! `files` 清单留空以免依赖 CurseForge API，走完整安装链
//! readInfo → readVersion → createInstance → extract → getInfo → download → done，
//! 校验实例创建、元数据写入与 overrides 解压。
//!
//! 版本 json 校验（`check_update`）需要联网下载 Mojang 版本清单，
//! 网络不可用时跳过，与 tests/common 的约定一致。
//! 全局初始化每进程只能执行一次，用互斥锁串行各用例。

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};

mod common;

use mml_downloader::download_item::DownloadItem;
use mml_game::add_game::{self, PackType};
use mml_game::gui_hook::{AddModPackState, IAddInstanceGui, IAddModPackGui};
use mml_game::launcher::ModPackType;
use mml_game::loader::LoaderType;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// 测试运行目录，用进程号区分多进程
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join(format!("mml-modpack-install-{}", std::process::id()))
}

/// 初始化链（与 mml_core::init 相同），每进程一次
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
        mml_net::init();

        mml_downloader::init(&dir).unwrap();
        mml_downloader::set_gui_handel(Box::new(TestDownloader));
        mml_downloader::start();
    });
}

/// 网络探测：Mojang 版本清单可达才继续，`check_update` 依赖它
async fn network_available() -> bool {
    let ok = mml_net::get_work_client()
        .get_bytes("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .await
        .map(|data| !data.is_empty())
        .unwrap_or(false);
    if !ok {
        eprintln!("跳过: 网络不可用（无法下载 Mojang 版本清单）");
    }
    ok
}

/// 全局状态共享同一进程，串行执行各用例
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 程序化生成一个 zip 压缩包
fn make_zip(dir: &PathBuf, stem: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let zip_path = dir.join(format!("{stem}.zip"));
    let file = std::fs::File::create(&zip_path).expect("创建测试压缩包失败");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, content) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap();
    zip_path
}

/// 实例回调：允许自动改名、拒绝覆盖
struct TestGui;

#[async_trait::async_trait]
impl IAddInstanceGui for TestGui {
    /// 名字被占用时一律允许自动改名
    async fn name_replace(&self, _name: &str) -> bool {
        true
    }

    /// 一律拒绝覆盖已有实例，逼安装流程走自动改名
    async fn overwrite(&self, _obj: mml_game::GameInstance) -> bool {
        false
    }
}

/// 安装阶段记录器，供断言走完所有阶段
#[derive(Default, Clone)]
struct StateRecorder {
    /// 已经到达过的阶段 id，按到达顺序追加
    states: Arc<Mutex<Vec<&'static str>>>,
}

impl StateRecorder {
    /// 判断某个阶段是否到达过
    ///
    /// - `state`: 阶段 id，取值见 [`state_id`]
    ///
    /// # 返回值
    ///
    /// 到达过返回 `true`
    fn reached(&self, state: &str) -> bool {
        self.states.lock().unwrap().contains(&state)
    }
}

/// 把安装阶段映射成断言用的短 id
///
/// - `state`: 安装阶段枚举
///
/// # 返回值
///
/// 返回阶段 id，如 `readInfo` / `extract`
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
    /// 记下到达的阶段
    fn set_state(&self, state: AddModPackState) {
        self.states.lock().unwrap().push(state_id(&state));
    }

    /// 总体进度桩，测试不校验
    fn set_now(&self, _value: usize, _all: Option<usize>) {}

    /// 子标题桩，测试不校验
    fn set_sub_text(&self, _text: Option<String>) {}

    /// 子进度桩，测试不校验
    fn set_sub_now(&self, _value: usize, _all: Option<usize>) {}
}

/// 下载回调桩，测试里什么都不做
struct TestDownloader;

impl mml_downloader::IDownloadGui for TestDownloader {
    /// 单文件进度桩，测试不校验
    fn update(&self, _thread: u32, _file: &Arc<DownloadItem>) {}

    /// 任务状态桩，测试不校验
    fn update_task(&self, _state: mml_downloader::DownloadTaskState) {}
}

/// 构造一个 CurseForge 整合包，空文件清单加 overrides
fn make_curseforge_pack(name: &str) -> (PathBuf, PathBuf) {
    let dir = mml_testutil::temp_dir()
        .join("mml-modpack-install-packs")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();

    let manifest = format!(
        r#"{{"minecraft":{{"version":"1.21.1","modLoaders":[{{"id":"forge-41.0.100","primary":true}}]}},"manifestType":"minecraftModpack","manifestVersion":1,"name":"{name}","version":"1.0","author":"test","files":[],"overrides":"overrides"}}"#
    );
    let zip = make_zip(
        &dir,
        "cf-pack",
        &[
            ("manifest.json", manifest.as_bytes()),
            ("overrides/options.txt", b"lang:zh_cn\n"),
            ("overrides/mods/dummy.jar", b"dummy mod jar"),
        ],
    );
    (zip, dir)
}

/// 构造一个 Modrinth 整合包，空文件清单加 overrides
fn make_modrinth_pack(name: &str) -> (PathBuf, PathBuf) {
    let dir = mml_testutil::temp_dir()
        .join("mml-modpack-install-packs")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();

    let index = format!(
        r#"{{"formatVersion":1,"game":"minecraft","versionId":"1.0.0","name":"{name}","files":[],"dependencies":{{"minecraft":"1.20.1","fabric-loader":"0.15.11"}}}}"#
    );
    let zip = make_zip(
        &dir,
        "mr-pack",
        &[
            ("modrinth.index.json", index.as_bytes()),
            ("overrides/mods/mr-dummy.jar", b"dummy mod jar"),
        ],
    );
    (zip, dir)
}

/// CurseForge 整合包完整安装：实例创建、元数据、overrides 解压、重名自动改名
#[tokio::test]
async fn install_curseforge_pack_end_to_end() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if !network_available().await {
        return;
    }

    let (zip, pack_dir) = make_curseforge_pack("CF安装测试包");

    // 类型检测
    let detected = add_game::detect_pack(&zip).expect("类型检测失败");
    assert!(matches!(detected.pack_type, PackType::CurseForge));
    assert_eq!(detected.name, "CF安装测试包");

    let recorder = StateRecorder::default();
    let token = CancellationToken::new();
    // 目标分组：安装流程按 uuid 收，先建好组
    let group = mml_game::add_group("e2e组").expect("建组失败");
    let uuid = add_game::install_archive_from_file(
        &zip,
        Some("cf-e2e-实例".to_string()),
        Some(group),
        None,
        Some(Arc::new(TestGui)),
        Some(Arc::new(recorder.clone())),
        None,
        PackType::CurseForge,
        token,
    )
    .await
    .expect("CurseForge 整合包安装失败");

    // 实例元数据
    let instance = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let game = instance.read().unwrap();
    assert_eq!(game.name, "cf-e2e-实例");
    assert_eq!(game.version, "1.21.1");
    assert!(game.is_modpack);
    assert!(matches!(game.loader, LoaderType::Forge));
    assert_eq!(game.loader_version.as_deref(), Some("41.0.100"));
    assert!(matches!(game.modpack_type, ModPackType::CurseForge));
    drop(game);

    // 分组不在实例配置里：归属得去内核分组表查，安装时选的分组不能丢
    assert_eq!(mml_game::get_instance_group(&uuid), Some(group));

    // overrides 解压到游戏目录
    let game_path = mml_game::get_instance(&uuid)
        .unwrap()
        .read()
        .unwrap()
        .get_game_path();
    let options = std::fs::read_to_string(game_path.join("options.txt")).unwrap();
    assert_eq!(options, "lang:zh_cn\n");
    let jar = std::fs::read(game_path.join("mods").join("dummy.jar")).unwrap();
    assert_eq!(jar, b"dummy mod jar");

    // 全部安装阶段走完
    for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    // 同名再装一次：拒绝覆盖 + 允许改名 → 自动改名 "cf-e2e-实例1"
    let uuid2 = add_game::install_archive_from_file(
        &zip,
        Some("cf-e2e-实例".to_string()),
        None,
        None,
        Some(Arc::new(TestGui)),
        Some(Arc::new(StateRecorder::default())),
        None,
        PackType::CurseForge,
        CancellationToken::new(),
    )
    .await
    .expect("第二次安装失败");
    let name2 = mml_game::get_instance(&uuid2)
        .unwrap()
        .read()
        .unwrap()
        .name
        .clone();
    assert_eq!(name2, "cf-e2e-实例1", "重名实例应自动改名");

    // 清理
    mml_game::delete_instance(&uuid).unwrap();
    mml_game::delete_instance(&uuid2).unwrap();
    assert!(mml_game::get_instance(&uuid).is_none());
    assert!(mml_game::get_instance(&uuid2).is_none());

    let _ = std::fs::remove_dir_all(&pack_dir);
}

/// Modrinth 整合包完整安装：未传名字时取元数据里的 `name-versionId`
#[tokio::test]
async fn install_modrinth_pack_end_to_end() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if !network_available().await {
        return;
    }

    let (zip, pack_dir) = make_modrinth_pack("MR安装测试包");

    let detected = add_game::detect_pack(&zip).expect("类型检测失败");
    assert!(matches!(detected.pack_type, PackType::Modrinth));
    assert_eq!(detected.name, "MR安装测试包");

    let recorder = StateRecorder::default();
    let uuid = add_game::install_archive_from_file(
        &zip,
        None,
        None,
        None,
        Some(Arc::new(TestGui)),
        Some(Arc::new(recorder.clone())),
        None,
        PackType::Modrinth,
        CancellationToken::new(),
    )
    .await
    .expect("Modrinth 整合包安装失败");

    let instance = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let game = instance.read().unwrap();
    assert_eq!(game.name, "MR安装测试包-1.0.0");
    assert_eq!(game.version, "1.20.1");
    assert!(game.is_modpack);
    assert!(matches!(game.loader, LoaderType::Fabric));
    assert_eq!(game.loader_version.as_deref(), Some("0.15.11"));
    assert!(matches!(game.modpack_type, ModPackType::Modrinth));
    drop(game);

    let game_path = mml_game::get_instance(&uuid)
        .unwrap()
        .read()
        .unwrap()
        .get_game_path();
    let jar = std::fs::read(game_path.join("mods").join("mr-dummy.jar")).unwrap();
    assert_eq!(jar, b"dummy mod jar");

    for state in ["readInfo", "extract", "getInfo", "downloadFile", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    assert!(mml_game::get_instance(&uuid).is_none());

    let _ = std::fs::remove_dir_all(&pack_dir);
}

/// 取消令牌预取消：安装返回 TaskCancel，且不残留实例
#[tokio::test]
async fn install_curseforge_cancelled_cleans_up() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    if !network_available().await {
        return;
    }

    let (zip, pack_dir) = make_curseforge_pack("CF取消测试包");
    let before = mml_game::get_instances().len();

    let token = CancellationToken::new();
    token.cancel();

    let res = add_game::install_archive_from_file(
        &zip,
        Some("cf-cancel-实例".to_string()),
        None,
        None,
        Some(Arc::new(TestGui)),
        Some(Arc::new(StateRecorder::default())),
        None,
        PackType::CurseForge,
        token,
    )
    .await;

    assert!(res.is_err(), "预取消的安装应失败");
    assert_eq!(
        mml_game::get_instances().len(),
        before,
        "取消后不应残留实例"
    );

    let _ = std::fs::remove_dir_all(&pack_dir);
}
