//! 导入游戏文件夹
//!
//! 直接把一个已解开的游戏目录复制成实例，MMC 与官启版本都能认出。

use super::path_not_exists;

use std::path::{Path, PathBuf};

use mml_base::serialize_tools;
use mml_names::{
    i18_items::error_type::{ArgEmptyData, CoreResult, ErrorType},
    names,
};
use mml_sys::path_helper;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    GameInstance, game_options,
    gui_hook::{AddInstanceGui, ProgressGui},
    launcher::instance_setting_obj::InstanceSettingObj,
    launcher_path::version_path,
    other_launcher::{self, mmc_obj::MMCObj, official_obj::OfficialObj},
};

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
