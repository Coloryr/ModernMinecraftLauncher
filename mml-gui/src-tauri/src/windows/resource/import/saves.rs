//! 存档目录的导入
//!
//! 存档按「一个目录一个存档」搬，落点由存档名决定。

use std::path::{PathBuf};

use mml_game::launcher::instance_setting_obj::InstanceSettingObj;

use crate::dtos::ResourceDropDto;

use super::common::{MODE_CHECK, MODE_SKIP, MODE_RENAME, unique_name, save_dir_name};


/// 存档导入：压缩包解压成一个存档目录，口径与内核 `import_save` 一致
///
/// - `game`: 实例设置本体
/// - `files`: 待导入的存档压缩包
/// - `mode`: 同名冲突的处理方式
///
/// # 返回值
///
/// 本次结果：`imported` / `skipped` / `conflicts` 三个名单
///
/// 目录名由压缩包内结构决定，`import_save` 不接受指定名字，所以"重命名"在这里的含义是
/// **把已存在的那个存档目录先挪成 ` (1)`**，再把新存档解压到原名 —— 结果是
/// "老的没丢、新的用原名"，与复制型的重命名是同一个意图。
pub(super) fn import_saves(
    game: &InstanceSettingObj,
    files: &[PathBuf],
    mode: &str,
) -> Result<ResourceDropDto, String> {
    let dir = game.get_saves_path();
    let mut result = ResourceDropDto::default();

    for file in files {
        let name = save_dir_name(file);
        if name.is_empty() {
            continue;
        }
        let target = dir.join(&name);
        if mode == MODE_CHECK {
            if target.exists() {
                result.conflicts.push(name);
            }
            continue;
        }
        if target.exists() {
            match mode {
                MODE_SKIP => {
                    result.skipped.push(name);
                    continue;
                }
                MODE_RENAME => {
                    let unique = unique_name(&dir, &name);
                    std::fs::rename(&target, &unique.path).map_err(|err| err.to_string())?;
                }
                _ => {}
            }
        }
        game.import_save(file).map_err(|err| err.to_string())?;
        result.imported.push(name);
    }
    Ok(result)
}
