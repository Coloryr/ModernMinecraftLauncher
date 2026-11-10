//! 服务器整合包管理
//!
//! 清单的保存与版本判定在 `state`，升级同步在 `upgrade`，文件比对与下载在 `files`。

mod files;
mod state;
mod upgrade;

pub use self::files::{ServerPackPhase, ServerPackProgress};

pub mod generate;
mod paths;
pub mod plan;
pub mod serverpack_obj;

/// `gui_setting.json` 里服务器包状态的键
pub(super) const GUI_SERVER_PACK_KEY: &str = "ServerPack";
/// 服务器包状态里跳过记录的键
pub(super) const GUI_SKIPPED_KEY: &str = "Skipped";

/// 玩家上次跳过更新时本地留下的旧文件
///
/// 存在实例的 `gui_setting.json` 的 `ServerPack.Skipped` 里，不写进 `server.json`：
/// 这是本机玩家状态，不是发布内容。
#[derive(serde::Serialize, serde::Deserialize, Debug, Default, Clone)]
#[serde(default, rename_all = "PascalCase")]
pub struct ServerPackSkippedObj {
    /// 新清单里的相对路径，如 `mods/foo.jar`
    pub file: String,
    /// 本地留下来的旧文件，相对游戏目录；当时没有旧文件时为空
    pub kept: String,
}

#[cfg(test)]
mod tests {
    use mml_base::archives::{ArchiveType, BaseArchive};

    use super::files::{archive_has_no_files, existing_file, under};

    /// 禁用文件两个后缀都认，优先新后缀；模组以外的文件改名停用同样算
    #[test]
    fn existing_file_accepts_legacy_suffix() {
        let dir = mml_testutil::temp_path(format!("mml-existing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        let target = dir.join("mods/a.jar");

        std::fs::write(dir.join("mods/a.jar.disable"), b"old").unwrap();
        let found = existing_file(&target).expect("旧后缀也要认");
        assert!(found.1);
        assert!(found.0.ends_with("a.jar.disable"));

        std::fs::write(dir.join("mods/a.jar.disabled"), b"new").unwrap();
        let found = existing_file(&target).expect("新后缀要认");
        assert!(found.1);
        assert!(found.0.ends_with("a.jar.disabled"), "两个都在时用新后缀");

        std::fs::write(dir.join("config.toml.disable"), b"x").unwrap();
        assert!(
            existing_file(&dir.join("config.toml")).is_some(),
            "不限于 .jar：任何被改名停用的文件都要认"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn under_only_matches_descendants() {
        assert!(under("config/setting", "config"));
        assert!(under("config/setting/deep", "config/setting"));
        assert!(!under("config", "config"));
        assert!(!under("config2", "config"));
        assert!(!under("config/setting", "config2"));
    }

    /// 空包按删除目录处理；带文件的包不能误判
    #[test]
    fn empty_archive_counts_as_directory_delete() {
        let dir = mml_testutil::temp_path(format!("mml-archive-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let empty = dir.join("empty.zip");
        drop(BaseArchive::create_empty(ArchiveType::Zip, &empty).unwrap());
        assert!(archive_has_no_files(&empty, ArchiveType::Zip));

        let full = dir.join("full.zip");
        let mut archive = BaseArchive::create_empty(ArchiveType::Zip, &full).unwrap();
        archive.add_data("a.txt", b"x").unwrap();
        drop(archive);
        assert!(!archive_has_no_files(&full, ArchiveType::Zip));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
