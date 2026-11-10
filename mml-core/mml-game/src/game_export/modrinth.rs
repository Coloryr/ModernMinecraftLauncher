//! Modrinth 整合包导出
//!
//! 生成 `modrinth.index.json`，逐文件算 sha1 与下载地址。

use std::{collections::HashMap, path::PathBuf};

use mml_base::{
    archives::{ArchiveAddFile, ArchiveType, BaseArchive},
    serialize_tools,
};
use mml_names::{i18_items::error_type::CoreResult, names};

use mml_net::modrinth_api::version_obj::HasheObj;

use super::{ExportArg, check_cancel};

use crate::{
    launcher::{
        self, instance_setting_obj::InstanceSettingObj, project_save_obj::MmlProjectSaveObj,
    },
    loader::LoaderType,
    modrinth::pack_obj::{ModrinthPackFileObj, ModrinthPackObj},
};

/// 导出为 Modrinth 整合包格式
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
pub(super) async fn modrinth(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut obj = ModrinthPackObj {
        format_version: 1,
        game: String::from(names::MINECRAFT_KEY),
        version_id: arg.version,
        name: arg.name,
        summary: arg.summary,
        files: Vec::new(),
        dependencies: HashMap::new(),
    };

    obj.dependencies
        .insert(String::from(names::MINECRAFT_KEY), game.version.clone());
    match game.loader {
        LoaderType::Forge => {
            obj.dependencies.insert(
                String::from(names::FORGE_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        LoaderType::Fabric => {
            obj.dependencies.insert(
                String::from(names::FABRIC_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        LoaderType::Quilt => {
            obj.dependencies.insert(
                String::from(names::QUILT_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        LoaderType::NeoForge => {
            obj.dependencies.insert(
                String::from(names::NEOFORGE_KEY),
                game.loader_version.clone().unwrap_or_default(),
            );
        }
        _ => {}
    }

    let path = game.get_game_path();

    for item in arg.mods {
        if let Some(info) = item.info {
            let rel = match item.file.strip_prefix(&path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            obj.files.push(ModrinthPackFileObj {
                path: rel.to_string_lossy().replace('\\', "/"),
                hashes: HasheObj {
                    sha1: item.hash.get_sha1().unwrap_or_default(),
                    sha512: item.hash.get_sha512().unwrap_or_default(),
                },
                downloads: vec![item.url],
                file_size: item.size as u64,
                project: Some(MmlProjectSaveObj {
                    source_type: launcher::get_source_type(&info.modid, &info.fileid),
                    pid: info.modid.clone(),
                    fid: info.fileid.clone(),
                }),
            });
        }
    }

    // 没有 projectID、只有下载地址的条目：本地来源的模组、或用户自己填了地址的文件。
    // 它们照样能写进 index 的 files（downloads 给地址，project 留空），
    // 与 overrides 的区别是"由谁提供这份文件"
    for item in arg.files {
        let rel = match item.file.strip_prefix(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        obj.files.push(ModrinthPackFileObj {
            path: rel.to_string_lossy().replace('\\', "/"),
            hashes: HasheObj {
                sha1: item.hash.get_sha1().unwrap_or_default(),
                sha512: item.hash.get_sha512().unwrap_or_default(),
            },
            downloads: vec![item.url],
            file_size: item.size as u64,
            project: None,
        });
    }

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;

    if let Some(gui) = &arg.gui {
        gui.start(arg.select.len() + 1);
    }

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MODRINTH_FILE.to_string()), 0);
    }

    zip.add_data(names::MODRINTH_FILE, &serialize_tools::json_to_bytes(&obj)?)?;

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
    zip.add_files_reporting(&pending, arg.gui.clone(), 1)?;

    Ok(())
}
