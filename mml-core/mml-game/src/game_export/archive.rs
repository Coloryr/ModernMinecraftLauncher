//! 压缩包导出
//!
//! MML 自有格式与普通 zip：按选择复制游戏文件后打包。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use mml_base::{
    archives::{ArchiveAddFile, BaseArchive, BaseArchiveGui},
    serialize_tools,
};
use mml_names::{i18_items::error_type::CoreResult, names};

use super::{ExportArg, check_cancel};

use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 导出为 ColorMC 整合包格式
///
/// - `game`: 目标实例
/// - `data`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
pub(super) fn colormc(game: &InstanceSettingObj, data: ExportArg) -> CoreResult<()> {
    let mut list = data.unselect;
    list.push(game.get_online_info_file());

    let list = list
        .iter()
        .map(|item| item.to_string_lossy().to_string())
        .collect();

    let mut list1 = HashMap::new();
    for item in data.mods.iter() {
        if let Some(info) = &item.info {
            list1.insert(info.modid.clone(), info.clone());
        }
    }

    let mut archive = BaseArchive::compress(
        data.archive,
        &data.file,
        &game.get_base_path(),
        None,
        &Some(list),
        data.gui.clone(),
    )?;
    archive.add_data(
        names::MOD_INFO_FILE,
        &serialize_tools::json_to_bytes(&list1)?,
    )?;

    Ok(())
}

/// 导出为纯压缩包：整个实例目录原样打包，不写任何整合包清单
///
/// 包内就是实例目录本身：游戏文件在 `.minecraft/` 下，`game.json` 等实例元数据在根。
/// 因为没有清单，这份包不能被任何启动器当整合包导入，只能手工解包使用。
///
/// - `game`: 目标实例
/// - `data`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
pub(super) fn zip_pack(game: &InstanceSettingObj, data: ExportArg) -> CoreResult<()> {
    let list = data
        .unselect
        .iter()
        .map(|item| item.to_string_lossy().to_string())
        .collect();

    BaseArchive::compress(
        data.archive,
        &data.file,
        &game.get_base_path(),
        None,
        &Some(list),
        data.gui.clone(),
    )?;

    Ok(())
}

/// 把根目录下的一批文件按 `<prefix>/<相对根目录的路径>` 加进压缩包
///
/// 两种整包格式的差别只在根与包内前缀：MMC 以实例目录为根、前缀是实例名；
/// HMCL 以游戏目录为根、前缀是 overrides。
///
/// - `zip`: 目标压缩包
/// - `base`: 选文件时的根目录
/// - `prefix`: 包内前缀，末尾不带 `/`
/// - `files`: 待打包的文件
/// - `gui`: 进度回调，`None` 时不上报
/// - `offset`: 进度起点，回调里的序号加在它上面
/// - `cancel`: 取消令牌，见 [`check_cancel`]
///
/// # 返回值
///
/// 打包失败或被取消时返回对应错误
pub(super) fn add_game_files(
    zip: &mut BaseArchive,
    base: &Path,
    prefix: &str,
    files: &[PathBuf],
    gui: &BaseArchiveGui,
    offset: usize,
    cancel: &Option<Arc<AtomicBool>>,
) -> CoreResult<()> {
    let mut pending = Vec::with_capacity(files.len());
    for item in files {
        check_cancel(cancel)?;
        // 不在游戏目录下的文件直接跳过：包内路径只能相对它算
        let Ok(rel) = item.strip_prefix(base) else {
            continue;
        };
        let name = format!("{}/{}", prefix, rel.to_string_lossy().replace('\\', "/"));
        pending.push(ArchiveAddFile {
            src: item.clone(),
            dest: PathBuf::from(name),
        });
    }

    // 一次写完：逐条 add_file 每写一条都要重写一遍中央目录
    zip.add_files_reporting(&pending, gui.clone(), offset)
}

/// 整包格式要打包的文件：没有下载清单，模组与文件一律进包
///
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 待打包的文件列表
pub(super) fn pack_all_files(arg: &ExportArg) -> Vec<PathBuf> {
    let mut files = arg.select.clone();
    files.extend(arg.mods.iter().map(|item| item.file.clone()));
    files.extend(arg.files.iter().map(|item| item.file.clone()));
    files
}
