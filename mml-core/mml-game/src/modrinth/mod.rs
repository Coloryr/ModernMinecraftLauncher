//! Modrinth 内容查询与整合包安装

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, OnceLock},
};

use mml_base::{
    archives::{BaseArchive, BaseArchiveGui},
    file_item::{FileHash, FileItemObj, LaterRun},
    tools,
};
use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};
use mml_net::{
    modrinth_api::{
        self, ModrinthCategoriesObj,
        search_obj::HitObj,
        version_obj::{DependencieObj, ModrinthVersionObj},
    },
    urls,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tokio_util::sync::CancellationToken;

use crate::{
    GameInstance,
    data_res::DownloadItemRes,
    gui_hook::{AddModPackGui, AddModPackState},
    launcher::{
        FileType, file_online_info_obj::OnlineInfoObj, instance_setting_obj::InstanceSettingObj,
    },
    loader::LoaderType,
    modpack::{BaseModPackWorker, ModPackWorker, modrinth_worker::ModrinthPackWorker},
    modrinth::pack_obj::{ModrinthPackFileObj, ModrinthPackObj},
};
use crate::error::{modrinth_avatar_err};

pub mod pack_obj;

/// 分类信息缓存
static CATEGORISE: OnceLock<Vec<ModrinthCategoriesObj>> = OnceLock::new();
/// 加载器名缓存，即 `tag/loader` 的名字表
static LOADERS: OnceLock<Vec<String>> = OnceLock::new();
/// 游戏版本列表缓存
static GAME_VERSIONS: OnceLock<Vec<String>> = OnceLock::new();

/// 转换为 Modrinth 的加载器 ID
///
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回加载器 ID；该加载器不被 Modrinth 支持返回 `None`
pub fn to_loader_id(loader: &LoaderType) -> Option<String> {
    match loader {
        LoaderType::Forge => Some(loader.id().to_string()),
        LoaderType::Fabric => Some(loader.id().to_string()),
        LoaderType::Quilt => Some(loader.id().to_string()),
        LoaderType::NeoForge => Some(loader.id().to_string()),
        _ => None,
    }
}

/// 创建下载项目
///
/// - `obj`: Modrinth 版本信息
/// - `path`: 保存目录
///
/// # 返回值
///
/// 返回主文件下载项
pub fn make_download_obj<P: AsRef<Path>>(obj: &ModrinthVersionObj, path: P) -> FileItemObj {
    let file = obj
        .files
        .iter()
        .filter(|item| item.primary)
        .next()
        .unwrap_or(&obj.files[0]);
    FileItemObj {
        name: obj.name.clone(),
        file: path.as_ref().join(&file.filename),
        url: file.url.clone(),
        hash: FileHash::Sha1Sha512(file.hashes.sha1.clone(), file.hashes.sha512.clone()),
        later: LaterRun::None,
    }
}

/// 创建整合包内文件的下载项目
///
/// - `obj`: 整合包文件信息
/// - `path`: 保存目录
///
/// # 返回值
///
/// 返回下载项
pub fn make_pack_download_obj<P: AsRef<Path>>(obj: &ModrinthPackFileObj, path: P) -> FileItemObj {
    FileItemObj {
        url: obj.downloads[0].clone(),
        name: obj.path.clone(),
        file: path.as_ref().join(&obj.path),
        hash: FileHash::Sha1Sha512(obj.hashes.sha1.clone(), obj.hashes.sha512.clone()),
        ..Default::default()
    }
}

/// 创建在线文件信息
///
/// - `obj`: Modrinth 版本信息
/// - `path`: 实例内相对目录
///
/// # 返回值
///
/// 返回在线文件信息
pub fn make_file_online_obj(obj: &ModrinthVersionObj, path: &str) -> OnlineInfoObj {
    let file = obj
        .files
        .iter()
        .filter(|item| item.primary)
        .next()
        .unwrap_or(&obj.files[0]);
    OnlineInfoObj {
        path: path.to_string(),
        name: obj.name.clone(),
        file: file.filename.clone(),
        sha1: file.hashes.sha1.clone(),
        url: file.url.clone(),
        modid: obj.project_id.clone(),
        fileid: obj.id.clone(),
    }
}

/// 获取分类图标地址
///
/// - `name`: 分类名
///
/// # 返回值
///
/// 返回图标地址；分类不存在返回 `None`
pub async fn get_categories_icon(name: &str) -> Option<String> {
    let cap = match CATEGORISE.get() {
        Some(cap) => cap,
        None => {
            let list = modrinth_api::get_categories().await;
            match list {
                Ok(list) => CATEGORISE.get_or_init(|| list),
                Err(err) => {
                    mml_log::error_type(err);
                    &Vec::new()
                }
            }
        }
    };

    cap.iter()
        .find(|item| item.name == name)
        .map(|item| item.icon.clone())
}

/// 获取分组
///
/// - `file_type`: 项目文件类型
///
/// # 返回值
///
/// 返回分类名映射表；查询失败返回对应错误
pub async fn get_categories(file_type: FileType) -> CoreResult<HashMap<String, String>> {
    let cap = match CATEGORISE.get() {
        Some(cap) => cap,
        None => {
            let list = modrinth_api::get_categories().await?;
            CATEGORISE.get_or_init(|| list)
        }
    };

    let project_type = match file_type {
        FileType::Shaderpack => modrinth_api::CLASS_SHADERPACK,
        FileType::Resourcepack => modrinth_api::CLASS_RESOURCEPACK,
        FileType::Modpack => modrinth_api::CLASS_MODPACK,
        _ => modrinth_api::CLASS_MOD,
    };

    let map = cap
        .iter()
        .filter(|item| item.project_type == project_type && item.header == "categories")
        .map(|item| (item.name.clone(), item.name.clone()))
        .collect();

    Ok(map)
}

/// 把搜索命中的标签排成"加载器在前、分类在后"
///
/// Modrinth 的搜索接口把加载器与分类混在同一个 `categories` 数组里返回（命中项没有
/// `loaders` 字段），顺序由服务端定，于是分类会夹在加载器中间。这里按 `tag/loader`
/// 的名字表把加载器认出来整体挪到前面，认不出来的一律当分类、保持原顺序 ——
/// 与项目详情页一致（那边接口里 `loaders` / `categories` 本来就是分开的）。
///
/// - `names`: 命中项的 `categories`，原样传入
///
/// # 返回值
///
/// 排好序的标签名；加载器表取不到时原样返回
pub async fn order_tags(names: &[String]) -> Vec<String> {
    let loaders = match LOADERS.get() {
        Some(list) => list,
        None => {
            let list = modrinth_api::get_loaders().await;
            match list {
                Ok(list) => {
                    LOADERS.get_or_init(|| list.into_iter().map(|item| item.name).collect())
                }
                Err(err) => {
                    mml_log::error_type(err);
                    &Vec::new()
                }
            }
        }
    };

    let (loader_tags, rest): (Vec<&String>, Vec<&String>) =
        names.iter().partition(|name| loaders.contains(*name));

    loader_tags.into_iter().chain(rest).cloned().collect()
}

/// 一批命中项里去重后的作者 ID
///
/// - `hits`: 搜索命中项
///
/// # 返回值
///
/// 按出现顺序去重的作者 ID；空 ID 跳过
fn author_ids(hits: &[HitObj]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for hit in hits {
        if !hit.author_id.is_empty() && !ids.contains(&hit.author_id) {
            ids.push(hit.author_id.clone());
        }
    }
    ids
}

/// 批量取一批搜索命中项的作者头像
///
/// Modrinth 的搜索结果只给作者名，不给头像；头像按作者 ID 另查一次，
/// 一批 ID 走一次请求。查不到（接口出错等）就少几个头像，不影响列表本身。
///
/// - `hits`: 搜索命中项，只读其中的 `author_id`
///
/// # 返回值
///
/// 作者 ID → 头像 URL；一个都没查到或请求失败时是空表
pub async fn author_avatars(hits: &[HitObj]) -> HashMap<String, String> {
    let ids = author_ids(hits);
    if ids.is_empty() {
        return HashMap::new();
    }

    match modrinth_api::get_users(&ids).await {
        Ok(list) => list
            .into_iter()
            .filter_map(|item| item.avatar_url.map(|url| (item.id, url)))
            .collect(),
        Err(err) => {
            mml_log::error_type(modrinth_avatar_err(err));
            HashMap::new()
        }
    }
}

/// 获取所有游戏版本
///
/// # 返回值
///
/// 返回游戏版本列表，首位为空串表示不限；查询失败返回对应错误
pub async fn get_game_versions() -> CoreResult<Vec<String>> {
    match GAME_VERSIONS.get() {
        Some(versions) => Ok(versions.clone()),
        None => {
            let list = modrinth_api::get_game_versions().await?;
            let mut list1: Vec<String> = list.into_iter().map(|item| item.version).collect();

            list1.insert(0, String::new());

            Ok(GAME_VERSIONS.get_or_init(|| list1).clone())
        }
    }
}

/// 获取整合包模组信息
///
/// - `path`: 整合包解压目录
/// - `info`: 整合包信息
/// - `pack_gui`: 整合包安装界面回调
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 返回文件下载项列表与在线信息表；取消返回 `TaskCancel`
pub async fn get_mod_info<P: AsRef<Path>>(
    path: P,
    info: &ModrinthPackObj,
    pack_gui: AddModPackGui,
    cancel: CancellationToken,
) -> CoreResult<DownloadItemRes> {
    let mut list = Vec::new();
    let mut mods = HashMap::new();

    let size = info.files.len();
    let mut now = 0usize;

    let mut hash = HashMap::new();

    for item in info.files.iter() {
        if cancel.is_cancelled() {
            return Err(ErrorType::TaskCancel);
        }

        let item1 = make_pack_download_obj(item, &path);
        let url = item
            .downloads
            .iter()
            .filter(|item| item.starts_with(&format!("{}data/", urls::MODRINTH_DOWNLOAD)))
            .next();

        let sha1 = item.hashes.sha1.clone();
        let path = tools::get_path_part(&item.path);
        // 有隐藏信息
        if let Some(data) = &item.project {
            mods.remove(&data.pid);
            mods.insert(
                data.pid.clone(),
                OnlineInfoObj {
                    path: path.parent,
                    name: path.file.clone(),
                    file: path.file,
                    sha1,
                    url: item1.url.clone(),
                    modid: data.pid.clone(),
                    fileid: data.fid.clone(),
                },
            );
        } else if let Some(url) = url {
            // 是modr的标准下载地址
            let modid = tools::get_string(url, "data/", "/ver");
            let fileid = tools::get_string(url, "versions/", "/");

            mods.remove(&modid);
            mods.insert(
                modid.clone(),
                OnlineInfoObj {
                    path: path.parent,
                    name: path.file.clone(),
                    file: path.file,
                    sha1,
                    url: item1.url.clone(),
                    modid,
                    fileid,
                },
            );
        } else {
            // 尝试从hash获取版本信息
            hash.insert(
                item.hashes.sha512.clone(),
                OnlineInfoObj {
                    path: path.parent,
                    name: path.file.clone(),
                    file: path.file,
                    sha1,
                    url: item1.url.clone(),
                    ..Default::default()
                },
            );
        }

        now += 1;

        if let Some(gui) = &pack_gui {
            gui.set_sub_now(now, Some(size));
        }

        list.push(item1);
    }

    if !hash.is_empty() {
        let ids = hash.keys().cloned().collect();
        let data = modrinth_api::get_version_from_sha512(ids).await;
        if let Ok(data) = data {
            for (key, value) in data {
                let mut temp = hash.remove(&key).unwrap();
                temp.modid = value.project_id;
                temp.fileid = value.id;

                mods.remove(&temp.modid);
                mods.insert(temp.modid.clone(), temp);
            }
        }
    }

    Ok(DownloadItemRes { list, online: mods })
}

/// 模组依赖列表
pub struct ModrinthModDependenciesRes {
    /// 名字
    pub name: String,
    /// 图标地址，项目接口给的是远程地址，没有时是 None
    pub icon: Option<String>,
    /// 项目编号
    pub mod_id: String,
    /// 是否可选
    pub opt: bool,
    /// 候选文件，按"合适程度"排好，**第一个是自动选中的那个**；调用方可以让用户改选
    pub list: Vec<ModrinthVersionObj>,
}

/// 获取模组依赖
///
/// - `obj`: Modrinth 文件信息
/// - `version`: 游戏版本
/// - `loader`: 加载器类型
///
/// # 返回值
///
/// 返回依赖列表，含递归解析出的子依赖，已去重
pub async fn get_mod_dependencies(
    obj: &ModrinthVersionObj,
    version: &str,
    loader: LoaderType,
) -> Vec<ModrinthModDependenciesRes> {
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
    dependencies: &Vec<DependencieObj>,
    version: &str,
    loader: &LoaderType,
    ids: &Mutex<HashSet<String>>,
    handle: &tokio::runtime::Handle,
) -> Vec<ModrinthModDependenciesRes> {
    let list: Mutex<Vec<ModrinthModDependenciesRes>> = Mutex::new(Vec::new());

    dependencies.par_iter().for_each(|item| {
        // 无 project_id 的依赖是打包内置文件，如 Mod Menu Helper.zip，无法通过项目 API 解析
        let Some(project_id) = &item.project_id else {
            return;
        };

        // 原子检查并插入：HashSet::insert 在元素已存在时返回 false
        {
            let mut ids_guard = ids.lock().unwrap();
            if !ids_guard.insert(project_id.clone()) {
                return;
            }
        }

        let id = project_id.clone();
        // 只保留"要用户去下"的两种类型。embedded 是打包在 jar 里的、incompatible 是明确
        // 冲突的，都不该出现在前置列表里，也不必为它们发请求。
        // 注意这里算的是"是否可选"，写进结果时**不要再取反**
        let optional = if item.dependency_type.eq_ignore_ascii_case("required") {
            false
        } else if item.dependency_type.eq_ignore_ascii_case("optional") {
            true
        } else {
            return;
        };

        let (res1, res2) = handle.block_on(async {
            let res1 = modrinth_api::get_project(&id).await;

            match res1 {
                Ok(data) => match &item.version_id {
                    // 依赖指名了版本：候选就只有这一个
                    Some(version) => (
                        Ok(data),
                        modrinth_api::get_version(project_id, version)
                            .await
                            .map(|item| vec![item]),
                    ),
                    None => {
                        let loader = if matches!(loader, LoaderType::Custom)
                            || matches!(loader, LoaderType::Normal)
                            || matches!(loader, LoaderType::OptiFine)
                        {
                            None
                        } else {
                            Some(loader.id())
                        };
                        // 整份候选都带上：前端要让用户在这些版本里挑，不只用第一个
                        let data1 =
                            modrinth_api::get_file_versions(project_id, Some(version), loader)
                                .await;

                        (Ok(data), data1)
                    }
                },
                Err(err) => (Err(err), Err(ErrorType::InvalidOperation)),
            }
        });

        let Ok(data) = res1 else { return };
        let Ok(data1) = res2 else { return };
        // 该项目在本版本 / 加载器下一个文件都没有：当作这条依赖解析失败，整条跳过
        let Some(first) = data1.first() else { return };

        let sub_deps = get_mod_dependencies_inner(&first.dependencies, version, loader, ids, handle);

        // 图标：项目接口给的是空串，按"没有"处理
        let icon = (!data.icon_url.is_empty()).then(|| data.icon_url.clone());

        list.lock().unwrap().push(ModrinthModDependenciesRes {
            name: data.title,
            icon,
            mod_id: data.id.clone(),
            opt: optional,
            list: data1,
        });

        // 添加未被标记的子依赖，避免同时持有 ids 和 list 锁
        for item5 in sub_deps {
            let mut ids_guard = ids.lock().unwrap();
            if ids_guard.contains(&item5.mod_id) {
                continue;
            }
            ids_guard.insert(item5.mod_id.clone());
            drop(ids_guard);
            list.lock().unwrap().push(item5);
        }
    });

    list.into_inner().unwrap()
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
    data: &mut ModrinthVersionObj,
    pack_gui: AddModPackGui,
    archive_gui: BaseArchiveGui,
    cancel: CancellationToken,
) -> CoreResult<()> {
    let obj = make_download_obj(data, mml_downloader::get_download_path());
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
    let mut worker = ModrinthPackWorker::new(BaseModPackWorker::new(
        zip,
        None,
        pack_gui.clone(),
        archive_gui,
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

    // **排除包根的 `modrinth.index.json`**：`extract` 会把"不在 `overrides/` 下"的条目写进
    // 实例 base 目录，覆盖掉 `check_upgrade` 要读的旧清单，差异算成空就不下载也不删除。
    worker
        .extract(Some(vec![names::MODRINTH_FILE.to_string()]))
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
        game.pid = Some(data.project_id.clone());
        game.fid = Some(data.id.clone());
        game.save();
    }

    // 实例名与目录跟着新版走，与安装时同一套规则；用户手动改过的名字也会被覆盖
    crate::modpack::rename_after_upgrade(game, worker.default_instance_name());

    Ok(())
}

impl InstanceSettingObj {
    /// 自动标记模组
    ///
    /// - `over`: 是否覆盖已经标记的模组
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；查询或保存失败返回对应错误
    pub async fn auto_mark(&self, over: bool) -> CoreResult<()> {
        let list = self.read_mod_fast(None).await;
        let mut online = self.read_online_info();

        let hashs: Vec<String> = online
            .values()
            .map(|data| data.sha1.clone().to_ascii_lowercase())
            .collect();

        let mut check = Vec::new();
        for item in list.iter() {
            let sha1 = item.hash.get_sha1().unwrap();
            if hashs.contains(&sha1) && !over {
                continue;
            }

            check.push(sha1);
        }

        if !check.is_empty() {
            let list = modrinth_api::get_version_from_sha1(check).await?;

            for (_, value) in list {
                online.remove(&value.project_id);
                let file = &value.files[0];
                online.insert(
                    value.project_id.clone(),
                    OnlineInfoObj {
                        path: names::GAME_MODS_DIR.to_string(),
                        name: value.name,
                        file: file.filename.clone(),
                        sha1: file.hashes.sha1.clone(),
                        url: file.url.clone(),
                        modid: value.project_id,
                        fileid: value.id,
                    },
                );
            }

            self.save_online_info(&online);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个只填了作者 ID 的命中项
    fn hit(author_id: &str) -> HitObj {
        HitObj {
            author_id: author_id.to_string(),
            ..Default::default()
        }
    }

    /// 作者 ID 去重、空 ID 跳过
    #[test]
    fn author_ids_dedups_and_skips_empty() {
        let hits = vec![hit("a"), hit("a"), hit(""), hit("b")];
        assert_eq!(author_ids(&hits), vec!["a".to_string(), "b".to_string()]);
        assert!(author_ids(&[]).is_empty());
    }
}
