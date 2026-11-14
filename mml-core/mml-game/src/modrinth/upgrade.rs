//! Modrinth 整合包升级

use super::*;

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

    let zip = BaseArchive::open_readonly(file)?;
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

    worker.download().await?;

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
