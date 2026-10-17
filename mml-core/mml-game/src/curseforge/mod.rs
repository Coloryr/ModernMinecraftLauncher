//! CurseForge 内容查询与整合包安装

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tokio_util::sync::CancellationToken;

use crate::{
    GameInstance,
    curseforge::pack_obj::CurseForgePackObj,
    data_res::{DownloadItemRes, ItemPathRes},
    gui_hook::{AddModPackGui, AddModPackState},
    launcher::{
        FileType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    loader::LoaderType,
    modpack::{BaseModPackWorker, ModPackWorker, curseforge_worker::CurseForgeWorker},
};
use mml_base::{
    archives::{BaseArchive, BaseArchiveGui},
    file_item::{FileHash, FileItemObj, LaterRun},
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use mml_net::curseforge_api::{
    self, CurseFogreArg,
    categories_obj::CurseForgeCategoriesObj,
    file_obj::{CurseForgeFileDataObj, CurseForgeFileObj, DependenciesObj},
    version_obj::{CurseForgeVersionDataObj, CurseForgeVersionTypeDataObj},
};

pub mod pack_obj;

/// 分类信息缓存
static CATEGORIES: OnceLock<CurseForgeCategoriesObj> = OnceLock::new();
/// 游戏版本列表缓存
static GAME_VERSION: OnceLock<Vec<String>> = OnceLock::new();

/// 排序编号
pub enum CurseForgeSortField {
    /// 精选
    Featured,
    /// 热度
    Popularity,
    /// 最后更新
    LastUpdated,
    /// 名字
    Name,
    /// 作者
    Author,
    /// 下载量
    TotalDownloads,
    /// 分类
    Category,
    /// 游戏版本
    GameVersion,
}

impl CurseForgeSortField {
    /// 获取排序编号
    ///
    /// # 返回值
    ///
    /// 返回 CurseForge API 对应的排序 ID
    pub fn get_id(&self) -> u32 {
        match self {
            CurseForgeSortField::Featured => 1,
            CurseForgeSortField::Popularity => 2,
            CurseForgeSortField::LastUpdated => 3,
            CurseForgeSortField::Name => 4,
            CurseForgeSortField::Author => 5,
            CurseForgeSortField::TotalDownloads => 6,
            CurseForgeSortField::Category => 7,
            CurseForgeSortField::GameVersion => 8,
        }
    }
}

/// 转换为 CurseForge 的加载器 ID
///
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回加载器 ID；该加载器不被 CurseForge 支持返回 `None`
pub fn to_loader_id(loader: &LoaderType) -> Option<u32> {
    match loader {
        LoaderType::Forge => Some(1),
        LoaderType::Fabric => Some(4),
        LoaderType::Quilt => Some(5),
        LoaderType::NeoForge => Some(6),
        _ => None,
    }
}

/// 创建下载项目
///
/// - `obj`: CurseForge 文件信息
/// - `path`: 保存目录
///
/// # 返回值
///
/// 返回主文件下载项
pub fn make_file_item_obj<P: AsRef<Path>>(obj: &mut CurseForgeFileDataObj, path: P) -> FileItemObj {
    obj.fix_download_url();

    FileItemObj {
        url: obj.download_url.clone().unwrap(),
        name: obj.display_name.clone(),
        file: path.as_ref().join(&obj.file_name),
        hash: FileHash::Sha1(obj.sha1_hash()),
        later: LaterRun::None,
    }
}

/// 创建在线文件信息
///
/// - `obj`: CurseForge 文件信息
/// - `path`: 实例内相对目录
///
/// # 返回值
///
/// 返回在线文件信息
pub fn make_file_online_info_obj(obj: &mut CurseForgeFileDataObj, path: &str) -> OnlineInfoObj {
    obj.fix_download_url();

    OnlineInfoObj {
        path: path.to_string(),
        name: obj.display_name.clone(),
        file: obj.file_name.clone(),
        sha1: obj.sha1_hash(),
        url: obj.download_url.clone().unwrap_or_default(),
        modid: obj.mod_id.to_string(),
        fileid: obj.id.to_string(),
    }
}

/// 模组依赖列表
pub struct CurseForgeModDependenciesRes {
    /// 名字
    pub name: String,
    /// 图标地址，项目接口给的是远程地址，没有时是 None
    pub icon: Option<String>,
    /// 项目编号
    pub mod_id: u64,
    /// 是否可选
    pub opt: bool,
    /// 候选文件，按"合适程度"排好，**第一个是自动选中的那个**；调用方可以让用户改选
    pub list: Vec<CurseForgeFileDataObj>,
}

/// 获取模组依赖
///
/// - `obj`: CurseForge 文件信息
/// - `version`: 游戏版本
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回依赖列表，含递归解析出的子依赖，已去重
pub async fn get_mod_dependencies(
    obj: &CurseForgeFileDataObj,
    version: &str,
    loader: LoaderType,
) -> Vec<CurseForgeModDependenciesRes> {
    let ids = Mutex::new(HashSet::new());
    let handle = tokio::runtime::Handle::current();
    let dependencies = obj.dependencies.clone();
    let version = version.to_string();

    tokio::task::spawn_blocking(move || {
        get_mod_dependencies_inner(&dependencies, &version, &loader, &ids, &handle)
    })
    .await
    .unwrap()
}

/// 递归解析模组依赖
///
/// - `dependencies`: 依赖列表
/// - `version`: 游戏版本
/// - `loader`: 加载器类型
/// - `ids`: 已解析的项目 ID 集合，避免重复解析
/// - `handle`: 用于并发查询的运行时句柄
///
/// # 返回值
///
/// 返回新解析出的依赖列表
fn get_mod_dependencies_inner(
    dependencies: &Option<Vec<DependenciesObj>>,
    version: &str,
    loader: &LoaderType,
    ids: &Mutex<HashSet<u64>>,
    handle: &tokio::runtime::Handle,
) -> Vec<CurseForgeModDependenciesRes> {
    let dep = match dependencies {
        Some(dep) if !dep.is_empty() => dep,
        _ => return Vec::new(),
    };

    let list: Mutex<Vec<CurseForgeModDependenciesRes>> = Mutex::new(Vec::new());

    dep.par_iter().for_each(|item| {
        // 原子检查并插入：HashSet::insert 在元素已存在时返回 false
        {
            let mut ids_guard = ids.lock().unwrap();
            if !ids_guard.insert(item.mod_id) {
                return;
            }
        }

        let id = item.mod_id.to_string();
        // 只保留"要用户去下"的两种关系：1 内嵌库 / 4 工具 / 5 明确冲突 / 6 包含都不算前置。
        // 这里算的是"是否可选"，写进结果时**不要再取反**
        let optional = match item.relation_type {
            2 => true,
            3 => false,
            _ => return,
        };

        let (res1, res2) = handle.block_on(async {
            let res1 = curseforge_api::get_files_page(CurseFogreArg {
                id: Some(id.clone()),
                version: Some(version.to_string()),
                loader: to_loader_id(loader),
                ..Default::default()
            })
            .await;

            if res1.is_err() {
                return (None, None);
            }

            let data = res1.unwrap();
            if data.data.is_empty() {
                return (None, None);
            }

            let res2 = curseforge_api::get_mod_info(&id).await;
            (Some(data), res2.ok())
        });

        let Some(data) = res1 else { return };
        let Some(data1) = res2 else { return };

        // 在移动 data.data 之前，先递归获取第一个文件的依赖
        let sub_deps =
            get_mod_dependencies_inner(&data.data[0].dependencies, version, loader, ids, handle);

        list.lock().unwrap().push(CurseForgeModDependenciesRes {
            name: data1.data.name.clone(),
            icon: data1.data.logo.url.clone(),
            mod_id: data1.data.id,
            opt: optional,
            list: data.data,
        });

        // 添加未被标记的子依赖，避免同时持有 ids 和 list 锁
        for item5 in sub_deps {
            let mut ids_guard = ids.lock().unwrap();
            if ids_guard.contains(&item5.mod_id) {
                continue;
            }
            ids_guard.insert(item5.mod_id);
            drop(ids_guard);
            list.lock().unwrap().push(item5);
        }
    });

    list.into_inner().unwrap()
}

/// 版本类型排序：id > 17 按 id 倒序在前，id < 18 按 id 正序在后
fn sort_version_types(
    data: Vec<&CurseForgeVersionTypeDataObj>,
) -> Vec<&CurseForgeVersionTypeDataObj> {
    let mut new_list: Vec<_> = data.iter().copied().filter(|item| item.id > 17).collect();
    new_list.sort_by_key(|item| Reverse(item.id));

    let mut old_list: Vec<_> = data.iter().copied().filter(|item| item.id < 18).collect();
    old_list.sort_by_key(|item| item.id);

    new_list.extend(old_list);
    new_list
}

/// 获取文件类型对应的 class ID
///
/// - `file_type`: 项目文件类型
///
/// # 返回值
///
/// 返回 CurseForge API 对应的 class ID；无对应项返回 0
fn get_classid(file_type: FileType) -> u32 {
    match file_type {
        FileType::Mod => curseforge_api::CLASS_MOD,
        FileType::Save => curseforge_api::CLASS_SAVES,
        FileType::Shaderpack => curseforge_api::CLASS_SHADERPACKS,
        FileType::Resourcepack => curseforge_api::CLASS_RESOURCEPACKS,
        _ => 0,
    }
}

/// 获取分组数据
///
/// - `file_type`: 项目文件类型
///
/// # 返回值
///
/// 返回分类 ID 到分类名的映射表；查询失败返回对应错误
pub async fn get_categories(file_type: FileType) -> CoreResult<HashMap<String, String>> {
    let temp = match CATEGORIES.get() {
        Some(data) => data,
        None => {
            let list = curseforge_api::get_categories().await?;
            CATEGORIES.get_or_init(|| list)
        }
    };

    let classid = get_classid(file_type);

    let mut list: Vec<_> = temp
        .data
        .iter()
        .filter(|item| item.class_id == classid)
        .collect();

    list.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(list
        .into_iter()
        .map(|item| (item.id.to_string(), item.name.clone()))
        .collect())
}

/// 获取支持的游戏版本
///
/// # 返回值
///
/// 返回游戏版本列表，首位为空串表示不限；查询失败返回对应错误
pub async fn get_game_versions() -> CoreResult<Vec<String>> {
    if let Some(data) = GAME_VERSION.get() {
        return Ok(data.clone());
    }

    let types = curseforge_api::get_version_type().await?;
    let type_list: Vec<&CurseForgeVersionTypeDataObj> = types
        .data
        .iter()
        .filter(|item| item.name.starts_with("Minecraft "))
        .collect();

    let versions = curseforge_api::get_version().await?;
    let version_map: HashMap<u32, &CurseForgeVersionDataObj> = versions
        .data
        .iter()
        .map(|item| (item.verion_type, item))
        .collect();

    let mut list = vec![String::new()];
    for item in sort_version_types(type_list) {
        if let Some(data) = version_map.get(&item.id) {
            list.extend_from_slice(&data.versions);
        }
    }

    Ok(GAME_VERSION.get_or_init(|| list).clone())
}

/// 构建下载结果，串行处理
/// 只有 `get_modpack_info` 中的 `spawn_fetch_task` 才需要并行处理
async fn build_results(
    game: &GameInstance,
    items: Vec<CurseForgeFileDataObj>,
    gui: &AddModPackGui,
    size: usize,
) -> DownloadItemRes {
    let mut list = Vec::with_capacity(size);
    let mut online = HashMap::with_capacity(size);
    let mut now = 0usize;

    for mut item in items {
        let Some(path) = get_item_path(game, &item.file_name, item.mod_id).await.ok() else {
            continue;
        };

        // 修正下载地址并只计算一次 SHA1，避免在 make_file_item_obj
        // 和 make_file_online_info_obj 中被重复计算
        item.fix_download_url();
        let url = item.download_url.clone().unwrap_or_default();
        let sha1 = item.sha1_hash();

        let modid_str = item.mod_id.to_string();

        let mut file_item = FileItemObj {
            url: url.clone(),
            name: item.display_name.clone(),
            file: path.file_path.join(&item.file_name),
            hash: FileHash::Sha1(sha1.clone()),
            later: LaterRun::None,
        };

        let online_item = if matches!(path.file_type, FileType::Save) {
            file_item.later = LaterRun::UnpackSave(game.read().unwrap().get_saves_path());
            None
        } else {
            Some(OnlineInfoObj {
                path: path.path.clone(),
                name: item.display_name.clone(),
                file: item.file_name.clone(),
                sha1,
                url,
                modid: modid_str.clone(),
                fileid: item.id.to_string(),
            })
        };

        let now_val = now;
        now += 1;
        if let Some(gui) = gui {
            gui.set_sub_now(now_val, Some(size));
        }

        // 组装：后写覆盖的去重方式
        list.push(file_item);
        online.remove(&modid_str);
        if let Some(oi) = online_item {
            online.insert(modid_str, oi);
        }
    }

    DownloadItemRes { list, online }
}

/// 获取整合包模组信息
///
/// - `game`: 目标实例
/// - `obj`: 整合包信息
/// - `gui`: 整合包安装界面回调
///
/// # 返回值
///
/// 返回文件下载项列表与在线信息表；任一文件查询失败返回空结果
pub async fn get_modpack_info(
    game: &GameInstance,
    obj: &CurseForgePackObj,
    gui: &AddModPackGui,
) -> CoreResult<DownloadItemRes> {
    let size = obj.files.len();

    let file_ids: Vec<_> = obj.files.iter().map(|f| f.file_id).collect();

    // ── 批量路径：get_files 成功 → 一次性处理全部 ──
    if let Ok(items) = curseforge_api::get_files(file_ids).await {
        return Ok(build_results(game, items, gui, size).await);
    }

    // ── 后备路径：逐个获取文件，任一失败 → 返回空 ──
    const CONCURRENCY: usize = 20;
    let failed = Arc::new(AtomicBool::new(false));
    let mut fetched: Vec<CurseForgeFileDataObj> = Vec::with_capacity(size);

    {
        let mut tasks = tokio::task::JoinSet::new();
        let mut iter = obj.files.iter();

        // 填充初始批次
        for file_ref in (&mut iter).take(CONCURRENCY) {
            spawn_fetch_task(&mut tasks, file_ref, &failed);
        }

        // 取空并补充
        while let Some(result) = tasks.join_next().await {
            if let Ok(Some(data)) = result {
                fetched.push(data.data);
            }
            // 任一任务失败后停止派发新任务，但继续等待剩余任务完成
            if failed.load(Ordering::Relaxed) {
                continue;
            }
            if let Some(file_ref) = iter.next() {
                spawn_fetch_task(&mut tasks, file_ref, &failed);
            }
        }
    }

    if failed.load(Ordering::Relaxed) {
        return Ok(DownloadItemRes {
            list: Vec::new(),
            online: HashMap::new(),
        });
    }

    Ok(build_results(game, fetched, gui, size).await)
}

/// 派发单个文件的查询任务；查询失败时置位 failed 标记
fn spawn_fetch_task(
    tasks: &mut tokio::task::JoinSet<Option<CurseForgeFileObj>>,
    file_ref: &pack_obj::FilesObj,
    failed: &Arc<AtomicBool>,
) {
    let pid = file_ref.project_id.to_string();
    let fid = file_ref.file_id.to_string();
    let failed = failed.clone();
    tasks.spawn(async move {
        match curseforge_api::get_mod(&pid, &fid).await {
            Ok(data) => Some(data),
            Err(_) => {
                failed.store(true, Ordering::Relaxed);
                None
            }
        }
    });
}

/// 解析条目路径和文件类型。
///
/// 先完成所有异步请求，再取读锁同步解析路径，避免把 `RwLockReadGuard`
/// 带进 `.await`，因此无需克隆整个 [`InstanceSettingObj`]。
async fn get_item_path(
    game: &GameInstance,
    file_name: &str,
    mod_id: u64,
) -> CoreResult<ItemPathRes> {
    // 非 .jar 文件需要联网查询文件类型；期间不持有读锁
    let class_ids: Option<(Vec<u32>, u32)> = if file_name.ends_with(names::JAR_DOT_EXT) {
        None
    } else {
        let info1 = curseforge_api::get_mod_info(&mod_id.to_string()).await?;
        Some((
            info1
                .data
                .categories
                .iter()
                .map(|item2| item2.class_id)
                .collect(),
            info1.data.class_id,
        ))
    };

    // 异步结束后再取读锁，同步构建路径
    let game = game.read().unwrap();
    let mut item1 = ItemPathRes {
        file_path: game.get_mods_path(),
        path: names::GAME_MODS_DIR.to_string(),
        file_type: FileType::Mod,
    };
    if let Some((category_ids, fallback_id)) = class_ids {
        // 分类列表：第一个匹配的生效
        for class_id in category_ids {
            if apply_class_id(class_id, &mut item1, &game) {
                break;
            }
        }

        // 后备：data.class_id 可能覆盖分类结果
        apply_class_id(fallback_id, &mut item1, &game);
    }

    Ok(item1)
}

/// 将 CurseForge 的 class_id 应用到路径解析器。当该 id 匹配到
/// 已知的文件类型分类时返回 `true`。
#[inline]
fn apply_class_id(class_id: u32, item: &mut ItemPathRes, instance: &InstanceSettingObj) -> bool {
    match class_id {
        curseforge_api::CLASS_RESOURCEPACKS => {
            item.change_to_resourcepacks(instance);
            true
        }
        curseforge_api::CLASS_SHADERPACKS => {
            item.change_to_shaderpacks(instance);
            true
        }
        curseforge_api::CLASS_SAVES => {
            item.change_to_saves(instance);
            true
        }
        curseforge_api::CLASS_OPENLOADER_DATAPACK => {
            item.change_to_openloader_datapack(instance);
            true
        }
        _ => false,
    }
}

/// 升级整合包
///
/// - `game`: 目标实例
/// - `data`: 新版本的文件信息
/// - `pack_gui`: 整合包安装界面回调
/// - `archive_gui`: 压缩包操作界面回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 成功返回 `Ok(())`；下载 / 解包 / 检查升级失败返回对应错误
pub async fn upgrade_modpack(
    game: &GameInstance,
    data: &mut CurseForgeFileDataObj,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<()> {
    data.fix_download_url();

    let obj = make_file_item_obj(data, mml_downloader::get_download_path());
    let file = obj.file.clone();

    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::DownloadPack);
        gui.set_now(1, Some(6));
    }

    let res = mml_downloader::start_download_task(vec![obj]).await;
    if !res {
        return Err(ErrorType::DownloadFileFail);
    }

    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::ReadInfo);
        gui.set_now(2, Some(6));
    }

    let zip = BaseArchive::open(file)?;
    let mut worker = CurseForgeWorker::new(BaseModPackWorker::new(
        zip,
        None,
        pack_gui.clone(),
        archive_gui.clone(),
        cancel.clone(),
        None, // 升级不重建实例，图标保持原样
    ));

    worker.read_info()?;
    worker.read_version().await?;

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    worker.update_game(game);

    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::Extract);
        gui.set_now(3, Some(6));
    }

    // **排除包根的 `manifest.json`**，理由见 modrinth 那边的详细说明：`extract` 会把
    // "不在 `overrides/` 下"的条目写进实例 base 目录，而那份清单就是 `check_upgrade` 读的
    // "旧清单"；被新包覆盖后差异算成空 → 不下载也不删除。清单由 `check_upgrade` 末尾写回。
    worker
        .extract(Some(vec![names::MANIFEST_FILE.to_string()]))
        .await?;

    if let Some(gui) = &pack_gui {
        gui.set_sub_text(None);
        gui.set_sub_now(0, None);
        gui.set_state(AddModPackState::GetInfo);
        gui.set_now(4, Some(6));
    }

    if cancel.is_cancelled() {
        return Err(ErrorType::TaskCancel);
    }

    worker.check_upgrade().await?;

    if let Some(gui) = &pack_gui {
        gui.set_sub_text(None);
        gui.set_sub_now(0, None);
        gui.set_state(AddModPackState::DownloadFile);
        gui.set_now(5, Some(6));
    }

    worker.download().await;

    if let Some(gui) = &pack_gui {
        gui.set_state(AddModPackState::Done);
        gui.set_now(6, Some(6));
    }

    // 记下装的是哪一份：更新检查按它比"有没有新版本"，不记的话每次都当有新版本。
    // 只在走到这里才写 —— 取消或中途失败时实例仍指向旧版本，与旧 manifest 一致
    {
        let mut game = game.write().unwrap();
        game.pid = Some(data.mod_id.to_string());
        game.fid = Some(data.id.to_string());
        game.save();
    }

    // 实例名与目录跟着新版走，与安装时同一套规则；用户手动改过的名字也会被覆盖
    crate::modpack::rename_after_upgrade(game, worker.default_instance_name());

    Ok(())
}
