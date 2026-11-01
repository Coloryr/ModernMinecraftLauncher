//! 在下载或清理前校验服务器包的新旧清单。

use super::serverpack_obj::ServerPackObj;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::path_helper::safe_join;
use std::path::{Path, PathBuf};

/// 检查文件或待删除目录，禁止将根目录本身作为目标。
///
/// - `root`: 可信根目录
/// - `name`: 清单中的相对路径
///
/// # 返回值
///
/// 根目录内的目标路径；越界、经过已有链接或指向根目录时返回错误
pub(super) fn file_path(root: &Path, name: &str) -> CoreResult<PathBuf> {
    let path = safe_join(root, name)?;
    if path == root {
        return Err(ErrorType::UnsafePath {
            root: root.to_path_buf(),
            path,
        });
    }
    Ok(path)
}

/// 先检查整份清单，避免处理到非法条目前已经删除旧文件。
///
/// - `pack`: 待检查的新清单或旧清单
/// - `game`: 游戏文件根目录
/// - `temp`: 下载归档的临时根目录
///
/// # 返回值
///
/// 全部路径通过检查时返回 `Ok(())`，否则返回首个路径错误
pub(super) fn validate_pack(pack: &ServerPackObj, game: &Path, temp: &Path) -> CoreResult<()> {
    for item in &pack.online_list {
        file_path(game, &item.file)?;
    }
    for item in &pack.archive_list {
        file_path(temp, &item.file)?;
        if item.delete_old {
            file_path(game, &item.dir)?;
        } else {
            safe_join(game, &item.dir)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/serverpack_paths_tests.rs"]
mod tests;
