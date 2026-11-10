//! CurseForge 整合包导出
//!
//! 生成 `manifest.json` 并按项目回查在线文件 id。

use std::path::PathBuf;

use mml_base::{
    archives::{ArchiveAddFile, ArchiveType, BaseArchive},
    serialize_tools,
};
use mml_names::{i18_items::error_type::CoreResult, names};

use mml_net::curseforge_api::{self};

use super::{ExportArg, check_cancel, get_author_string};

use crate::{
    curseforge::pack_obj::{CurseForgePackObj, FilesObj, MinecraftObj, ModLoadersObj},
    launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
};

/// 导出为 CurseForge 整合包格式
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
pub(super) async fn curseforge(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut obj = CurseForgePackObj {
        name: arg.name,
        author: arg.author,
        version: arg.version,
        manifest_type: "minecraftModpack".to_string(),
        manifest_version: 1,
        overrides: names::OVERRIDE_DIR.to_string(),
        minecraft: MinecraftObj {
            version: game.version.clone(),
            mod_loaders: Vec::new(),
        },
        ..Default::default()
    };

    if game.loader != LoaderType::Normal {
        obj.minecraft.mod_loaders.push(ModLoadersObj {
            id: format!(
                "{}-{}",
                game.loader.id(),
                game.loader_version.clone().unwrap_or_default()
            ),
            primary: true,
        });
    }

    for item in arg.mods {
        if let Some(info) = item.info {
            obj.files.push(FilesObj {
                file_id: info.fileid.parse::<u64>().unwrap_or_default(),
                project_id: info.modid.parse::<u64>().unwrap_or_default(),
                required: true,
            });
        }
    }

    // modlist.html 只是给人看的索引，项目名与链接要联网查：查不到就写个空列表，
    // 没有可索引的模组时干脆不查，别让网络问题把整个导出卡住
    let mut project_ids = obj
        .files
        .iter()
        .map(|item| item.project_id)
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    project_ids.sort_unstable();
    project_ids.dedup();
    let mut html = String::from("<ul>");
    if !project_ids.is_empty() {
        match curseforge_api::get_mods_info(project_ids).await {
            Ok(data) => {
                for item in data.data {
                    html.push_str(&format!(
                        "<li><a href=\"{}\"{} (by {})</a></li>",
                        item.links.website_url,
                        item.name,
                        get_author_string(&item.authors)
                    ));
                }
            }
            Err(err) => mml_log::error_type(err),
        }
    }
    html.push_str("</ul>");

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;

    if let Some(gui) = &arg.gui {
        gui.start(arg.select.len() + 2);
    }

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MANIFEST_FILE.to_string()), 0);
    }

    zip.add_data(names::MANIFEST_FILE, &serialize_tools::json_to_bytes(&obj)?)?;

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MOD_LIST_FILE.to_string()), 0);
    }

    zip.add_data(names::MOD_LIST_FILE, &html.as_bytes())?;

    let path = game.get_game_path();
    let mut pending = Vec::with_capacity(arg.select.len());

    for item in &arg.select {
        check_cancel(&arg.cancel)?;
        let rel = match item.strip_prefix(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let name = if rel_str.starts_with('/') {
            format!("{}{}", names::OVERRIDE_DIR, rel_str)
        } else {
            format!("{}/{}", names::OVERRIDE_DIR, rel_str)
        };

        pending.push(ArchiveAddFile {
            src: item.clone(),
            dest: PathBuf::from(name),
        });
    }

    // 一次写完：逐条 add_file 每写一条都要重写一遍中央目录
    zip.add_files_reporting(&pending, arg.gui.clone(), 2)?;

    Ok(())
}
