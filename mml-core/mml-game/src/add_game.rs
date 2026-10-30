//! 添加游戏实例操作
//! 导入文件夹，导入压缩包
use std::path::{Path, PathBuf};

use mml_base::{
    archives::{ArchiveEntryInfo, BaseArchive, BaseArchiveGui},
    file_item::{FileHash, FileItemObj, LaterRun},
    serialize_tools::{self, MiniJsonObj},
};
use mml_names::{
    i18,
    i18_items::{
        error_type::{ArgEmptyData, CoreResult, DataNotFoundData, ErrorType, PathNotExistsData},
        info_type::InfoType,
    },
    names,
};
use mml_net::{
    curseforge_api::file_obj::CurseForgeFileDataObj, modrinth_api::version_obj::ModrinthVersionObj,
};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    GameInstance,
    curseforge::{self, pack_obj::CurseForgePackObj},
    game_options,
    gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState, ProgressGui},
    launcher::{
        ModPackType,
        file_online_info_obj::OnlineInfoObj,
        instance_setting_obj::{CustomLoaderObj, InstanceSettingObj},
    },
    launcher_path::version_path,
    modpack::{
        BaseModPackWorker, ModPackWorker, curseforge_worker::CurseForgeWorker,
        modrinth_worker::ModrinthPackWorker,
    },
    modrinth,
    other_launcher::{
        self,
        hmcl_obj::{HMCLObj, HMCLServerObj},
        mmc_obj::MMCObj,
        official_obj::OfficialObj,
    },
    serverpack::serverpack_obj::ServerPackObj,
};

/// 路径不存在转 PathNotExists
fn path_not_exists(path: PathBuf) -> ErrorType {
    ErrorType::PathNotExists(PathNotExistsData { path })
}

/// 压缩包类型
pub enum PackType {
    /// CurseForge 整合包
    CurseForge,
    /// Modrinth 整合包
    Modrinth,
    /// MMC 导出包
    MMC,
    /// HMCL 导出包
    HMCL,
    /// HMCL 服务器包
    HMCLServer,
    /// 直接解压，包内是本启动器导出的实例
    ArchivePack,
    /// 包含其他启动器数据目录的压缩包
    LauncherPack,
}

impl PackType {
    /// 压缩包独立 ID，与前端 i18n 键对应，不随语言变化
    ///
    /// # 返回值
    ///
    /// 固定的英文 ID，如 `curseforge`
    pub fn id(&self) -> &'static str {
        match self {
            PackType::CurseForge => "curseforge",
            PackType::Modrinth => "modrinth",
            PackType::MMC => "mmc",
            PackType::HMCL => "hmcl",
            PackType::HMCLServer => "hmcl_server",
            PackType::ArchivePack => "archive",
            PackType::LauncherPack => "launcher_pack",
        }
    }

    /// 按 ID 解析压缩包类型
    ///
    /// - `id`: `id()` 给出的英文 ID
    ///
    /// # 返回值
    ///
    /// 对应的类型；未知 ID 返回 `None`
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "curseforge" => Some(PackType::CurseForge),
            "modrinth" => Some(PackType::Modrinth),
            "mmc" => Some(PackType::MMC),
            "hmcl" => Some(PackType::HMCL),
            "hmcl_server" => Some(PackType::HMCLServer),
            "archive" => Some(PackType::ArchivePack),
            "launcher_pack" => Some(PackType::LauncherPack),
            _ => None,
        }
    }

    /// 全部压缩包 ID，下拉列表的数据源
    ///
    /// # 返回值
    ///
    /// 全部类型的 ID，顺序固定
    pub fn ids() -> Vec<&'static str> {
        vec![
            "curseforge",
            "modrinth",
            "mmc",
            "hmcl",
            "hmcl_server",
            "archive",
            "launcher_pack",
        ]
    }
}

/// Modrinth 整合包索引文件名
const MODRINTH_INDEX_FILE: &str = "modrinth.index.json";

/// 服务器包清单在网址里的结尾，带前导斜杠
const SERVER_PACK_SUFFIX: &str = "/server.json";

/// 压缩包检测结果，添加实例窗口选好压缩包后自动填表用
pub struct DetectedPack {
    /// 检测出的压缩包类型
    pub pack_type: PackType,
    /// 推荐实例名：元数据里的名字，取不到就用文件名去扩展名
    pub name: String,
}

/// 读取压缩包内 JSON 条目的 `name` 字段，读不到返回空串
fn archive_json_name(archive: &BaseArchive, entry: &str) -> String {
    archive
        .read(entry)
        .ok()
        .and_then(|data| MiniJsonObj::from_stream(data.as_slice()).ok())
        .and_then(|json| json.as_object())
        .map(|map| {
            let name = map.get_string("name");
            let version = map.get_opt_string("version").or(map.get_opt_string("versionId"));

            match version {
                Some(data) => format!("{name}-{data}"),
                None => name,
            }
        })
        .unwrap_or_default()
}

/// 读取 MMC 包 `instance.cfg` 里的 `name=xxx`，读不到返回空串
fn archive_cfg_name(archive: &BaseArchive) -> String {
    let entry = match archive
        .entries()
        .iter()
        .find(|e| !e.is_dir && e.name.replace('\\', "/").ends_with(names::MMCCFG_FILE))
    {
        Some(entry) => entry.name.clone(),
        None => return String::new(),
    };
    let Ok(data) = archive.read(&entry) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&data);
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("name=") {
            return value.trim().to_string();
        }
    }
    String::new()
}

/// 检测压缩包的整合包类型，并取推荐实例名
///
/// - `file`: 压缩包路径
///
/// 按元数据文件判定类型，HMCL 包内可能同时带 `mcbbs.packmeta` 与 `manifest.json`，
/// 所以 CurseForge 的判定放在 HMCL 之后；无元数据时按 `.minecraft` 目录判启动器包，
/// 否则视为直接解压。名字取元数据里的 `name`，MMC 读 `instance.cfg`，取不到用文件名去扩展名。
///
/// # 返回值
///
/// 检测结果；压缩包无法打开返回对应错误
pub fn detect_pack<P: AsRef<Path>>(file: P) -> CoreResult<DetectedPack> {
    let file = file.as_ref();
    let archive = BaseArchive::open(file)?;

    let find_entry = |suffix: &str| -> Option<String> {
        archive
            .entries()
            .iter()
            .find(|e| !e.is_dir && e.name.replace('\\', "/").ends_with(suffix))
            .map(|e| e.name.clone())
    };

    // 文件名去扩展名兜底
    let fallback = || {
        file.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };

    let mut detected: Option<(PackType, String)> = None;
    for (suffix, pack_type) in [
        (names::HMCLFILE, PackType::HMCL),
        (names::SERVER_MANIFEST_FILE, PackType::HMCLServer),
        (names::MMCJSON_FILE, PackType::MMC),
        (MODRINTH_INDEX_FILE, PackType::Modrinth),
        (names::MANIFEST_FILE, PackType::CurseForge),
        (names::GAME_FILE, PackType::ArchivePack),
    ] {
        if let Some(entry) = find_entry(suffix) {
            detected = Some((pack_type, archive_json_name(&archive, &entry)));
            break;
        }
    }

    let (pack_type, name) = match detected {
        Some((PackType::MMC, json_name)) if json_name.is_empty() => {
            (PackType::MMC, archive_cfg_name(&archive))
        }
        Some((pack_type, name)) => (pack_type, name),
        None if find_minecraft_prefix(archive.entries()).is_some() => {
            (PackType::LauncherPack, String::new())
        }
        None => (PackType::ArchivePack, String::new()),
    };

    let name = if name.is_empty() { fallback() } else { name };
    Ok(DetectedPack { pack_type, name })
}

/// 导入文件夹
///
/// - `dir`: 游戏文件夹
/// - `name`: 实例名，覆盖原名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不复制的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `progress_gui`: 复制进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例；路径不存在、无实例名或创建失败返回对应错误
pub async fn add_game_folder<P: AsRef<Path>>(
    dir: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<PathBuf>>,
    instance_gui: AddInstanceGui,
    progress_gui: ProgressGui,
    cancel: CancellationToken,
) -> CoreResult<GameInstance> {
    if !dir.as_ref().exists() || !dir.as_ref().is_dir() {
        return Err(path_not_exists(dir.as_ref().to_path_buf()));
    }

    let mut instance: Option<InstanceSettingObj> = None;
    let mut is_mmc = false;
    // 是 MMC 实例：按 mmc-pack.json 与 instance.cfg 建实例
    if other_launcher::is_mmc_version(&dir)
        && let Ok(mmc) =
            serialize_tools::json_from_file::<MMCObj>(dir.as_ref().join(names::MMCJSON_FILE))
    {
        let cfg =
            game_options::read_options_from_file(dir.as_ref().join(names::MMCCFG_FILE), Some('='))?;

        instance = Some(mmc.to_instance(cfg));
        is_mmc = true;
    } else {
        // 不是 MMC 就按官启版本 json 建实例
        let files = path_helper::get_files(&dir);
        for item in files {
            if let Some(ext) = item.extension()
                && ext == names::JSON_EXT
            {
                if let Ok(data) = OfficialObj::read_from_file(item) {
                    instance = Some(data.to_instance());
                }
            }
        }
    }

    let mut instance = match instance {
        Some(data) => data,
        None => InstanceSettingObj {
            version: version_path::get_latest_version(),
            ..Default::default()
        },
    };

    // 传了非空名字就覆盖包内解析出的名字
    if let Some(name) = &name
        && !name.is_empty()
    {
        instance.name = name.clone();
    }

    if instance.name.is_empty() && name.is_none() {
        return Err(ErrorType::ArgEmpty(ArgEmptyData::Name));
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 分组不在实例配置里，见 game_group，创建时一并交给分组表
    let res = instance
        .create_instance_in_group(instance_gui.clone(), group)
        .await?;

    res.read()
        .unwrap()
        .copy_files(dir, unselect, is_mmc, progress_gui.clone())
        .await?;

    Ok(res)
}

/// 从整合包添加实例
///
/// - `file`: 整合包压缩包
/// - `source`: 整合包类型
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
/// - `icon`: 整合包图标 URL，创建实例时写进实例，`None` 表示没有
///
/// # 返回值
///
/// 新实例 uuid；路径不存在、元数据缺失或中途取消返回对应错误
async fn modpack<P: AsRef<Path>>(
    file: P,
    source: ModPackType,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
    icon: Option<String>,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(5));
        pack_gui.set_sub_now(0, Some(1));
    }

    let mut work: Box<dyn ModPackWorker> = if source == ModPackType::CurseForge {
        Box::new(CurseForgeWorker::new(BaseModPackWorker::new(
            BaseArchive::open(file)?,
            instance_gui,
            pack_gui.clone(),
            archive_gui,
            cancel.clone(),
            icon,
        )))
    } else {
        Box::new(ModrinthPackWorker::new(BaseModPackWorker::new(
            BaseArchive::open(file)?,
            instance_gui,
            pack_gui.clone(),
            archive_gui,
            cancel.clone(),
            icon,
        )))
    };

    work.read_info()?;
    work.read_version().await?;

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    let uuid = work.create_instance(name, group).await?;

    // 实例已经建出来了：从这里往后每一步失败或取消，都必须把这个半成品实例删掉，
    // 否则界面报"安装失败"，实例列表里却多出一个装了一半的实例。所以收进一个块统一收拾残局
    let res: CoreResult<()> = async {
        // 把新实例交给 worker：extract 与 get_info 都依赖 worker 里的 game
        let game = crate::get_instance(&uuid)
            .ok_or_else(|| ErrorType::DataNotFound(DataNotFoundData::GameInstance))?;
        work.update_game(&game);

        if let Some(pack_gui) = &pack_gui {
            pack_gui.set_state(AddModPackState::Extract);
            pack_gui.set_now(2, Some(5));
            pack_gui.set_sub_now(0, Some(1));
        }

        work.extract(unselect).await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(pack_gui) = &pack_gui {
            pack_gui.set_state(AddModPackState::GetInfo);
            pack_gui.set_sub_text(None);
            pack_gui.set_sub_now(0, None);
            pack_gui.set_now(3, Some(5));
            pack_gui.set_sub_now(0, None);
        }

        work.get_info().await?;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(pack_gui) = &pack_gui {
            pack_gui.set_state(AddModPackState::DownloadFile);
            pack_gui.set_sub_text(None);
            pack_gui.set_now(4, Some(5));
            pack_gui.set_sub_now(0, None);
        }

        work.download().await;

        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        if let Some(pack_gui) = &pack_gui {
            pack_gui.set_state(AddModPackState::Done);
            pack_gui.set_now(5, Some(5));
        }

        Ok(())
    }
    .await;

    if let Err(err) = res {
        // 删半成品实例：删失败只记日志，别把真正的失败原因盖掉
        if let Err(err) = crate::delete_instance(&uuid) {
            mml_log::error_type(err);
        }
        return Err(err);
    }

    Ok(uuid)
}

/// 解压整合包压缩包到实例目录，统一处理压缩包整体套一层文件夹的情况
///
/// - `archive`: 已打开的压缩包
/// - `output_dir`: 解压目标目录，实例基础目录
/// - `unselect`: 按压缩包内完整条目名排除的文件列表
/// - `strip_dir`: 压缩包整体套的顶层目录名，不剥时传 `None`
/// - `archive_gui`: 可选的进度回调
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解压失败返回对应错误
fn extract_pack<P: AsRef<Path>>(
    archive: &BaseArchive,
    output_dir: P,
    unselect: Vec<String>,
    strip_dir: Option<String>,
    archive_gui: BaseArchiveGui,
) -> CoreResult<()> {
    archive.extract_all(output_dir, Some(unselect), strip_dir, archive_gui)
}

/// 直接解压
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖 `game.json` 里的名字
///
/// 压缩包内是本启动器导出的实例包，含 `game.json`。
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或 `game.json` 缺失返回对应错误
async fn archive<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open(file)?;
    // 找到 game.json 所在条目，兼容压缩包整体套一层文件夹的情况
    let game_entry = archive
        .entries()
        .iter()
        .find(|e| !e.is_dir && e.name.ends_with(names::GAME_FILE))
        .ok_or_else(|| ErrorType::DataNotFound(DataNotFoundData::Info))?;
    let data = archive.read(&game_entry.name)?;
    let mut obj = serialize_tools::json_from_bytes::<InstanceSettingObj>(&data)?;

    if let Some(name) = name {
        obj.name = name;
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;
    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // game.json 所在目录即实例包裹目录，解压时去掉该层，直接放进实例目录
    let strip_dir = game_entry
        .name
        .strip_suffix(names::GAME_FILE)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches(['/', '\\']).to_string());

    // 包内的 game.json 是导出时的旧元数据，旧 uuid；create_instance 已保存
    // 新实例的 game.json，不能被解压覆盖
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(game_entry.name.clone());

    extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        strip_dir,
        archive_gui,
    )?;

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 导入 MMC 导出包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或 MMC 元数据缺失返回对应错误
async fn mmc_archive<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open(&file)?;

    let mut path = String::new();
    let mut mmc = None;
    let mut cfg = None;

    for item in archive.entries() {
        if item.is_dir {
            continue;
        } else if mmc.is_none() && item.name.ends_with(names::MMCJSON_FILE) {
            path = item.name.replace(names::MMCJSON_FILE, "");
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<MMCObj>(&data)?;
            mmc = Some(obj);
        } else if cfg.is_none() && item.name.ends_with(names::MMCCFG_FILE) {
            let data = archive.read_stream(&item.name)?;
            cfg = Some(game_options::read_options(data, Some('='))?);
        }

        if mmc.is_some() && cfg.is_some() {
            break;
        }
    }

    if mmc.is_none() || cfg.is_none() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }

    let mut obj = mmc.unwrap().to_instance(cfg.unwrap());
    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // MMC 元数据按完整条目名加入排除列表
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(format!("{path}{}", names::MMCJSON_FILE));
    unselect.push(format!("{path}{}", names::MMCCFG_FILE));

    // 解压到实例基础目录，`.minecraft/` 下的内容自然落到游戏目录 base/.minecraft。
    // mmc-pack.json 所在目录即包裹层，解压时去掉
    let strip_dir = path.trim_end_matches(['/', '\\']).to_string();
    extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        (!strip_dir.is_empty()).then_some(strip_dir),
        archive_gui.clone(),
    )?;

    let json = game.read().unwrap().read_custom_json();
    if !json.is_empty() {
        game.write().unwrap().custom_loader = Some(CustomLoaderObj {
            custom_json: true,
            ..Default::default()
        });
        game.read().unwrap().save();
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 导入 HMCL 导出包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或 HMCL 元数据缺失返回对应错误
async fn hmcl_archive<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open(&file)?;

    let mut path = String::new();
    let mut hmcl = None;
    let mut cfg = None;

    for item in archive.entries() {
        if item.is_dir {
            continue;
        } else if hmcl.is_none() && item.name.ends_with(names::HMCLFILE) {
            path = item.name.replace(names::HMCLFILE, "");
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<HMCLObj>(&data)?;
            hmcl = Some(obj);
        } else if cfg.is_none() && item.name.ends_with(names::MANIFEST_FILE) {
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<CurseForgePackObj>(&data)?;
            cfg = Some(obj);
        }

        if hmcl.is_some() && cfg.is_some() {
            break;
        }
    }

    if hmcl.is_none() || cfg.is_none() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }

    let mut obj = hmcl.unwrap().to_instance();
    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // HMCL 元数据按完整条目名加入排除列表
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(format!("{path}{}", names::HMCLFILE));

    let over = if let Some(cfg) = cfg {
        cfg.overrides.clone()
    } else {
        names::OVERRIDE_DIR.to_string()
    };

    let mut strip_dir = path.trim_end_matches(['/', '\\']).to_string();
    let dir = if !strip_dir.is_empty() {
        over
    } else {
        strip_dir.push_str(&over);
        strip_dir
    };

    extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        Some(dir),
        archive_gui.clone(),
    )?;

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 导入 HMCL 服务器包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或服务器清单缺失返回对应错误
async fn hmcl_server_archive<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open(&file)?;

    let mut path = String::new();
    let mut hmcl = None;

    for item in archive.entries() {
        if item.is_dir {
            continue;
        } else if hmcl.is_none() && item.name.ends_with(names::SERVER_MANIFEST_FILE) {
            path = item.name.replace(names::SERVER_MANIFEST_FILE, "");
            let data = archive.read(&item.name)?;
            let obj = serialize_tools::json_from_bytes::<HMCLServerObj>(&data)?;
            hmcl = Some(obj);
            break;
        }
    }

    if hmcl.is_none() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }

    let hmcl = hmcl.unwrap();

    let mut obj = hmcl.to_instance();
    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    let mut online = game.read().unwrap().read_online_info();

    if !hmcl.files.is_empty() && !hmcl.file_api.is_empty() {
        let url = if hmcl.file_api.ends_with('/') {
            format!("{}/", hmcl.file_api)
        } else {
            hmcl.file_api
        };
        for item in hmcl.files {
            let path = Path::new(&item.path);
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let dir = path
                .parent()
                .map(|item| item.to_string_lossy())
                .map(|item| item.to_string())
                .unwrap_or_default();
            online.insert(
                item.path.clone(),
                OnlineInfoObj {
                    modid: Default::default(),
                    fileid: Default::default(),
                    path: dir,
                    name: name.clone(),
                    file: name.clone(),
                    sha1: item.hash,
                    url: format!("{url}{}", item.path),
                },
            );
        }
    }

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    // HMCL 元数据按完整条目名加入排除列表
    let mut unselect = unselect.unwrap_or_default();
    unselect.push(format!("{path}{}", names::HMCLFILE));

    let over = names::OVERRIDE_DIR.to_string();
    let mut strip_dir = path.trim_end_matches(['/', '\\']).to_string();
    let dir = if !strip_dir.is_empty() {
        over
    } else {
        strip_dir.push_str(&over);
        strip_dir
    };

    extract_pack(
        &archive,
        game.read().unwrap().get_base_path(),
        unselect,
        Some(dir),
        archive_gui.clone(),
    )?;

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 压缩包内扫描到的官方版本信息
pub struct LauncherVersion {
    /// 版本 json 在压缩包内的完整条目名
    pub entry: String,
    /// 版本号：隔离时取 `versions/{name}/` 的文件夹名，未隔离时取 json 的 id
    pub version_name: String,
    /// 解析出的官方版本信息
    pub obj: OfficialObj,
    /// 版本 json 是否位于 `versions/{name}/` 下，即是否开启版本隔离
    pub isolated: bool,
}

/// 查找压缩包内 `.minecraft` 文件夹所在目录前缀，含末尾分隔符
///
/// - `entries`: 压缩包条目列表
///
/// 兼容压缩包整体套一层文件夹的情况，如 `mygame/.minecraft/...` 返回 `mygame/.minecraft/`。
///
/// # 返回值
///
/// `.minecraft` 所在目录前缀，含末尾分隔符；找不到返回 `None`
pub fn find_minecraft_prefix(entries: &[ArchiveEntryInfo]) -> Option<String> {
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let norm = entry.name.replace('\\', "/");
        let parts: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
        if let Some(index) = parts.iter().position(|item| *item == names::GAME_DIR) {
            return Some(format!("{}/", parts[..=index].join("/")));
        }
    }
    None
}

/// 扫描 `.minecraft` 下可解析为官方版本 json 的文件
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
///
/// 版本 json 位于 `versions/{name}/` 下视为开启了版本隔离。
///
/// # 返回值
///
/// 扫描到的版本列表
pub fn scan_versions(archive: &BaseArchive, mc_prefix: &str) -> Vec<LauncherVersion> {
    let mut versions = Vec::new();
    for entry in archive.entries() {
        if entry.is_dir {
            continue;
        }
        let norm = entry.name.replace('\\', "/");
        let Some(rel) = norm.strip_prefix(mc_prefix) else {
            continue;
        };
        if !rel.ends_with(names::JSON_DOT_EXT) {
            continue;
        }
        let Ok(data) = archive.read(&entry.name) else {
            continue;
        };
        let Ok(obj) = OfficialObj::from_reader(data.as_slice()) else {
            continue;
        };
        if obj.id.is_empty() {
            continue;
        }

        let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
        let isolated = parts.first() == Some(&"versions") && parts.len() >= 2;
        let version_name = if isolated {
            parts[1].to_string()
        } else {
            obj.id.clone()
        };

        versions.push(LauncherVersion {
            entry: entry.name.clone(),
            version_name,
            obj,
            isolated,
        });
    }
    versions
}

/// 判断 `versions/{name}/` 文件夹内是否有游戏资源
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
/// - `name`: 版本文件夹名
///
/// 版本隔离开启时游戏资源存放在该文件夹内；若只有版本 json/jar，说明仍是未隔离的
/// 常规 `.minecraft` 布局，游戏资源在 `.minecraft` 根目录。
///
/// # 返回值
///
/// 该版本文件夹内是否有除版本 json/jar 以外的游戏资源
pub fn version_folder_has_data(archive: &BaseArchive, mc_prefix: &str, name: &str) -> bool {
    let folder = format!("{mc_prefix}versions/{name}/");
    archive.entries().iter().any(|entry| {
        if entry.is_dir {
            return false;
        }
        let norm = entry.name.replace('\\', "/");
        let Some(rest) = norm.strip_prefix(&folder) else {
            return false;
        };
        !rest.is_empty() && rest != format!("{name}.json") && rest != format!("{name}.jar")
    })
}

/// 读取官方启动器 `launcher_profiles.json` 中记录的版本号
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
///
/// # 返回值
///
/// 记录的版本号列表；文件或字段缺失返回 `None`
fn read_last_version_ids(archive: &BaseArchive, mc_prefix: &str) -> Option<Vec<String>> {
    let target = format!("{mc_prefix}{}", names::LAUNCHER_PROFILES_FILE);
    let entry = archive
        .entries()
        .iter()
        .find(|entry| !entry.is_dir && entry.name.replace('\\', "/") == target)?;
    let data = archive.read(&entry.name).ok()?;
    let json = MiniJsonObj::from_stream(data.as_slice()).ok()?;

    let mut list = Vec::new();
    if let Some(map) = json.as_object()
        && let Some(profiles) = map.get_object("profiles")
    {
        for (_, profile) in profiles.iter() {
            if let Some(profile) = profile.as_object() {
                let id = profile.get_string("lastVersionId");
                if !id.is_empty() {
                    list.push(id);
                }
            }
        }
    }
    Some(list)
}

/// 选择要导入的版本，返回它在列表中的下标
///
/// - `archive`: 已打开的压缩包
/// - `mc_prefix`: `.minecraft` 目录前缀，含末尾分隔符
/// - `versions`: 扫描到的版本列表
///
/// # 返回值
///
/// 选中版本的下标：优先官方启动器最后使用的版本，其次开启版本隔离且文件夹内有
/// 游戏资源的版本；列表为空返回 `None`
pub fn pick_primary(
    archive: &BaseArchive,
    mc_prefix: &str,
    versions: &[LauncherVersion],
) -> Option<usize> {
    if versions.is_empty() {
        return None;
    }
    if versions.len() == 1 {
        return Some(0);
    }

    // 优先使用官方启动器配置中最后使用的版本
    if let Some(ids) = read_last_version_ids(archive, mc_prefix) {
        for id in ids {
            if let Some(index) = versions
                .iter()
                .position(|item| item.obj.id == id || item.version_name == id)
            {
                return Some(index);
            }
        }
    }

    // 其次优先开启版本隔离且版本文件夹内有游戏资源的版本
    for (index, version) in versions.iter().enumerate() {
        if version.isolated && version_folder_has_data(archive, mc_prefix, &version.version_name) {
            return Some(index);
        }
    }

    Some(0)
}

/// 导入其他启动器的压缩包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖版本 json 解析出的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// 压缩包内可能是整个 `.minecraft` 文件夹，可能整体套了一层文件夹。
/// 通过扫描其中可解析为 [`OfficialObj`] 的版本 json 判断游戏版本与版本隔离：
/// - 版本 json 直接位于 `.minecraft` 下 → 未开启版本隔离，直接导入 `.minecraft` 内的游戏资源；
/// - 版本 json 位于 `.minecraft/versions/{name}/` 下 → 开启了版本隔离，将该版本文件夹内的资源导入。
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或找不到 `.minecraft` 目录返回对应错误
async fn launcher_pack<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    if !file.as_ref().exists() || file.as_ref().is_dir() {
        return Err(path_not_exists(file.as_ref().to_path_buf()));
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::ReadInfo);
        pack_gui.set_now(1, Some(3));
    }

    let archive = BaseArchive::open(&file)?;

    // 查找 `.minecraft` 文件夹
    let mc_prefix = match find_minecraft_prefix(archive.entries()) {
        Some(prefix) => prefix,
        None => return Err(ErrorType::DataNotFound(DataNotFoundData::Info)),
    };

    // 扫描可解析的版本 json，并选择要导入的版本
    let mut versions = scan_versions(&archive, &mc_prefix);
    let primary_index = pick_primary(&archive, &mc_prefix, &versions);

    // 版本隔离：版本 json 在 `versions/{name}/` 下 且 该版本文件夹内有游戏资源
    let isolated = primary_index
        .filter(|&index| versions[index].isolated)
        .map(|index| version_folder_has_data(&archive, &mc_prefix, &versions[index].version_name))
        .unwrap_or(false);

    // 提前取出版本文件夹名，供解压时判断路径
    let version_name = primary_index.map(|index| versions[index].version_name.clone());
    let version_prefix = version_name
        .as_ref()
        .filter(|_| isolated)
        .map(|name| format!("{mc_prefix}versions/{name}/"));

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    // 创建实例设置
    let mut obj = if let Some(index) = primary_index {
        let LauncherVersion {
            obj,
            entry: _,
            version_name: _,
            isolated: _,
        } = versions.remove(index);
        // 版本列表未加载时直接使用 json 中的版本号
        let fallback = if obj.inherits_from.is_empty() {
            obj.id.clone()
        } else {
            obj.inherits_from.clone()
        };
        let mut obj = obj.to_instance();
        if obj.version.is_empty() {
            obj.version = fallback;
        }
        obj
    } else {
        InstanceSettingObj {
            version: version_path::get_latest_version(),
            ..Default::default()
        }
    };

    if let Some(name) = name {
        obj.name = name;
    }

    if obj.name.is_empty() {
        obj.name = file
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Extract);
        pack_gui.set_now(2, Some(3));
    }

    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;

    if cancel.is_cancelled() {
        crate::delete_instance(&uuid)?;
        return Err(ErrorType::TaskCancel);
    }

    let game_path = game.read().unwrap().get_game_path();
    let unselect = unselect.unwrap_or_default();

    archive.extract_where(
        |entry| {
            if entry.is_dir {
                return None;
            }
            if unselect.iter().any(|item| item == &entry.name) {
                return None;
            }
            let norm = entry.name.replace('\\', "/");
            let Some(rel) = norm.strip_prefix(&mc_prefix) else {
                return None;
            };
            if let Some(prefix) = &version_prefix {
                // 版本隔离：只导入版本文件夹内的游戏资源
                let Some(rest) = norm.strip_prefix(prefix) else {
                    return None;
                };
                let name = version_name.as_ref().unwrap();
                if rest == format!("{name}.json") || rest == format!("{name}.jar") {
                    return None;
                }
                Some(game_path.join(rest))
            } else {
                // 未开启版本隔离：直接导入 `.minecraft` 下的游戏资源
                Some(game_path.join(rel))
            }
        },
        archive_gui.as_deref(),
    )?;

    if let Some(pack_gui) = &pack_gui {
        pack_gui.set_state(AddModPackState::Done);
        pack_gui.set_now(3, Some(3));
    }

    Ok(uuid)
}

/// 本地压缩包没有项目 / 文件 ID 时，把整合包类型清成「无」
///
/// 类型下拉只对能回查在线项目的包有意义；本地包留着 CurseForge / Modrinth 会让
/// 「检查更新」指向不存在的项目。`is_modpack` 保持原样，启动与模组检查照旧。
///
/// - `uuid`: 刚安装好的实例 uuid
fn clear_modpack_type_without_ids(uuid: &Uuid) {
    let Some(instance) = crate::get_instance(uuid) else {
        return;
    };
    let mut game = instance.write().unwrap();
    if game.pid.is_some() && game.fid.is_some() {
        return;
    }
    if game.modpack_type == ModPackType::None {
        return;
    }
    game.modpack_type = ModPackType::None;
    game.save();
}

/// 从文件路径安装压缩包
///
/// - `file`: 压缩包
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `pack_type`: 压缩包类型
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；路径不存在或元数据缺失返回对应错误
pub async fn install_archive_from_file<P: AsRef<Path>>(
    file: P,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    pack_type: PackType,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let uuid = match pack_type {
        PackType::CurseForge => {
            modpack(
                file,
                ModPackType::CurseForge,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
                None, // 本地压缩包没有图标 URL
            )
            .await
        }
        PackType::Modrinth => {
            modpack(
                file,
                ModPackType::Modrinth,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
                None, // 本地压缩包没有图标 URL
            )
            .await
        }
        PackType::MMC => {
            mmc_archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::HMCL => {
            hmcl_archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::HMCLServer => {
            hmcl_server_archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::ArchivePack => {
            archive(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
        PackType::LauncherPack => {
            launcher_pack(
                file,
                name,
                group,
                unselect,
                instance_gui,
                pack_gui,
                archive_gui,
                cancel,
            )
            .await
        }
    }?;

    // 本地包没有项目 / 文件 ID 时，类型留「无」：否则界面上会显示成可更新平台
    clear_modpack_type_without_ids(&uuid);
    Ok(uuid)
}

/// 从在线网址安装整合包
///
/// - `url`: 压缩包下载地址
/// - `name`: 实例名，覆盖包内元数据里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `unselect`: 不导入的文件列表
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `pack_type`: 压缩包类型
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；下载失败或安装失败返回对应错误
pub async fn install_archive_from_url(
    url: &str,
    name: Option<String>,
    group: Option<Uuid>,
    unselect: Option<Vec<String>>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    pack_type: PackType,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let file = mml_downloader::gen_temp_file();

    let res = mml_downloader::start_download_task(vec![FileItemObj {
        name: i18::get_info(InfoType::TempFile),
        file: file.clone(),
        url: url.to_string(),
        hash: FileHash::None,
        later: LaterRun::None,
    }])
    .await;

    if !res {
        return Err(ErrorType::DownloadFileFail);
    }

    install_archive_from_file(
        file,
        name,
        group,
        unselect,
        instance_gui,
        pack_gui,
        archive_gui,
        pack_type,
        cancel,
    )
    .await
}

/// 把用户填的在线实例地址规整成 `server.json` 的下载地址
///
/// 前端填的可能是发布目录，也可能直接是清单文件：末尾没有 `server.json` 就补上。
///
/// - `value`: 用户填的地址
///
/// # 返回值
///
/// 以 `server.json` 结尾的地址；空地址返回 `DataNotFound(Url)`
pub fn normalize_server_pack_url(value: &str) -> CoreResult<String> {
    let url = value.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Url));
    }
    if url.to_ascii_lowercase().ends_with(SERVER_PACK_SUFFIX) {
        return Ok(url.to_string());
    }
    Ok(format!("{url}/{}", names::SERVER_FILE))
}

/// 从清单地址取实例里要记的服务器地址
///
/// 启动时的 `server_pack_update` 自己拼 `<地址>/server.json` 与 `.sha1`，
/// 所以实例里存的是**目录**，不带清单文件名。
///
/// - `manifest`: 已经规整过的清单地址
///
/// # 返回值
///
/// 发布目录地址，末尾不带斜杠
pub fn server_pack_base_url(manifest: &str) -> String {
    if !manifest.to_ascii_lowercase().ends_with(SERVER_PACK_SUFFIX) {
        return manifest.to_string();
    }
    manifest[..manifest.len() - SERVER_PACK_SUFFIX.len()].to_string()
}

/// 下载并解析在线服务器包清单
///
/// 添加窗口填完地址后的预览与真正导入共用这一趟：地址补齐、下载、解析只写一份。
///
/// - `url`: 用户填的地址，发布目录或 `server.json` 都认
///
/// # 返回值
///
/// 实际读取的清单地址（已补 `server.json`）与解析出的清单
pub async fn fetch_server_pack(url: &str) -> CoreResult<(String, ServerPackObj)> {
    let manifest_url = normalize_server_pack_url(url)?;
    let data = mml_net::get_work_client().get_bytes(&manifest_url).await?;
    let pack = serialize_tools::json_from_bytes::<ServerPackObj>(&data)?;
    Ok((manifest_url, pack))
}

/// 从在线服务器包创建实例，并把包内容同步下来
///
/// 下载 `server.json` 读出游戏版本与加载器，登记成「在线网络整合包」后立刻同步一次：
/// 模组、配置与图标在导入时就落地，玩家拿到实例可以直接启动，不必等第一次启动再下。
///
/// - `url`: 用户填的地址，发布目录或 `server.json` 都认
/// - `name`: 实例名，覆盖清单里的名字
/// - `group`: 目标分组 uuid，`None` 表示默认分组
/// - `instance_gui`: 添加实例界面回调
/// - `pack_gui`: 安装进度回调，同步下载时用它显示进度
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；地址为空、清单解析失败、下载或解压失败返回对应错误，
/// 失败时已建的半成品实例会被删掉
pub async fn install_server_pack_from_url(
    url: &str,
    name: Option<String>,
    group: Option<Uuid>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let (manifest_url, pack) = fetch_server_pack(url).await?;
    let base_url = server_pack_base_url(&manifest_url);

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    let obj = InstanceSettingObj {
        name: name.unwrap_or(pack.name.clone()),
        version: pack.version.clone(),
        loader: pack.loader,
        loader_version: pack.loader_version.clone(),
        is_modpack: true,
        modpack_type: ModPackType::ServerPack,
        server_url: Some(base_url.clone()),
        ..Default::default()
    };
    let game = obj.create_instance_in_group(instance_gui, group).await?;
    let uuid = game.read().unwrap().uuid;
    let snapshot = game.read().unwrap().clone();

    let total = pack.online_list.len() + pack.archive_list.len();
    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::DownloadFile);
        gui.set_now(0, Some(total));
    }

    // 图标名先取出来：清单本体要整个交给同步流程
    let icon = pack.icon.clone();
    // 进度推到添加窗口的弹窗上：整批一个下载任务，完成数从任务表同步
    let progress_gui = pack_gui.clone();
    let report = move |done: usize, all: usize| {
        if let Some(gui) = &progress_gui {
            gui.set_now(done, Some(all));
        }
    };
    if let Err(e) = snapshot
        .upgrade_serverpack_with(pack, cancel, Some(&report))
        .await
    {
        // 半成品实例留着只会让人以为导入成功，删掉重来更干净
        let _ = crate::delete_instance(&uuid);
        return Err(e);
    }

    // 图标只是外观，下不下来都不影响实例可用
    download_server_pack_icon(&uuid, &snapshot, &base_url, icon.as_deref()).await;

    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::Done);
        gui.set_now(total, Some(total));
    }

    Ok(uuid)
}

/// 按发布地址重新同步一个在线服务器包实例
///
/// 启动时的服务器包检查与「实例设置」里的升级按钮走同一条链路：拉清单 → 与本地清单比对 →
/// 删旧文件、下新文件、重解压配置目录 → 回写在线信息与实例的版本 / 加载器。
///
/// - `game`: 目标实例的设置快照
/// - `gui`: 安装进度回调，`None` 表示不显示进度
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 同步成功返回 `Ok(())`；实例没有发布地址、清单读取失败、下载或解压失败返回对应错误
pub async fn upgrade_server_pack(
    game: &InstanceSettingObj,
    gui: AddModPackGui,
    cancel: CancellationToken,
) -> CoreResult<()> {
    let url = game
        .server_url
        .clone()
        .filter(|item| !item.trim().is_empty())
        .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?;

    let (manifest_url, pack) = fetch_server_pack(&url).await?;
    // 图标名先取出来：清单本体要整个交给同步流程
    let icon = pack.icon.clone();

    let total = pack.online_list.len() + pack.archive_list.len();
    if let Some(gui) = &gui {
        gui.set_state(AddModPackState::DownloadFile);
        gui.set_now(0, Some(total));
    }

    let progress_gui = gui.clone();
    let report = move |done: usize, all: usize| {
        if let Some(gui) = &progress_gui {
            gui.set_now(done, Some(all));
        }
    };
    // 旧清单先挪成 `.old`：升级时按新旧两份清单算该删什么，与启动时那条链路一致
    game.move_serverpack_to_old()?;
    game.upgrade_serverpack_with(pack, cancel, Some(&report))
        .await?;

    // 图标会随清单换，按新清单再取一次；下不下来都不影响实例可用
    let base = server_pack_base_url(&manifest_url);
    download_server_pack_icon(&game.uuid, game, &base, icon.as_deref()).await;

    if let Some(gui) = &gui {
        gui.set_state(AddModPackState::Done);
        gui.set_now(total, Some(total));
    }

    Ok(())
}

/// 下载服务器包里的图标并写进实例目录
///
/// 清单里的 `Icon` 是发布目录下的文件名：取 `<发布地址>/<文件名>` 存成实例的
/// `icon.png`，与玩家自己换图标走同一条路（实例图标只认本地文件）。
///
/// - `uuid`: 实例 uuid
/// - `game`: 实例设置快照，取实例目录用
/// - `base`: 发布地址
/// - `icon`: 清单里的图标文件名；空表示没有
async fn download_server_pack_icon(
    uuid: &Uuid,
    game: &InstanceSettingObj,
    base: &str,
    icon: Option<&str>,
) {
    let Some(name) = icon.map(str::trim).filter(|item| !item.is_empty()) else {
        return;
    };
    // 只认文件名：清单里的 `../` 不能让它写到发布目录外面去
    let Some(file) = Path::new(name).file_name().and_then(|item| item.to_str()) else {
        return;
    };
    let url = format!("{base}/{file}");
    let Ok(bytes) = mml_net::get_work_client().get_bytes(&url).await else {
        return;
    };
    if std::fs::write(game.get_base_path().join(names::ICON_FILE), &bytes).is_err() {
        return;
    }
    if let Some(instance) = crate::get_instance(uuid) {
        let mut write = instance.write().unwrap();
        write.icon = Some(names::ICON_FILE.to_string());
        write.save();
    }
}

/// 安装 Modrinth 在线整合包
///
/// - `data`: Modrinth 版本信息
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `icon`: 整合包图标 URL，交给安装器，创建实例时写入；下载失败只记日志，不影响安装
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；下载或安装失败返回对应错误
pub async fn install_modrinth(
    data: &ModrinthVersionObj,
    group: Option<Uuid>,
    icon: Option<String>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let item = modrinth::make_download_obj(data, mml_downloader::get_download_path());
    let file = item.file.clone();

    let res = mml_downloader::start_download_task(vec![item]).await;

    if !res {
        return Err(ErrorType::DownloadFileFail);
    }

    // 直接走 modpack，不走 install_archive_from_file：图标要一路塞给 worker，
    // 让它在创建实例时一起写，那个 7 臂分发器吃不下这个参数，其中 5 个臂也用不到
    let uuid = modpack(
        file,
        ModPackType::Modrinth,
        None,
        group,
        None,
        instance_gui,
        pack_gui,
        archive_gui,
        cancel,
        icon,
    )
    .await?;

    if let Some(instance) = crate::get_instance(&uuid) {
        let mut write = instance.write().unwrap();
        write.pid = Some(data.project_id.clone());
        write.fid = Some(data.id.clone());
        write.save();
    }

    Ok(uuid)
}

/// 安装 CurseForge 在线整合包
///
/// - `data`: CurseForge 文件信息，安装成功后回填项目与文件 ID
/// - `group`: 目标分组 uuid，`None` 表示默认分组，不存在或已删的组也会落到默认分组
/// - `icon`: 整合包图标 URL，交给安装器，创建实例时写入；下载失败只记日志，不影响安装
/// - `instance_gui`: 实例创建界面回调
/// - `pack_gui`: 安装进度回调
/// - `archive_gui`: 解压进度回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 新实例 uuid；下载或安装失败返回对应错误
pub async fn install_curseforge(
    data: &mut CurseForgeFileDataObj,
    group: Option<Uuid>,
    icon: Option<String>,
    instance_gui: AddInstanceGui,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<Uuid> {
    let item = curseforge::make_file_item_obj(data, mml_downloader::get_download_path());
    let file = item.file.clone();

    let res = mml_downloader::start_download_task(vec![item]).await;

    if !res {
        return Err(ErrorType::DownloadFileFail);
    }

    // 直接走 modpack，不走 install_archive_from_file：理由同 install_modrinth
    let uuid = modpack(
        file,
        ModPackType::CurseForge,
        None,
        group,
        None,
        instance_gui,
        pack_gui,
        archive_gui,
        cancel,
        icon,
    )
    .await?;

    if let Some(instance) = crate::get_instance(&uuid) {
        let mut write = instance.write().unwrap();
        write.pid = Some(data.mod_id.to_string());
        write.fid = Some(data.id.to_string());
        write.save();
    }

    Ok(uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 目录地址补上清单文件名，末尾斜杠不算内容
    #[test]
    fn directory_url_gets_manifest_file() {
        assert_eq!(
            normalize_server_pack_url("https://example.com/pack/").unwrap(),
            "https://example.com/pack/server.json"
        );
        assert_eq!(
            normalize_server_pack_url("  https://example.com/pack  ").unwrap(),
            "https://example.com/pack/server.json"
        );
    }

    /// 已经是清单地址就原样留着，大小写不敏感
    #[test]
    fn manifest_url_is_kept() {
        assert_eq!(
            normalize_server_pack_url("https://example.com/pack/server.json").unwrap(),
            "https://example.com/pack/server.json"
        );
        assert_eq!(
            normalize_server_pack_url("https://example.com/pack/SERVER.JSON").unwrap(),
            "https://example.com/pack/SERVER.JSON"
        );
    }

    #[test]
    fn empty_url_is_rejected() {
        assert!(normalize_server_pack_url("   ").is_err());
    }

    /// 实例里存的是发布目录：启动同步时自己拼 `server.json`
    #[test]
    fn instance_keeps_publish_directory() {
        assert_eq!(
            server_pack_base_url("https://example.com/pack/server.json"),
            "https://example.com/pack"
        );
        assert_eq!(
            server_pack_base_url("https://example.com/pack"),
            "https://example.com/pack"
        );
    }
}
