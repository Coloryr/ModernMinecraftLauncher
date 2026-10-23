//! 在下载或清理前校验服务器包的新旧清单。

use super::serverpack_obj::ServerPackObj;
use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::path_helper::safe_join;
use std::path::{Path, PathBuf};

/// 检查文件或待删除目录，禁止将根目录本身作为目标。
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
mod tests {
    use super::super::serverpack_obj::{ServerArchiveItemObj, ServerItemObj};
    use super::*;

    #[test]
    fn rejects_manifest_escape_and_root_deletion() {
        let root = mml_testutil::temp_path(format!("serverpack_paths_{}", uuid::Uuid::new_v4()));
        let game = root.join("game");
        let temp = root.join("temp");
        for bad in [
            "../outside",
            "/outside",
            "C:/outside",
            "..\\outside",
            "",
            ".",
        ] {
            let mut pack = ServerPackObj::default();
            pack.online_list.push(ServerItemObj {
                file: bad.into(),
                ..Default::default()
            });
            assert!(validate_pack(&pack, &game, &temp).is_err(), "{bad}");
            pack.online_list.clear();
            pack.archive_list.push(ServerArchiveItemObj {
                file: "pack.zip".into(),
                dir: bad.into(),
                delete_old: true,
                ..Default::default()
            });
            assert!(validate_pack(&pack, &game, &temp).is_err(), "{bad}");
            pack.archive_list[0].file = bad.into();
            pack.archive_list[0].dir = "config".into();
            assert!(validate_pack(&pack, &game, &temp).is_err(), "{bad}");
        }
        let mut pack = ServerPackObj::default();
        pack.online_list.push(ServerItemObj {
            file: "mods/example.jar".into(),
            ..Default::default()
        });
        pack.archive_list.push(ServerArchiveItemObj {
            file: "config.zip".into(),
            dir: "".into(),
            delete_old: false,
            ..Default::default()
        });
        validate_pack(&pack, &game, &temp).unwrap();
    }
}
