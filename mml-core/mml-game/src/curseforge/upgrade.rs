//! CurseForge 整合包升级

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

    let zip = BaseArchive::open_readonly(file)?;
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
