//! CurseForge 整合包文件信息获取与条目路径解析

use super::*;

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
