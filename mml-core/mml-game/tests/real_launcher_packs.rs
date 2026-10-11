//! 真实启动器导出包的安装测试。
//!
//! - M²L 即 ColorMC 实例压缩包：核心导出 API 做导出到导入的 roundtrip，离线可跑，
//!   另有真实导出的 colormc-instance.zip 用例。
//! - HMCL / MMC(Prism) 导出包、HMCL 整包、MultiMC 整包：真实启动器生成的压缩包，
//!   放到 `tests/packs/` 或环境变量 `MML_REAL_PACK_DIR` 指定的目录，文件不存在时
//!   自动跳过。断言基于包内元数据动态推导，不写死版本号。
//!
//! 全局初始化每进程一次，互斥锁串行各用例。

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};

use mml_base::archives::{ArchiveType, BaseArchive};
use mml_game::add_game::{
    self, PackType, detect_pack, find_minecraft_prefix, pick_primary, scan_versions,
    version_folder_has_data,
};
use mml_game::game_export::{ExportArg, ExportPackType};
use mml_game::gui_hook::{AddModPackState, IAddModPackGui};
use mml_game::loader::LoaderType;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_names::names as name_consts;
use uuid::Uuid;

mod common;

/// 真实导出包存放目录，取环境变量 `MML_REAL_PACK_DIR`，未设置时用 `tests/packs/`
fn pack_dir() -> PathBuf {
    std::env::var("MML_REAL_PACK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/packs"))
}

/// 查找真实导出包，不存在时打印生成说明并返回 `None`，用例据此跳过
///
/// - `file_name`: 包文件名
/// - `how_to_make`: 生成方式的说明文本，仅在缺包时打印
///
/// # 返回值
///
/// 返回包文件路径；文件不存在时返回 `None`
fn find_pack(file_name: &str, how_to_make: &str) -> Option<PathBuf> {
    let path = pack_dir().join(file_name);
    if path.exists() {
        return Some(path);
    }
    eprintln!("跳过: 缺少 {file_name}（{how_to_make}），放到 {}", path.display());
    None
}

/// 测试运行目录，系统临时目录加进程号，避免多进程冲突
fn run_dir() -> PathBuf {
    mml_testutil::temp_dir().join(format!("mml-real-launcher-packs-{}", std::process::id()))
}

/// 初始化链，与 `mml_core::init` 相同，每进程一次
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

/// 安装阶段记录器，供断言检查是否走完所有阶段
#[derive(Default, Clone)]
struct StateRecorder {
    /// 按发生顺序记下的阶段标识
    states: Arc<Mutex<Vec<&'static str>>>,
}

impl StateRecorder {
    /// 判断某个阶段是否走到过
    ///
    /// - `state`: 阶段标识，取值同 [`state_id`]
    ///
    /// # 返回值
    ///
    /// 走到过返回 `true`
    fn reached(&self, state: &str) -> bool {
        self.states.lock().unwrap().contains(&state)
    }
}

/// 把安装阶段枚举转成稳定的字符串标识，供 [`StateRecorder::reached`] 比对
///
/// - `state`: 安装阶段
///
/// # 返回值
///
/// 返回该阶段的字符串标识
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

/// 安装入口，不带界面回调，重名时自动改名
///
/// - `zip`: 待安装的压缩包
/// - `name`: 实例名，`None` 时由包内元数据决定
/// - `pack_type`: 压缩包类型
///
/// # 返回值
///
/// 返回新实例的 uuid 与阶段记录器；安装失败会 panic
async fn install(
    zip: &PathBuf,
    name: Option<String>,
    pack_type: PackType,
) -> (Uuid, StateRecorder) {
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
    (uuid, recorder)
}

/// 读取压缩包内指定后缀的文件内容
///
/// - `archive`: 已打开的压缩包
/// - `suffix`: 条目名后缀
///
/// # 返回值
///
/// 返回文件内容；没有匹配条目或读取失败时返回 `None`
fn read_entry(archive: &BaseArchive, suffix: &str) -> Option<Vec<u8>> {
    let entry = archive
        .entries()
        .iter()
        .find(|e| !e.is_dir && e.name.replace('\\', "/").ends_with(suffix))?;
    archive.read(&entry.name).ok()
}

/// 把 HMCL packmeta 的组件列表转成加载器类型与版本
///
/// - `addons`: 组件列表，每项为组件 ID 与版本
///
/// # 返回值
///
/// 返回加载器类型与版本；没有已知加载器组件时为原版与 `None`
fn hmcl_loader(addons: &[(String, String)]) -> (LoaderType, Option<String>) {
    for (id, version) in addons {
        match id.as_str() {
            name_consts::FORGE_KEY => return (LoaderType::Forge, Some(version.clone())),
            name_consts::NEOFORGE_KEY => return (LoaderType::NeoForge, Some(version.clone())),
            name_consts::FABRIC_KEY => return (LoaderType::Fabric, Some(version.clone())),
            name_consts::QUILT_KEY => return (LoaderType::Quilt, Some(version.clone())),
            _ => {}
        }
    }
    (LoaderType::Normal, None)
}

/// M²L 即 ColorMC 实例压缩包：导出再导入的 roundtrip，离线可跑
#[tokio::test]
async fn mml_instance_archive_roundtrip() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    // ---------- 创建原始实例并写入文件 ----------
    let setting = InstanceSettingObj {
        name: "导出测试实例".to_string(),
        version: "1.20.1".to_string(),
        loader: LoaderType::Forge,
        loader_version: Some("47.3.0".to_string()),
        ..Default::default()
    };
    // 分组不在实例配置里，导出包也不带 GroupName 了，创建时单独指定
    let group = mml_game::add_group("导出分组").expect("建组失败");
    let game = setting
        .create_instance_in_group(None, Some(group))
        .await
        .unwrap();
    let original_uuid = game.read().unwrap().uuid;
    assert_eq!(mml_game::get_instance_group(&original_uuid), Some(group));
    let original_name = game.read().unwrap().name.clone();
    let base = game.read().unwrap().get_base_path();
    std::fs::write(base.join("options.txt"), "lang:zh_cn\n").unwrap();
    std::fs::create_dir_all(base.join("mods")).unwrap();
    std::fs::write(base.join("mods").join("testmod.jar"), b"test mod").unwrap();

    // ---------- 导出为 ColorMC 格式 ----------
    let export_file = mml_testutil::temp_dir().join(format!("mml-export-{}.zip", Uuid::new_v4()));
    game.read()
        .unwrap()
        .export(ExportArg {
            file: export_file.clone(),
            pack: ExportPackType::MML,
            archive: ArchiveType::Zip,
            mods: Vec::new(),
            files: Vec::new(),
            unselect: Vec::new(),
            select: Vec::new(),
            name: "导出测试实例".to_string(),
            author: String::new(),
            version: String::new(),
            summary: String::new(),
            gui: None,
        })
        .await
        .expect("导出失败");

    // 导出包内应包含实例元数据与在线文件信息
    let archive = BaseArchive::open(&export_file).unwrap();
    assert!(
        read_entry(&archive, name_consts::GAME_FILE).is_some(),
        "导出包应包含 game.json"
    );
    assert!(
        read_entry(&archive, name_consts::MOD_INFO_FILE).is_some(),
        "导出包应包含 modfileinfo.json"
    );
    drop(archive);

    // ---------- 导入：原实例保留，应自动改名且分配新 uuid ----------
    let recorder = StateRecorder::default();
    let imported_uuid = add_game::install_archive_from_file(
        &export_file,
        None,
        None,
        None,
        None,
        Some(Arc::new(recorder.clone())),
        None,
        PackType::ArchivePack,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("导入失败");

    let imported = mml_game::get_instance(&imported_uuid).expect("导入后应能取到实例");
    let read = imported.read().unwrap();
    assert_ne!(read.uuid, original_uuid, "导入实例应分配新的 uuid");
    assert_ne!(read.name, original_name, "与原实例重名应自动改名");
    assert_eq!(read.version, "1.20.1");
    assert!(matches!(read.loader, LoaderType::Forge));
    assert_eq!(read.loader_version.as_deref(), Some("47.3.0"));

    // 文件完整导入
    assert_eq!(
        std::fs::read(read.get_base_path().join("options.txt")).unwrap(),
        b"lang:zh_cn\n",
        "options.txt 应导入"
    );
    assert_eq!(
        std::fs::read(read.get_base_path().join("mods").join("testmod.jar")).unwrap(),
        b"test mod",
        "mod 应导入"
    );

    // 磁盘上的 game.json 应与新实例一致：game.json 由 config_save 异步落盘，
    // 且解压会先写入包内旧内容，轮询等待落盘稳定后再比对
    let disk = read.get_base_path().join(name_consts::GAME_FILE);
    let expect_uuid = read.uuid;
    drop(read);

    let mut disk_obj: Option<InstanceSettingObj> = None;
    for _ in 0..150 {
        if let Ok(obj) = mml_base::serialize_tools::json_from_file::<InstanceSettingObj>(&disk)
            && obj.uuid == expect_uuid
        {
            disk_obj = Some(obj);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        disk_obj.is_some(),
        "磁盘 game.json 应落盘且 uuid 与新实例一致: {}",
        disk.display()
    );

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&imported_uuid).unwrap();
    mml_game::delete_instance(&original_uuid).unwrap();
    let _ = std::fs::remove_file(&export_file);
}

/// HMCL 导出的整合包，mcbbs.packmeta 格式
///
/// 生成方式：HMCL → 版本 → 导出整合包，保存为 hmcl-instance.zip。
#[tokio::test]
async fn install_real_hmcl_modpack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = find_pack(
        "hmcl-instance.zip",
        "用 HMCL 的「导出整合包」生成",
    ) else {
        return;
    };

    // 类型检测
    let detected = detect_pack(&pack).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::HMCL), "应识别为 HMCL 包");
    assert!(!detected.name.is_empty(), "应从 packmeta 取到实例名");

    // 从包内元数据动态推导期望值
    let archive = BaseArchive::open(&pack).unwrap();
    let data = read_entry(&archive, name_consts::HMCLFILE).expect("应有 mcbbs.packmeta");
    let packmeta: mml_game::other_launcher::hmcl_obj::HMCLObj =
        mml_base::serialize_tools::json_from_bytes(&data).unwrap();
    let expect_name = packmeta.name.clone();
    let addons: Vec<(String, String)> = packmeta
        .addons
        .iter()
        .map(|a| (a.id.clone(), a.version.clone()))
        .collect();
    let expect_version = addons
        .iter()
        .find(|(id, _)| id.as_str() == name_consts::GAME_KEY)
        .map(|(_, v)| v.clone())
        .expect("packmeta 应有 game 版本");
    let (expect_loader, expect_loader_version) = hmcl_loader(&addons);

    // overrides 目录名取自随包 manifest.json，HMCL 导出必带
    let overrides = {
        let cfg_data = read_entry(&archive, name_consts::MANIFEST_FILE).expect("HMCL 包应带 manifest.json");
        let cfg: mml_game::curseforge::pack_obj::CurseForgePackObj =
            mml_base::serialize_tools::json_from_bytes(&cfg_data).unwrap();
        cfg.overrides
    };

    // 包内 overrides 目录下的文件应解压到实例基础目录
    let sample: Vec<String> = archive
        .entries()
        .iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| {
            let norm = e.name.replace('\\', "/");
            norm.strip_prefix(&format!("{overrides}/")).map(String::from)
        })
        .filter(|rel| !rel.is_empty())
        .take(5)
        .collect();
    drop(archive);

    let (uuid, recorder) = install(&pack, None, PackType::HMCL).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, expect_name, "实例名应取自 packmeta");
    assert_eq!(read.version, expect_version, "游戏版本应取自 packmeta");
    assert!(
        matches!(read.loader, LoaderType::Normal) || std::mem::discriminant(&read.loader)
            == std::mem::discriminant(&expect_loader),
        "加载器类型应与 packmeta 一致"
    );
    if let Some(v) = &expect_loader_version {
        assert_eq!(read.loader_version.as_deref(), Some(v.as_str()));
    }

    // 包内 overrides 目录下的文件应解压到实例基础目录；导出时 overrides
    // 为空则跳过文件检查
    assert!(
        sample.is_empty() || read.get_base_path().is_dir(),
        "实例目录应已创建"
    );
    for rel in &sample {
        assert!(
            read.get_base_path().join(rel).exists(),
            "文件应解压到实例目录: {rel}"
        );
    }
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// HMCL 带启动器的整包，含 HMCL.exe 与 .minecraft 目录结构，可能含多个版本
///
/// 生成方式：把 HMCL 整合目录连同 .minecraft 与 HMCL 启动器文件压缩为
/// hmcl.zip。
#[tokio::test]
async fn install_real_hmcl_portable_pack() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = find_pack(
        "hmcl.zip",
        "把带 HMCL 启动器的游戏目录（含 .minecraft）压缩",
    ) else {
        return;
    };

    // 无整合包元数据，按启动器包检测
    let detected = detect_pack(&pack).expect("检测失败");
    assert!(
        matches!(detected.pack_type, PackType::LauncherPack),
        "应识别为启动器包"
    );

    // 动态推导期望：版本 json 与应导入的文件
    let archive = BaseArchive::open(&pack).unwrap();
    let mc_prefix = find_minecraft_prefix(archive.entries()).expect("应有 .minecraft 目录");
    let versions = scan_versions(&archive, &mc_prefix);
    let primary = pick_primary(&archive, &mc_prefix, &versions).expect("应有可识别的版本");
    let expect_version = {
        let v = &versions[primary].obj;
        // HMCL 导出的版本 json 带 patches，game 补丁记录真实游戏版本
        match v.patches.iter().find(|p| p.id == name_consts::GAME_KEY) {
            Some(p) => p.version.clone(),
            None => {
                if v.inherits_from.is_empty() {
                    v.id.clone()
                } else {
                    v.inherits_from.clone()
                }
            }
        }
    };
    let isolated = versions[primary].isolated
        && version_folder_has_data(&archive, &mc_prefix, &versions[primary].version_name);
    let version_prefix = if isolated {
        format!("{mc_prefix}versions/{}/", versions[primary].version_name)
    } else {
        mc_prefix.clone()
    };
    // 版本隔离时排除版本自身的 json/jar
    let version_name = versions[primary].version_name.clone();
    let expect_files: Vec<String> = archive
        .entries()
        .iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| {
            let norm = e.name.replace('\\', "/");
            norm.strip_prefix(&version_prefix).map(String::from)
        })
        .filter(|rel: &String| {
            !rel.is_empty()
                && rel != &format!("{version_name}.json")
                && rel != &format!("{version_name}.jar")
        })
        .take(10)
        .collect();
    drop(archive);

    let (uuid, recorder) = install(&pack, Some("HMCL整包导入".to_string()), PackType::LauncherPack).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, "HMCL整包导入");
    assert_eq!(read.version, expect_version, "版本应取自版本 json");

    // 版本文件夹内的游戏资源应导入游戏目录
    for rel in &expect_files {
        assert!(
            read.get_game_path().join(rel).exists(),
            "游戏文件应导入: {rel}"
        );
    }
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// 真实 M²L 即 ColorMC 实例压缩包导入，布局为 game.json 加 .minecraft
///
/// 由 ColorMC/M²L 真实导出生成，保存为 colormc-instance.zip。
#[tokio::test]
async fn install_real_mml_instance_archive() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = find_pack(
        "colormc-instance.zip",
        "用 ColorMC/M²L 的实例导出生成",
    ) else {
        return;
    };

    let detected = detect_pack(&pack).expect("检测失败");
    assert!(
        matches!(detected.pack_type, PackType::ArchivePack),
        "应识别为 M²L 实例压缩包"
    );

    // 从包内 game.json 动态推导期望值
    let archive = BaseArchive::open(&pack).unwrap();
    let data = read_entry(&archive, name_consts::GAME_FILE).expect("应有 game.json");
    let meta: InstanceSettingObj = mml_base::serialize_tools::json_from_bytes(&data).unwrap();
    // 除元数据文件外的实例文件应完整导入
    let sample: Vec<String> = archive
        .entries()
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.name.replace('\\', "/"))
        .filter(|n| {
            n != name_consts::GAME_FILE
                && n != name_consts::MOD_INFO_FILE
                && n != "launch.json"
                && n != "guisetting.json"
        })
        .take(5)
        .collect();
    drop(archive);

    let (uuid, recorder) = install(&pack, None, PackType::ArchivePack).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, meta.name, "实例名应取自 game.json");
    assert_eq!(read.version, meta.version, "游戏版本应取自 game.json");
    assert!(
        std::mem::discriminant(&read.loader) == std::mem::discriminant(&meta.loader),
        "加载器类型应与 game.json 一致"
    );
    assert_eq!(read.loader_version, meta.loader_version, "加载器版本应一致");

    for rel in &sample {
        assert!(
            read.get_base_path().join(rel).exists(),
            "文件应导入实例目录: {rel}"
        );
    }
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// 真实 MultiMC 整包，即整个 MultiMC 目录，内含多个实例
///
/// 当前导入逻辑取包内第一个 mmc-pack.json 对应的实例，多实例只导入一个，
/// 保存为 MultiMC.zip。
#[tokio::test]
async fn install_real_multimc_dir() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = find_pack(
        "MultiMC.zip",
        "把 MultiMC 目录（含 instances）整体压缩",
    ) else {
        return;
    };

    let detected = detect_pack(&pack).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::MMC), "应识别为 MMC 包");

    // 包内第一个 mmc-pack.json 即导入目标
    let archive = BaseArchive::open(&pack).unwrap();
    let first = archive
        .entries()
        .iter()
        .find(|e| !e.is_dir && e.name.replace('\\', "/").ends_with(name_consts::MMCJSON_FILE))
        .expect("应有 mmc-pack.json")
        .name
        .replace('\\', "/");
    let inst_dir = first[..first.len() - name_consts::MMCJSON_FILE.len()].to_string();

    let data = read_entry(&archive, name_consts::MMCJSON_FILE).expect("应有 mmc-pack.json");
    let mmc: mml_game::other_launcher::mmc_obj::MMCObj =
        mml_base::serialize_tools::json_from_bytes(&data).unwrap();
    let mut expect_version = String::new();
    let mut expect_loader = LoaderType::Normal;
    let mut expect_loader_version: Option<String> = None;
    for c in &mmc.components {
        let uid = c.uid.to_ascii_lowercase();
        if uid == "net.minecraft" {
            expect_version = c.version.clone();
        } else if uid == "net.minecraftforge" {
            expect_loader = LoaderType::Forge;
            expect_loader_version = Some(c.version.clone());
        } else if uid == "net.neoforged" {
            expect_loader = LoaderType::NeoForge;
            expect_loader_version = Some(c.version.clone());
        } else if uid == "net.fabricmc.fabric-loader" {
            expect_loader = LoaderType::Fabric;
            expect_loader_version = Some(c.version.clone());
        } else if uid == "org.quiltmc.quilt-loader" {
            expect_loader = LoaderType::Quilt;
            expect_loader_version = Some(c.version.clone());
        }
    }

    // 该实例目录下的文件应解压到实例基础目录，元数据文件被排除
    let sample: Vec<String> = archive
        .entries()
        .iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| {
            let norm = e.name.replace('\\', "/");
            norm
                .strip_prefix(&inst_dir)
                .filter(|rel| {
                    !rel.is_empty()
                        && *rel != name_consts::MMCJSON_FILE
                        && *rel != name_consts::MMCCFG_FILE
                })
                .map(String::from)
        })
        .take(5)
        .collect();
    drop(archive);
    assert!(!expect_version.is_empty(), "mmc-pack.json 应有 net.minecraft 组件");

    let (uuid, recorder) = install(&pack, None, PackType::MMC).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, detected.name, "实例名应取自第一个 instance.cfg");
    assert_eq!(read.version, expect_version, "游戏版本应取自 mmc-pack.json");
    assert!(
        matches!(read.loader, LoaderType::Normal) || std::mem::discriminant(&read.loader)
            == std::mem::discriminant(&expect_loader),
        "加载器类型应与 mmc-pack.json 一致"
    );
    if let Some(v) = &expect_loader_version {
        assert_eq!(read.loader_version.as_deref(), Some(v.as_str()));
    }

    for rel in &sample {
        assert!(
            read.get_base_path().join(rel).exists(),
            "文件应解压到实例目录: {rel}"
        );
    }
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}

/// MMC / Prism 导出的实例，含 mmc-pack.json 与 instance.cfg
///
/// 生成方式：把 Prism/MMC 实例目录压缩为 mmc-instance.zip。
#[tokio::test]
async fn install_real_mmc_instance() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ensure_init();

    let Some(pack) = find_pack(
        "mmc-instance.zip",
        "把 Prism/MMC 实例目录压缩",
    ) else {
        return;
    };

    let detected = detect_pack(&pack).expect("检测失败");
    assert!(matches!(detected.pack_type, PackType::MMC), "应识别为 MMC 包");
    assert!(!detected.name.is_empty(), "应从 instance.cfg 取到实例名");

    // 动态推导期望：net.minecraft 版本与加载器组件
    let archive = BaseArchive::open(&pack).unwrap();
    let data = read_entry(&archive, name_consts::MMCJSON_FILE).expect("应有 mmc-pack.json");
    let mmc: mml_game::other_launcher::mmc_obj::MMCObj =
        mml_base::serialize_tools::json_from_bytes(&data).unwrap();
    let mut expect_version = String::new();
    let mut expect_loader = LoaderType::Normal;
    let mut expect_loader_version: Option<String> = None;
    for c in &mmc.components {
        let uid = c.uid.to_ascii_lowercase();
        if uid == "net.minecraft" {
            expect_version = c.version.clone();
        } else if uid == "net.minecraftforge" {
            expect_loader = LoaderType::Forge;
            expect_loader_version = Some(c.version.clone());
        } else if uid == "net.neoforged" {
            expect_loader = LoaderType::NeoForge;
            expect_loader_version = Some(c.version.clone());
        } else if uid == "net.fabricmc.fabric-loader" {
            expect_loader = LoaderType::Fabric;
            expect_loader_version = Some(c.version.clone());
        } else if uid == "org.quiltmc.quilt-loader" {
            expect_loader = LoaderType::Quilt;
            expect_loader_version = Some(c.version.clone());
        }
    }
    drop(archive);
    assert!(!expect_version.is_empty(), "mmc-pack.json 应有 net.minecraft 组件");

    let (uuid, recorder) = install(&pack, None, PackType::MMC).await;

    let game = mml_game::get_instance(&uuid).expect("安装后应能取到实例");
    let read = game.read().unwrap();
    assert_eq!(read.name, detected.name, "实例名应取自 instance.cfg");
    assert_eq!(read.version, expect_version, "游戏版本应取自 mmc-pack.json");
    assert!(
        matches!(read.loader, LoaderType::Normal) || std::mem::discriminant(&read.loader)
            == std::mem::discriminant(&expect_loader),
        "加载器类型应与 mmc-pack.json 一致"
    );
    if let Some(v) = &expect_loader_version {
        assert_eq!(read.loader_version.as_deref(), Some(v.as_str()));
    }
    drop(read);

    for state in ["readInfo", "extract", "done"] {
        assert!(recorder.reached(state), "缺少安装阶段: {state}");
    }

    mml_game::delete_instance(&uuid).unwrap();
}
