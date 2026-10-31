//! 资源链接 实验性功能
//! 模组和材质包支持文件硬链接
//! 可以将 download/mods 下的模组文件硬链接到实例 mods 下；
//! 每个实例的映射记录写在实例目录的 link_info.json

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        LazyLock, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use mml_base::{
    hash_helper::{self, HashType},
    serialize_tools,
};
use mml_config::config_save;
use mml_names::{
    i18_items::error_type::{CoreResult, DataNotFoundData, DownloadFileHashErrorData, ErrorType},
    names, uuids,
};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::info::{LinkInfoObj, ResourceInfoObj, ResourceType};

pub mod info;

/// 公共模组目录
static MOD_DOWNLOAD_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 公共资源包目录
static RESOURCE_PACK_DOWNLOAD_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 公共模组索引文件
static MOD_DOWNLOAD_FILE: OnceLock<PathBuf> = OnceLock::new();
/// 公共资源包索引文件
static RESOURCE_DOWNLOAD_FILE: OnceLock<PathBuf> = OnceLock::new();

/// 资源映射总开关，由 GUI 配置同步
static ENABLED: AtomicBool = AtomicBool::new(false);

/// 游戏实例链接资源文件存储
static LINK_DATA: OnceLock<RwLock<HashMap<Uuid, Vec<LinkInfoObj>>>> = OnceLock::new();

/// 下载到公共目录后补链所需的全部信息
#[derive(Clone, Debug)]
pub struct LinkPlan {
    /// 资源类型
    pub resource: ResourceType,
    /// 实例 uuid
    pub instance: Uuid,
    /// 实例根目录
    pub base: PathBuf,
    /// 实例内目标文件
    pub target: PathBuf,
    /// 公共目录里的源文件
    pub source: PathBuf,
    /// 项目 ID
    pub pid: String,
    /// 文件 ID
    pub fid: String,
    /// 显示名
    pub name: String,
    /// 文件 SHA1
    pub hash: String,
    /// 下载地址
    pub url: String,
}

/// 公共模组索引，按项目 ID 分组
static MOD_ITEMS: LazyLock<RwLock<HashMap<String, Vec<ResourceInfoObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 公共资源包索引，按项目 ID 分组
static RESOURCE_PACK_ITEMS: LazyLock<RwLock<HashMap<String, Vec<ResourceInfoObj>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 索引与映射记录的"克隆快照 + 入队"串行化
///
/// 多个下载同时完成时，若各自克隆后再入队，先克隆的旧快照可能后入队，把新记录盖掉。
static SAVE_LOCK: Mutex<()> = Mutex::new(());

impl ResourceType {
    /// 公共存储目录
    fn download_dir(self) -> &'static PathBuf {
        match self {
            Self::Mod => MOD_DOWNLOAD_PATH.get().unwrap(),
            Self::ResourcePack => RESOURCE_PACK_DOWNLOAD_PATH.get().unwrap(),
        }
    }

    /// 索引内存表
    fn items(self) -> &'static RwLock<HashMap<String, Vec<ResourceInfoObj>>> {
        match self {
            Self::Mod => &MOD_ITEMS,
            Self::ResourcePack => &RESOURCE_PACK_ITEMS,
        }
    }

    /// 索引落盘文件
    fn index_file(self) -> &'static PathBuf {
        match self {
            Self::Mod => MOD_DOWNLOAD_FILE.get().unwrap(),
            Self::ResourcePack => RESOURCE_DOWNLOAD_FILE.get().unwrap(),
        }
    }

    /// 索引保存任务的 UUID
    fn save_uuid(self) -> Uuid {
        match self {
            Self::Mod => uuids::MOD_DOWNLOAD_INFO_UUID,
            Self::ResourcePack => uuids::RESOURCE_PACK_DOWNLOAD_INFO_UUID,
        }
    }
}

/// 设置资源映射总开关
///
/// - `enabled`: 是否启用；关闭后只保留已映射文件，不再处理新下载
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// 资源映射是否启用
///
/// # 返回值
///
/// 启用时为 `true`
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// 初始化公共存储
///
/// 公共目录建在下载目录下，索引落在运行目录；实例映射记录跟着实例目录走。
///
/// - `path`: 启动器运行根目录
///
/// # 返回值
///
/// 目录创建失败时返回对应错误
pub fn init<P: AsRef<Path>>(path: P) -> CoreResult<()> {
    let path = path.as_ref();
    let download = mml_downloader::get_download_path();

    let dir = MOD_DOWNLOAD_PATH.get_or_init(|| download.join(names::GAME_MODS_DIR));
    path_helper::create_dir_all(dir)?;
    let dir =
        RESOURCE_PACK_DOWNLOAD_PATH.get_or_init(|| download.join(names::GAME_RESOURCEPACKS_DIR));
    path_helper::create_dir_all(dir)?;

    MOD_DOWNLOAD_FILE.get_or_init(|| path.join(names::MOD_DOWNLOAD_FILE));
    RESOURCE_DOWNLOAD_FILE.get_or_init(|| path.join(names::RESOURCEPACK_DOWNLOAD_FILE));
    LINK_DATA.get_or_init(Default::default);

    read_mod_download_data();
    read_resourcepack_download_data();

    Ok(())
}

/// 读取公共模组索引
fn read_mod_download_data() {
    let file = MOD_DOWNLOAD_FILE.get().unwrap();
    if !file.is_file() {
        return;
    }

    match serialize_tools::json_from_file::<HashMap<String, Vec<ResourceInfoObj>>>(file) {
        Ok(data) => {
            *MOD_ITEMS.write().unwrap() = data;
        }
        Err(err) => mml_log::error_type(err),
    }
}

/// 读取公共资源包索引
fn read_resourcepack_download_data() {
    let file = RESOURCE_DOWNLOAD_FILE.get().unwrap();
    if !file.is_file() {
        return;
    }

    match serialize_tools::json_from_file::<HashMap<String, Vec<ResourceInfoObj>>>(file) {
        Ok(data) => {
            *RESOURCE_PACK_ITEMS.write().unwrap() = data;
        }
        Err(err) => mml_log::error_type(err),
    }
}

/// 读取一个实例的映射记录
///
/// - `base`: 实例根目录，记录文件就在它下面
///
/// # 返回值
///
/// 读不到或解析失败时返回空列表
fn read_link_data(base: &Path) -> Vec<LinkInfoObj> {
    let file = base.join(names::LINK_INFO_FILE);
    if !file.is_file() {
        return Vec::new();
    }

    match serialize_tools::json_from_file::<Vec<LinkInfoObj>>(&file) {
        Ok(data) => data,
        Err(err) => {
            mml_log::error_type(err);
            Vec::new()
        }
    }
}

/// 保存一份公共索引
///
/// - `resource`: 要保存的资源类型
fn save_index(resource: ResourceType) {
    let _guard = SAVE_LOCK.lock().unwrap();
    let data = resource.items().read().unwrap();
    config_save::save(resource.save_uuid(), &*data, resource.index_file());
}

/// 保存公共模组索引
fn save_mod_download_data() {
    save_index(ResourceType::Mod);
}

/// 保存公共资源包索引
fn save_resource_pack_download_data() {
    save_index(ResourceType::ResourcePack);
}

/// 保存一个实例的映射记录
///
/// - `instance`: 实例 uuid，用来派生保存任务
/// - `base`: 实例根目录，记录文件就写在这里
fn save_link_data(instance: Uuid, base: &Path) {
    let _guard = SAVE_LOCK.lock().unwrap();
    let data = link_data()
        .read()
        .unwrap()
        .get(&instance)
        .cloned()
        .unwrap_or_default();
    config_save::save(
        uuids::mix_uuid(instance, uuids::LINK_UUID),
        &data,
        base.join(names::LINK_INFO_FILE),
    );
}

/// 获取公共模组目录
///
/// # 返回值
///
/// 下载目录下的 `mods`
pub fn get_mod_download_path() -> PathBuf {
    MOD_DOWNLOAD_PATH.get().unwrap().clone()
}

/// 获取公共资源包目录
///
/// # 返回值
///
/// 下载目录下的 `resourcepacks`
pub fn get_resource_pack_download_path() -> PathBuf {
    RESOURCE_PACK_DOWNLOAD_PATH.get().unwrap().clone()
}

/// 记录一份公共文件
///
/// - `resource`: 资源类型
/// - `info`: 文件信息，`hash` 必须是非空 SHA1
/// - `pid`: 项目 ID，同一项目可以有多个版本
pub fn add_download_info(resource: ResourceType, mut info: ResourceInfoObj, pid: String) {
    let Some(hash) = normalize_hash(&info.hash) else {
        return;
    };
    info.pid = pid.clone();
    info.hash = hash;
    if let Some(file) = Path::new(&info.file)
        .file_name()
        .and_then(|file| file.to_str())
    {
        info.file = file.to_string();
    }

    {
        let mut items = resource.items().write().unwrap();
        let list = items.entry(pid).or_default();
        if let Some(old) = list.iter_mut().find(|item| item.hash == info.hash) {
            old.name = info.name;
            old.file = info.file;
            old.url = info.url;
        } else {
            list.push(info);
        }
    }

    match resource {
        ResourceType::Mod => save_mod_download_data(),
        ResourceType::ResourcePack => save_resource_pack_download_data(),
    }
}

/// 获取实例的映射记录
///
/// - `uuid`: 实例 uuid
/// - `instance_base`: 实例根目录，记录文件就在它下面
///
/// # 返回值
///
/// 该实例记录过的映射；没有记录时为空列表
pub fn get_instance_link_data(uuid: Uuid, instance_base: &Path) -> Vec<LinkInfoObj> {
    if let Some(list) = link_data().read().unwrap().get(&uuid) {
        return list.clone();
    }

    let list = read_link_data(instance_base);
    link_data().write().unwrap().insert(uuid, list.clone());
    list
}

/// 按项目 ID 取一条公共索引记录
///
/// 同一项目可能记着多个版本，取最后写入的那条。
///
/// - resource: 资源类型
/// - pid: 项目 ID
///
/// # 返回值
///
/// 找到时返回索引记录；否则 None
pub fn get_download_info(resource: ResourceType, pid: &str) -> Option<ResourceInfoObj> {
    let items = resource.items().read().unwrap();
    items.get(pid)?.last().cloned()
}

/// 公共存储中是否已有该 SHA1 的文件
///
/// - `resource`: 资源类型
/// - `hash`: 文件 SHA1，大小写不敏感
///
/// # 返回值
///
/// 找到且校验通过时返回公共文件路径
pub fn find_file(resource: ResourceType, hash: &str) -> Option<PathBuf> {
    let hash = normalize_hash(hash)?;
    {
        let items = resource.items().read().unwrap();
        for list in items.values() {
            for item in list {
                if !item.hash.eq_ignore_ascii_case(&hash) {
                    continue;
                }
                let file = resource.download_dir().join(&item.file);
                // 索引是下载完成时记的，文件在就当作有效；每次启动都对几百个模组重算
                // SHA1 太慢，坏文件留给实际加载报错
                if file.is_file() {
                    return Some(file);
                }
            }
        }
    }

    // 索引丢失或用户手动放进公共目录：按"哈希.扩展名"再扫一遍
    let dir = fs::read_dir(resource.download_dir()).ok()?;
    for entry in dir.flatten() {
        let file = entry.path();
        if !file.is_file() {
            continue;
        }
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.len() >= hash.len()
            && name
                .get(..hash.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(&hash))
            && hash_matches(&file, &hash)
        {
            return Some(file);
        }
    }

    None
}

/// 公共存储中的下载目标路径
///
/// 文件名沿用原始文件名，同一个文件在多个实例间共用一个路径。
/// 同名文件内容不同时加 SHA1 前 8 位后缀，不覆盖已有文件。
///
/// - `resource`: 资源类型
/// - `hash`: 文件 SHA1
/// - `name`: 原始文件名
///
/// # 返回值
///
/// 哈希非法或目录创建失败时返回 `None`
pub fn download_path(resource: ResourceType, hash: &str, name: &str) -> Option<PathBuf> {
    let hash = normalize_hash(hash)?;
    let file_name = file_name(name).unwrap_or_else(|| hash.clone());
    let base = resource.download_dir().join(file_name);
    if let Some(parent) = base.parent()
        && let Err(err) = path_helper::create_dir_all(parent)
    {
        mml_log::error_type(err);
        return None;
    }

    // 同名文件内容不同：保留原来的文件，给新文件加哈希后缀，不能覆盖别人的模组
    if !base.exists() || hash_matches(&base, &hash) {
        return Some(base);
    }
    Some(unique_file(&base, &hash))
}

/// 为一次新下载准备公共目录路径
///
/// 公共目录已有该文件时直接链接到实例并把目标从下载列表里去掉；没有时返回一份
/// 下载计划，调用方把下载目标改成 plan.source，下载完成后调用 finish_plan。
///
/// - resource: 资源类型
/// - instance: 实例 uuid
/// - instance_base: 实例根目录
/// - target: 实例内目标文件
/// - pid / fid / name / hash / url: 资源元数据
/// - verify_target: 目标已存在时是否核对哈希；启动前检查可传 false，避免每次启动重算全部文件
///
/// # 返回值
///
/// 需要下载时返回计划；公共目录已有并完成链接、或目标已存在且无需处理时返回 None
#[allow(clippy::too_many_arguments)]
pub fn plan_download(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
    verify_target: bool,
) -> Option<LinkPlan> {
    if !is_enabled() {
        return None;
    }
    let hash = normalize_hash(hash)?;

    if let Some(source) = find_file(resource, &hash) {
        // 索引是下载完成时记的；文件真被改坏时删掉重下，别把坏文件硬链接出去
        if !hash_matches(&source, &hash) {
            let _ = path_helper::delete(&source);
            return Some(LinkPlan {
                resource,
                instance,
                base: instance_base.to_path_buf(),
                target: target.to_path_buf(),
                source,
                pid: pid.to_string(),
                fid: fid.to_string(),
                name: name.to_string(),
                hash,
                url: url.to_string(),
            });
        }

        let source = prefer_source(resource, &hash, target, source);
        if target.exists() {
            // 实例里是同一份文件就不用动；内容不同则补一份链接计划，让调用方覆盖它
            if !verify_target || hash_matches(target, &hash) {
                return None;
            }
            return Some(LinkPlan {
                resource,
                instance,
                base: instance_base.to_path_buf(),
                target: target.to_path_buf(),
                source,
                pid: pid.to_string(),
                fid: fid.to_string(),
                name: name.to_string(),
                hash,
                url: url.to_string(),
            });
        }

        if let Err(err) = try_link(
            resource,
            instance,
            instance_base,
            target,
            pid,
            fid,
            name,
            &hash,
            url,
        ) {
            mml_log::error_type(err);
        }
        return None;
    }

    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name);
    let source = download_path(resource, &hash, target_name)?;
    Some(LinkPlan {
        resource,
        instance,
        base: instance_base.to_path_buf(),
        target: target.to_path_buf(),
        source,
        pid: pid.to_string(),
        fid: fid.to_string(),
        name: name.to_string(),
        hash,
        url: url.to_string(),
    })
}

/// 尝试把公共文件硬链接到实例
///
/// 目标已存在时不覆盖，交给调用方走正常下载与收尾流程。
///
/// - `resource`: 资源类型
/// - `instance`: 实例 uuid
/// - `instance_base`: 实例根目录，用来记相对路径
/// - `target`: 实例内目标文件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `name`: 显示名
/// - `hash`: 文件 SHA1
/// - `url`: 下载地址
///
/// # 返回值
///
/// 成功复用并完成映射时返回 `true`；公共存储没有该文件或目标已存在时返回 `false`
pub fn try_link(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
) -> CoreResult<bool> {
    let Some(hash) = normalize_hash(hash) else {
        return Ok(false);
    };
    let Some(source) = find_file(resource, &hash) else {
        return Ok(false);
    };
    if !hash_matches(&source, &hash) {
        return Ok(false);
    }
    let source = prefer_source(resource, &hash, target, source);
    if target.exists() {
        return Ok(false);
    }

    let is_link = link_file(&source, target)?;
    ensure_link_data(instance, instance_base);
    add_download_info(
        resource,
        ResourceInfoObj {
            pid: pid.to_string(),
            name: name.to_string(),
            hash,
            file: source
                .file_name()
                .and_then(|file| file.to_str())
                .unwrap_or_default()
                .to_string(),
            url: url.to_string(),
        },
        pid.to_string(),
    );
    record_link(instance, instance_base, target, pid, fid, is_link);
    save_link_data(instance, instance_base);
    Ok(true)
}

/// 下载完成后登记公共文件并链接到实例
///
/// - `resource`: 资源类型
/// - `instance`: 实例 uuid
/// - `instance_base`: 实例根目录，用来记相对路径
/// - `target`: 实例内目标文件
/// - `source`: 已下载到公共目录的文件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `name`: 显示名
/// - `hash`: 文件 SHA1
/// - `url`: 下载地址
///
/// # 返回值
///
/// 链接或复制失败时返回对应错误
pub fn finish_download(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    source: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
) -> CoreResult<()> {
    // 下载器已经校验过；这里再确认一次，避免把坏文件记进索引
    let Some(hash) = normalize_hash(hash) else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    if !hash_matches(source, &hash) {
        return Err(ErrorType::DownloadFileHashError(
            DownloadFileHashErrorData {
                file: source.to_path_buf(),
                now: String::new(),
                hash,
            },
        ));
    }

    let is_link = link_file(source, target)?;
    ensure_link_data(instance, instance_base);
    add_download_info(
        resource,
        ResourceInfoObj {
            pid: pid.to_string(),
            name: name.to_string(),
            hash,
            file: source
                .file_name()
                .and_then(|file| file.to_str())
                .unwrap_or_default()
                .to_string(),
            url: url.to_string(),
        },
        pid.to_string(),
    );
    record_link(instance, instance_base, target, pid, fid, is_link);
    save_link_data(instance, instance_base);
    Ok(())
}

/// 完成一次下载计划：把公共目录里的文件链接到实例
///
/// - `plan`: plan_download 返回的计划
///
/// # 返回值
///
/// 链接或复制失败时返回对应错误
pub fn finish_plan(plan: &LinkPlan) -> CoreResult<()> {
    finish_download(
        plan.resource,
        plan.instance,
        &plan.base,
        &plan.target,
        &plan.source,
        &plan.pid,
        &plan.fid,
        &plan.name,
        &plan.hash,
        &plan.url,
    )
}

/// 把实例内一个已经存在的文件映射进公共目录
///
/// 用于更新这类先下到实例、再收尾的链路：公共目录有同哈希文件就补链，没有就把实例文件
/// 复制过去再链接。实例里没有文件时返回错误。
///
/// - resource: 资源类型
/// - instance: 实例 uuid
/// - instance_base: 实例根目录
/// - target: 实例内文件
/// - pid / fid / name / hash / url: 资源元数据
///
/// # 返回值
///
/// 映射失败时返回对应错误
#[allow(clippy::too_many_arguments)]
pub fn map_existing(
    resource: ResourceType,
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    name: &str,
    hash: &str,
    url: &str,
) -> CoreResult<()> {
    if !is_enabled() {
        return Ok(());
    }
    let Some(hash) = normalize_hash(hash) else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name);
    let source = if let Some(source) = find_file(resource, &hash) {
        if hash_matches(&source, &hash) {
            prefer_source(resource, &hash, target, source)
        } else {
            let _ = path_helper::delete(&source);
            download_path(resource, &hash, target_name)
                .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?
        }
    } else {
        download_path(resource, &hash, target_name)
            .ok_or(ErrorType::DataNotFound(DataNotFoundData::Info))?
    };

    let plan = LinkPlan {
        resource,
        instance,
        base: instance_base.to_path_buf(),
        target: target.to_path_buf(),
        source,
        pid: pid.to_string(),
        fid: fid.to_string(),
        name: name.to_string(),
        hash,
        url: url.to_string(),
    };
    if plan.source.is_file() {
        finish_plan(&plan)
    } else {
        adopt_existing(&plan)
    }
}

/// 把旧的 `<sha1>.扩展名` 公共文件改成实例里的真实文件名
///
/// 只改文件名以哈希命名的旧文件；已有真实目标名时保持原样。
///
/// - resource: 资源类型
/// - hash: 小写 SHA1
/// - target: 实例内目标文件，用来取真实文件名
/// - source: 当前公共文件
///
/// # 返回值
///
/// 迁移后或保持不变的公共文件路径
fn prefer_source(resource: ResourceType, hash: &str, target: &Path, source: PathBuf) -> PathBuf {
    let Some(name) = target.file_name().and_then(|name| name.to_str()) else {
        return source;
    };
    let Some(name) = file_name(name) else {
        return source;
    };
    if source.file_stem().and_then(|stem| stem.to_str()) != Some(hash) {
        return source;
    }

    let preferred = resource.download_dir().join(&name);
    if preferred == source || preferred.exists() {
        return source;
    }
    if let Err(err) = path_helper::move_file(&source, &preferred) {
        mml_log::error_type(err);
        return source;
    }

    if let Some(old) = source.file_name().and_then(|name| name.to_str()) {
        rename_index_file(resource, old, &name);
    }
    preferred
}

/// 把索引里指向旧文件名的记录改成新文件名
///
/// - resource: 资源类型
/// - old: 旧公共文件名
/// - new: 新公共文件名
fn rename_index_file(resource: ResourceType, old: &str, new: &str) {
    {
        let mut items = resource.items().write().unwrap();
        for list in items.values_mut() {
            for item in list {
                if item.file == old {
                    item.file = new.to_string();
                }
            }
        }
    }
    save_index(resource);
}

/// 下载失败但实例里已有同哈希文件时，把它复制进公共目录再补链
///
/// - `plan`: 本次下载计划
///
/// # 返回值
///
/// 目标不存在或哈希不符时返回错误
pub fn adopt_existing(plan: &LinkPlan) -> CoreResult<()> {
    if !plan.target.is_file() || !hash_matches(&plan.target, &plan.hash) {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    }
    if let Some(parent) = plan.source.parent() {
        path_helper::create_dir_all(parent)?;
    }
    // 搬运而不是复制：实例文件先移进公共目录，再硬链接回原位
    path_helper::move_file(&plan.target, &plan.source)?;
    finish_plan(plan)
}

/// 把公共文件链接到实例，硬链接失败时回退为复制
///
/// - `source`: 公共文件
/// - `target`: 实例内目标文件
///
/// # 返回值
///
/// 成功时返回是否真的建成了硬链接；目标是已存在文件时会先移入回收站
fn link_file(source: &Path, target: &Path) -> CoreResult<bool> {
    if let Some(parent) = target.parent() {
        path_helper::create_dir_all(parent)?;
    }
    if target.exists() {
        path_helper::move_to_trash(target)?;
    }

    match path_helper::hard_link(source, target) {
        Ok(()) => Ok(true),
        Err(_) => {
            path_helper::copy_file(source, target)?;
            Ok(false)
        }
    }
}

/// 确保实例映射已从实例目录读进内存
///
/// 记录文件随实例走；第一次写入前不读的话，保存会用空表覆盖已有记录。
///
/// - `instance`: 实例 uuid
/// - `base`: 实例根目录
fn ensure_link_data(instance: Uuid, base: &Path) {
    let mut data = link_data().write().unwrap();
    if !data.contains_key(&instance) {
        let list = read_link_data(base);
        data.insert(instance, list);
    }
}

/// 记录实例内的一条映射
///
/// - `instance`: 实例 uuid
/// - `instance_base`: 实例根目录
/// - `target`: 实例内目标文件
/// - `pid`: 项目 ID
/// - `fid`: 文件 ID
/// - `is_link`: 是否真的建成了硬链接
fn record_link(
    instance: Uuid,
    instance_base: &Path,
    target: &Path,
    pid: &str,
    fid: &str,
    is_link: bool,
) {
    let path = target
        .strip_prefix(instance_base)
        .unwrap_or(target)
        .to_string_lossy()
        .replace('\\', "/");

    let mut data = link_data().write().unwrap();
    let list = data.entry(instance).or_default();
    // 同一项目换文件（更新）时只保留新路径，旧路径的检查记录要一起清掉
    list.retain(|item| item.pid != pid || item.path == path);
    if let Some(old) = list.iter_mut().find(|item| item.path == path) {
        old.pid = pid.to_string();
        old.fid = fid.to_string();
        old.is_link = is_link;
    } else {
        list.push(LinkInfoObj {
            is_link,
            pid: pid.to_string(),
            fid: fid.to_string(),
            path,
        });
    }
}

/// 删除实例内一条映射记录
///
/// 用户删除资源时调用：否则下次启动检查会按旧记录把它重新链回来。
///
/// - instance: 实例 uuid
/// - instance_base: 实例根目录
/// - target: 已删除的实例内文件
pub fn remove_link(instance: Uuid, instance_base: &Path, target: &Path) {
    let path = target
        .strip_prefix(instance_base)
        .unwrap_or(target)
        .to_string_lossy()
        .replace('\\', "/");

    ensure_link_data(instance, instance_base);
    {
        let mut data = link_data().write().unwrap();
        if let Some(list) = data.get_mut(&instance) {
            list.retain(|item| item.path != path);
        }
    }
    save_link_data(instance, instance_base);
}

/// 实例映射记录表
///
/// # 返回值
///
/// 全局映射表；未初始化时懒建立空表
fn link_data() -> &'static RwLock<HashMap<Uuid, Vec<LinkInfoObj>>> {
    LINK_DATA.get_or_init(Default::default)
}

/// 规范化 SHA1
///
/// - `hash`: 原始哈希
///
/// # 返回值
///
/// 合法时返回小写 SHA1，否则 `None`
fn normalize_hash(hash: &str) -> Option<String> {
    let hash = hash.trim();
    if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(hash.to_ascii_lowercase())
}

/// 从原始文件名取出安全的单段文件名
///
/// Windows 不允许的字符替换成下划线；空名或路径片段返回 None。
///
/// - name: 原始文件名，可能带目录
///
/// # 返回值
///
/// 可用的文件名；无法得到时返回 None
fn file_name(name: &str) -> Option<String> {
    let file = Path::new(name).file_name()?.to_str()?.trim();
    if file.is_empty() || file == "." || file == ".." {
        return None;
    }

    let file: String = file
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '|' | '?' | '*') {
                '_'
            } else {
                ch
            }
        })
        .collect();
    if file.is_empty() || file == "." || file == ".." {
        None
    } else {
        Some(file)
    }
}

/// 同名冲突时的候选文件名：名字.sha1前8位.扩展名
///
/// - base: 原目标路径
/// - hash: 小写 SHA1
///
/// # 返回值
///
/// 带哈希后缀的路径
fn collide_file(base: &Path, hash: &str) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file");
    let short = &hash[..hash.len().min(8)];
    let name = match base.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{stem}.{short}.{ext}"),
        _ => format!("{stem}.{short}"),
    };
    base.with_file_name(name)
}

/// 为冲突文件找一个不覆盖已有内容的路径
///
/// - base: 原目标路径
/// - hash: 新文件 SHA1
///
/// # 返回值
///
/// 可写入的路径；已存在同哈希文件时返回它
fn unique_file(base: &Path, hash: &str) -> PathBuf {
    let mut file = collide_file(base, hash);
    if file.is_file() && hash_matches(&file, hash) {
        return file;
    }

    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file");
    let short = &hash[..hash.len().min(8)];
    let ext = base.extension().and_then(|ext| ext.to_str());
    let mut index = 1;
    loop {
        let name = match ext {
            Some(ext) if !ext.is_empty() => format!("{stem}.{short}-{index}.{ext}"),
            _ => format!("{stem}.{short}-{index}"),
        };
        file = base.with_file_name(name);
        if !file.exists() || hash_matches(&file, hash) {
            return file;
        }
        index += 1;
    }
}

/// 校验文件 SHA1
///
/// - `file`: 待校验文件
/// - `hash`: 小写 SHA1
///
/// # 返回值
///
/// 能读出文件且哈希一致时为 `true`
fn hash_matches(file: &Path, hash: &str) -> bool {
    let Ok(mut stream) = path_helper::open_read(file) else {
        return false;
    };
    let Ok(now) = hash_helper::gen_hash_from_reader(HashType::Sha1, &mut stream) else {
        return false;
    };
    now.eq_ignore_ascii_case(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 公共文件名保留真实文件名，只清掉路径片段和非法字符
    #[test]
    fn file_name_keeps_real_name() {
        assert_eq!(file_name("Demo.JAR").as_deref(), Some("Demo.JAR"));
        assert_eq!(file_name("dir/foo.jar").as_deref(), Some("foo.jar"));
        assert_eq!(file_name("bad<name>.jar").as_deref(), Some("bad_name_.jar"));
        assert!(file_name("").is_none());
    }

    /// 非 SHA1 不接受
    #[test]
    fn hash_must_be_sha1() {
        assert!(normalize_hash("").is_none());
        assert!(normalize_hash("abc").is_none());
        assert!(normalize_hash(&"g".repeat(40)).is_none());
        assert_eq!(
            normalize_hash(&"A".repeat(40)).as_deref(),
            Some("a".repeat(40).as_str())
        );
    }

    /// 映射记录落在实例目录，不落在启动器运行目录
    #[test]
    fn link_info_is_written_inside_instance() {
        let dir = mml_testutil::temp_path(format!("mml-link-{}", Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        mml_downloader::init(&dir).unwrap();
        init(&dir).unwrap();
        set_enabled(true);
        config_save::start();

        let hash = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
        let source = download_path(ResourceType::Mod, hash, "demo.jar").unwrap();
        assert_eq!(
            source.file_name().and_then(|name| name.to_str()),
            Some("demo.jar")
        );
        fs::write(&source, b"hello").unwrap();
        add_download_info(
            ResourceType::Mod,
            ResourceInfoObj {
                pid: "p".to_string(),
                name: "Demo".to_string(),
                hash: hash.to_string(),
                file: "demo.jar".to_string(),
                url: "https://example.com/demo.jar".to_string(),
            },
            "p".to_string(),
        );

        let instance = Uuid::new_v4();
        let base = dir.join("instance");
        let target = base.join(".minecraft").join("mods").join("demo.jar");
        assert!(
            try_link(
                ResourceType::Mod,
                instance,
                &base,
                &target,
                "p",
                "f",
                "Demo",
                hash,
                "https://example.com/demo.jar"
            )
            .unwrap()
        );
        assert!(target.is_file());

        // 第二个实例命中同一份公共文件，直接链接，不再下载
        let instance2 = Uuid::new_v4();
        let base2 = dir.join("instance2");
        let target2 = base2.join(".minecraft").join("mods").join("demo.jar");
        assert!(
            try_link(
                ResourceType::Mod,
                instance2,
                &base2,
                &target2,
                "p",
                "f",
                "Demo",
                hash,
                "https://example.com/demo.jar"
            )
            .unwrap()
        );
        assert!(target2.is_file());

        // 旧的哈希文件名要原地改成真实文件名，同时把实例里的文件搬进去
        let world_hash = "7c211433f02071597741e6ff5a8ea34789abbf43";
        let old_central = get_mod_download_path().join(format!("{world_hash}.jar"));
        fs::write(&old_central, b"world").unwrap();
        add_download_info(
            ResourceType::Mod,
            ResourceInfoObj {
                pid: "p3".to_string(),
                name: "World".to_string(),
                hash: world_hash.to_string(),
                file: format!("{world_hash}.jar"),
                url: "https://example.com/world.jar".to_string(),
            },
            "p3".to_string(),
        );
        let instance3 = Uuid::new_v4();
        let base3 = dir.join("instance3");
        let target3 = base3.join(".minecraft").join("mods").join("world.jar");
        fs::create_dir_all(target3.parent().unwrap()).unwrap();
        fs::write(&target3, b"world").unwrap();
        map_existing(
            ResourceType::Mod,
            instance3,
            &base3,
            &target3,
            "p3",
            "f3",
            "World",
            world_hash,
            "https://example.com/world.jar",
        )
        .unwrap();
        assert!(get_mod_download_path().join("world.jar").is_file());
        assert!(!old_central.exists());
        assert_eq!(fs::read(&target3).unwrap(), b"world",);

        config_save::stop();

        assert!(!dir.join(names::LINK_INFO_FILE).exists());
        let file = base.join(names::LINK_INFO_FILE);
        let data: Vec<LinkInfoObj> =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].path, ".minecraft/mods/demo.jar");
        assert!(data[0].is_link);

        let index: HashMap<String, Vec<ResourceInfoObj>> =
            serde_json::from_str(&fs::read_to_string(dir.join(names::MOD_DOWNLOAD_FILE)).unwrap())
                .unwrap();
        assert!(
            index
                .values()
                .flatten()
                .any(|item| { item.url == "https://example.com/demo.jar" })
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
