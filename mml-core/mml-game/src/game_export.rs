//! 实例导出为整合包

use std::{collections::HashMap, path::PathBuf};

use mml_base::{
    archives::{ArchiveType, BaseArchive, BaseArchiveGui},
    file_item::FileHash,
    serialize_tools,
};
use mml_names::{i18_items::error_type::CoreResult, names};

use mml_net::{
    curseforge_api::{self, list_obj::AuthorsObj},
    modrinth_api::version_obj::HasheObj,
};

use crate::{
    curseforge::pack_obj::{CurseForgePackObj, FilesObj, MinecraftObj, ModLoadersObj},
    launcher::{
        self, file_online_info_obj::OnlineInfoObj, get_source_type,
        instance_setting_obj::InstanceSettingObj, project_save_obj::MmlProjectSaveObj,
    },
    loader::LoaderType,
    modrinth::pack_obj::{ModrinthPackFileObj, ModrinthPackObj},
};

/// 导出压缩包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPackType {
    /// MML 格式
    MML,
    /// CurseForge 格式
    CurseForge,
    /// Modrinth 格式
    Modrinth,
    /// 纯压缩包，只打包文件不带整合包清单
    Zip,
}

/// 导出的在线文件
pub struct OnlineFileExport {
    /// 文件路径
    pub file: PathBuf,
    /// 文件大小，单位字节
    pub size: usize,
    /// 导出方式
    pub pack: ExportPackType,
    /// 下载地址
    pub url: String,
    /// 文件校验值
    pub hash: FileHash,
    /// 文件在线信息，非线上来源时为 `None`
    pub info: Option<OnlineInfoObj>,
}

/// 导出需要的参数
pub struct ExportArg {
    /// 导出保存的位置
    pub file: PathBuf,
    /// 打包类型
    pub pack: ExportPackType,
    /// 压缩类型
    pub archive: ArchiveType,
    /// 在线模组信息
    pub mods: Vec<OnlineFileExport>,
    /// 在线文件信息
    pub files: Vec<OnlineFileExport>,
    /// 不打包的文件
    pub unselect: Vec<PathBuf>,
    /// 一起打包的文件
    pub select: Vec<PathBuf>,
    /// 整合包名字
    pub name: String,
    /// 整合包作者
    pub author: String,
    /// 整合包版本号
    pub version: String,
    /// 整合包简介
    pub summary: String,
    /// 压缩进度条
    pub gui: BaseArchiveGui,
}

impl InstanceSettingObj {
    /// 导出整合包
    ///
    /// - `arg`: 导出参数
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；打包失败返回对应错误
    pub async fn export(&self, arg: ExportArg) -> CoreResult<()> {
        match arg.pack {
            ExportPackType::MML => colormc(self, arg),
            ExportPackType::CurseForge => curseforge(self, arg).await,
            ExportPackType::Modrinth => modrinth(self, arg).await,
            ExportPackType::Zip => todo!(),
        }
    }
}

/// 将作者列表拼接为字符串
///
/// - `author`: 作者列表
///
/// # 返回值
///
/// 返回拼接后的作者名；空列表返回空串
fn get_author_string(author: &Vec<AuthorsObj>) -> String {
    if author.is_empty() {
        String::new()
    } else {
        let mut data = String::new();

        for item in author.iter() {
            data.push_str(&item.name);
        }

        data
    }
}

/// 导出为 ColorMC 整合包格式
///
/// - `game`: 目标实例
/// - `data`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
fn colormc(game: &InstanceSettingObj, data: ExportArg) -> CoreResult<()> {
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

/// 导出为 CurseForge 整合包格式
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
async fn curseforge(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
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
                game.loader.to_string(),
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

    let data =
        curseforge_api::get_mods_info(obj.files.iter().map(|item| item.project_id).collect())
            .await?;
    let mut html = String::from("<ul>");
    for item in data.data {
        html.push_str(&format!(
            "<li><a href=\"{}\"{} (by {})</a></li>",
            item.links.website_url,
            item.name,
            get_author_string(&item.authors)
        ));
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
    let mut index = 0;

    for item in arg.select {
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

        if let Some(gui) = &arg.gui {
            gui.update(Some(name.clone()), 2 + index);
        }

        zip.add_file(&name, &item)?;
        index += 1;
    }

    Ok(())
}

/// 导出为 Modrinth 整合包格式
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误
async fn modrinth(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut obj = ModrinthPackObj {
        format_version: 1,
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
                path: rel.to_string_lossy().to_string(),
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

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;

    if let Some(gui) = &arg.gui {
        gui.start(arg.select.len() + 1);
    }

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MODRINTH_FILE.to_string()), 0);
    }

    zip.add_data(names::MODRINTH_FILE, &serialize_tools::json_to_bytes(&obj)?)?;

    let mut index = 0;

    for item in arg.select {
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

        if let Some(gui) = &arg.gui {
            gui.update(Some(name.clone()), 1 + index);
        }

        zip.add_file(&name, &item)?;
        index += 1;
    }

    Ok(())
}
