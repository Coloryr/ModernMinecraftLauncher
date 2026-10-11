//! 其他启动器导出包的安装测试，离线，程序化构造假包：
//! HMCL 的 mcbbs.packmeta、HMCL 服务器包的 server-manifest.json、
//! MMC / Prism 的 mmc-pack.json + instance.cfg、game.json 直接解压包、
//! 其他启动器 PCL 导出的 `.minecraft` 目录，版本隔离 / 非隔离两种布局。
//!
//! 全部纯离线：这些包类型不经过 `check_update`，不需要网络。
//! 全局初始化每进程一次，互斥锁串行各用例。

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};

use mml_game::add_game::{self, PackType};
use mml_game::gui_hook::{AddModPackState, IAddModPackGui};
use mml_game::launcher::ModPackType;
use mml_game::loader::LoaderType;
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

mod common;
use common::{ZipEntry, entry};

/// 测试运行目录，mml_testutil 临时目录下的进程号子目录，避免多进程冲突
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join(format!("mml-launcher-packs-{}", std::process::id()))
}

/// 初始化链与 mml_core::init 相同，每进程一次
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

/// 全局状态共享同一进程，串行执行各用例
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 假包输出目录
fn pack_dir() -> PathBuf {
    let dir = mml_testutil::temp_dir()
        .join("mml-launcher-packs-packs")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 程序化生成一个 zip 压缩包
///
/// - `dir`: 输出目录
/// - `stem`: 压缩包文件名，不带扩展名
/// - `files`: 条目路径与原始内容
///
/// # 返回值
///
/// 生成的压缩包路径
fn make_zip(dir: &PathBuf, stem: &str, files: &[ZipEntry]) -> PathBuf {
    let zip_path = dir.join(format!("{stem}.zip"));
    let file = std::fs::File::create(&zip_path).expect("创建测试压缩包失败");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for e in files {
        writer.start_file(e.name, options).unwrap();
        writer.write_all(e.data).unwrap();
    }
    writer.finish().unwrap();
    zip_path
}

/// 安装阶段记录器，供断言走完所有阶段
#[derive(Default, Clone)]
struct StateRecorder {
    /// 已走过的阶段 ID，按到达顺序追加
    states: Arc<Mutex<Vec<&'static str>>>,
}

impl StateRecorder {
    /// 该阶段是否已经走到
    fn reached(&self, state: &str) -> bool {
        self.states.lock().unwrap().contains(&state)
    }
}

/// 把安装状态映射成前端使用的 ID
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
    fn set_state(&self, state: AddModPackState) {
        self.states.lock().unwrap().push(state_id(&state));
    }

    fn set_now(&self, _value: usize, _all: Option<usize>) {}

    fn set_sub_text(&self, _text: Option<String>) {}

    fn set_sub_now(&self, _value: usize, _all: Option<usize>) {}
}

/// 一次安装的产物
struct InstalledPack {
    /// 新建实例的 uuid
    uuid: Uuid,
    /// 安装阶段记录器，与传给安装流程的是同一份
    recorder: StateRecorder,
}

/// 通用安装入口：无界面回调，重名自动改名
///
/// # 返回值
///
/// 见 [`InstalledPack`]；安装失败会 panic
async fn install(zip: &PathBuf, name: Option<String>, pack_type: PackType) -> InstalledPack {
    let recorder = StateRecorder::default();
    let uuid = add_game::install_archive_from_file(
        zip,
        name,
        None,
        None,
        None,
        Some(Arc::new(recorder.clone())),
        None,
        pack_type,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("安装失败");
    InstalledPack { uuid, recorder }
}

/// 断言读到的文件内容
fn assert_file(path: std::path::PathBuf, expected: &[u8], what: &str) {
    assert_eq!(std::fs::read(path).expect(what), expected, "{what}");
}

/// HMCL 导出包：`mcbbs.packmeta` 提供名字/版本/加载器，`manifest.json` 声明 overrides 目录。
/// 文件在 `minecraft/` 下，解压后去掉该层放进实例基础目录。
#[tokio::test]
async fn install_hmcl_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = pack_dir();
    let packmeta =
        r#"{"name":"HMCL测试包","files":[],"addons":[{"id":"game","version":"1.20.1"},{"id":"forge","version":"47.3.0"}]}"#;
    let manifest = r#"{"name":"HMCL测试包","manifestType":"minecraftModpack","manifestVersion":1,"overrides":"minecraft","files":[],"minecraft":{"version":"1.20.1","modLoaders":[]}}"#;
    let zip = make_zip(
        &dir,
        "hmcl-pack",
        &[
            entry("mcbbs.packmeta", packmeta.as_bytes()),
            entry("manifest.json", manifest.as_bytes()),
            entry("minecraft/options.txt", b"lang:zh_cn\n"),
            entry("minecraft/mods/dummy.jar", b"dummy mod jar"),
        ],
    );

    // 类型检测
    let detected = add_game::detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::HMCL));
    assert_eq!(detected.name, "HMCL测试包");

    let InstalledPack { uuid, recorder } = install(&zip, None, PackType::HMCL).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, "HMCL测试包");
    assert_eq!(read.version, "1.20.1");
    assert!(matches!(read.loader, LoaderType::Forge));
    assert_eq!(read.loader_version.as_deref(), Some("47.3.0"));

    // minecraft/ 层被剥离，文件落到实例基础目录
    assert_file(read.get_base_path().join("options.txt"), b"lang:zh_cn\n", "options.txt 应解压");
    assert_file(
        read.get_base_path().join("mods").join("dummy.jar"),
        b"dummy mod jar",
        "mod 应解压",
    );
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// HMCL 服务器包：`server-manifest.json`，文件在 `overrides/` 下，落到实例基础目录。
#[tokio::test]
async fn install_hmcl_server_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = pack_dir();
    let manifest = r#"{"name":"HMS测试包","author":"test","version":"1.0","description":"","fileApi":"","files":[],"addons":[{"id":"game","version":"1.21.1"}]}"#;
    let zip = make_zip(
        &dir,
        "hms-pack",
        &[
            entry("server-manifest.json", manifest.as_bytes()),
            entry("overrides/server.properties", b"online-mode=false\n"),
            entry("overrides/mods/dummy.jar", b"dummy mod jar"),
        ],
    );

    let detected = add_game::detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::HMCLServer));

    let InstalledPack { uuid, recorder } = install(&zip, None, PackType::HMCLServer).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, "HMS测试包");
    assert_eq!(read.version, "1.21.1");
    assert!(read.is_modpack);
    assert!(matches!(read.modpack_type, ModPackType::ServerPack));

    assert_file(
        read.get_base_path().join("server.properties"),
        b"online-mode=false\n",
        "server.properties 应解压",
    );
    assert_file(
        read.get_base_path().join("mods").join("dummy.jar"),
        b"dummy mod jar",
        "mod 应解压",
    );
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// MMC / Prism 导出包：`mmc-pack.json` 的 components 声明版本与加载器，
/// `instance.cfg` 提供实例名，游戏文件在 `.minecraft/` 下落到实例基础目录。
#[tokio::test]
async fn install_mmc_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = pack_dir();
    let mmc = r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.15.11"}]}"#;
    let cfg = "name=MMC测试包\nInstanceType=OneSix\n";
    let zip = make_zip(
        &dir,
        "mmc-pack",
        &[
            entry("mmcinst/mmc-pack.json", mmc.as_bytes()),
            entry("mmcinst/instance.cfg", cfg.as_bytes()),
            entry("mmcinst/.minecraft/mods/dummy.jar", b"dummy mod jar"),
            entry("mmcinst/.minecraft/options.txt", b"lang:zh_cn\n"),
        ],
    );

    let detected = add_game::detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::MMC));
    assert_eq!(detected.name, "MMC测试包");

    let InstalledPack { uuid, recorder } = install(&zip, None, PackType::MMC).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, "MMC测试包");
    assert_eq!(read.version, "1.20.1");
    assert!(matches!(read.loader, LoaderType::Fabric));
    assert_eq!(read.loader_version.as_deref(), Some("0.15.11"));

    // mmcinst/ 层被剥离，.minecraft 内容落到实例基础目录的 .minecraft
    assert_file(
        read.get_base_path().join(".minecraft").join("options.txt"),
        b"lang:zh_cn\n",
        "options.txt 应解压",
    );
    assert_file(
        read.get_base_path().join(".minecraft").join("mods").join("dummy.jar"),
        b"dummy mod jar",
        "mod 应解压",
    );
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// game.json 直接解压包：实例配置随包携带，文件去包裹层后整个解压。
#[tokio::test]
async fn install_game_json_archive() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = pack_dir();
    let game_json = r#"{"Name":"占位"}"#;
    let zip = make_zip(
        &dir,
        "plain-pack",
        &[
            entry("pack/game.json", game_json.as_bytes()),
            entry("pack/options.txt", b"lang:zh_cn\n"),
            entry("pack/mods/dummy.jar", b"dummy mod jar"),
        ],
    );

    let detected = add_game::detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::ArchivePack));

    // 传入的名字覆盖包内配置的名字
    let InstalledPack { uuid, recorder } =
        install(&zip, Some("直接解压导入".to_string()), PackType::ArchivePack).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, "直接解压导入");

    // pack/ 层被剥离
    assert_file(read.get_base_path().join("options.txt"), b"lang:zh_cn\n", "options.txt 应解压");
    assert_file(
        read.get_base_path().join("mods").join("dummy.jar"),
        b"dummy mod jar",
        "mod 应解压",
    );
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// PCL 等启动器导出的 `.minecraft` 目录，版本隔离布局：
/// 版本 json 在 `versions/{name}/` 下且该目录有游戏资源，只导入版本文件夹内容。
#[tokio::test]
async fn install_launcher_pack_isolated() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = pack_dir();
    let version_name = "PCL测试版本";
    let version_json = r#"{"id":"PCL测试版本"}"#;
    let json_entry = format!(".minecraft/versions/{version_name}/{version_name}.json");
    let zip = make_zip(
        &dir,
        "pcl-pack",
        &[
            entry(json_entry.as_str(), version_json.as_bytes()),
            entry(
                format!(".minecraft/versions/{version_name}/options.txt").as_str(),
                b"lang:zh_cn\n",
            ),
            entry(
                format!(".minecraft/versions/{version_name}/mods/dummy.jar").as_str(),
                b"dummy mod jar",
            ),
        ],
    );

    let detected = add_game::detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::LauncherPack));

    let InstalledPack { uuid, recorder } =
        install(&zip, Some("PCL导入".to_string()), PackType::LauncherPack).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, "PCL导入");
    // 版本号取版本文件夹名，不在版本清单里时回退 json 的 id
    assert_eq!(read.version, version_name);

    // 版本文件夹内的游戏资源导入游戏目录
    assert_file(read.get_game_path().join("options.txt"), b"lang:zh_cn\n", "options.txt 应导入");
    assert_file(
        read.get_game_path().join("mods").join("dummy.jar"),
        b"dummy mod jar",
        "mod 应导入",
    );
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// 其他启动器导出的 `.minecraft` 目录，非隔离布局：
/// 版本 json 直接在 `.minecraft` 根下，整个 `.minecraft` 内容导入游戏目录。
#[tokio::test]
async fn install_launcher_pack_not_isolated() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let dir = pack_dir();
    let zip = make_zip(
        &dir,
        "plain-mc-pack",
        &[
            entry(".minecraft/1.20.1.json", r#"{"id":"1.20.1"}"#.as_bytes()),
            entry(".minecraft/mods/dummy.jar", b"dummy mod jar"),
            entry(".minecraft/options.txt", b"lang:zh_cn\n"),
        ],
    );

    let detected = add_game::detect_pack(&zip).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::LauncherPack));

    let InstalledPack { uuid, recorder } = install(&zip, None, PackType::LauncherPack).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    // 未传名字时取版本 json 的 id
    assert_eq!(read.name, "1.20.1");
    assert_eq!(read.version, "1.20.1");

    assert_file(read.get_game_path().join("options.txt"), b"lang:zh_cn\n", "options.txt 应导入");
    assert_file(
        read.get_game_path().join("mods").join("dummy.jar"),
        b"dummy mod jar",
        "mod 应导入",
    );
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
