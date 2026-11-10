//! 整合包安装
//!
//! 解出 `modrinth.index.json` 或 CurseForge 清单，按文件列表下载后落进实例目录。

use super::path_not_exists;

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use mml_base::{
    archives::{BaseArchive, BaseArchiveGui},
    file_item::FileItemObj,
};
use mml_downloader::TaskOutcome;
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    gui_hook::{AddInstanceGui, AddModPackGui, AddModPackState},
    launcher::ModPackType,
    modpack::{
        BaseModPackWorker, ModPackWorker, curseforge_worker::CurseForgeWorker,
        modrinth_worker::ModrinthPackWorker,
    },
};

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
/// 下载进度的分母：按千分比写入任务的子进度，避开 u32 字节数上限
const DOWNLOAD_PROGRESS_TOTAL: usize = 1000;

/// 下载一批文件，并把字节进度写进整合包安装任务的子进度
///
/// - `items`: 待下载文件
/// - `pack_gui`: 安装任务进度回调；`None` 时只等结果
/// - `cancel`: 取消令牌
///
/// # 返回值
///
/// 下载结果
pub(crate) async fn download_with_progress(
    items: Vec<FileItemObj>,
    pack_gui: AddModPackGui,
    cancel: CancellationToken,
) -> TaskOutcome {
    let Some(handle) = mml_downloader::start_download_task_handle(items, cancel) else {
        return TaskOutcome::Cancelled;
    };
    let handle = Arc::new(handle);
    if let Some(gui) = &pack_gui {
        gui.set_sub_text(None);
        gui.set_sub_now(0, Some(DOWNLOAD_PROGRESS_TOTAL));
    }

    // 下载句柄的 wait 只等结果；每 200ms 从快照读一次字节进度，交给任务子进度条
    let stop = Arc::new(AtomicBool::new(false));
    let reporter = {
        let handle = handle.clone();
        let gui = pack_gui.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                report_download(&gui, &handle.status());
                if handle.is_finished() {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        })
    };

    let outcome = handle.wait().await;
    stop.store(true, Ordering::Release);
    let _ = reporter.join();
    report_download(&pack_gui, &handle.status());
    outcome
}

/// 把下载任务快照折算成千分比子进度
///
/// - `gui`: 安装任务进度回调
/// - `status`: 下载任务快照
fn report_download(gui: &AddModPackGui, status: &mml_downloader::TaskSnapshot) {
    let Some(gui) = gui else {
        return;
    };
    let done = if status.all_bytes > 0 {
        status.now_bytes.min(status.all_bytes) * DOWNLOAD_PROGRESS_TOTAL as u64 / status.all_bytes
    } else if status.total > 0 {
        status.completed as u64 * DOWNLOAD_PROGRESS_TOTAL as u64 / status.total as u64
    } else {
        0
    };
    gui.set_sub_now(done as usize, Some(DOWNLOAD_PROGRESS_TOTAL));
}

pub(super) async fn modpack<P: AsRef<Path>>(
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
            BaseArchive::open_readonly(file)?,
            instance_gui,
            pack_gui.clone(),
            archive_gui,
            cancel.clone(),
            icon,
        )))
    } else {
        Box::new(ModrinthPackWorker::new(BaseModPackWorker::new(
            BaseArchive::open_readonly(file)?,
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
