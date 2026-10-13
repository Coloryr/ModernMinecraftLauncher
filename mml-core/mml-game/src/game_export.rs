//! 实例导出为整合包

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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
    other_launcher::{
        hmcl_obj::{AddonsObj, HMCLObj},
        mmc_obj::{ComponentsObj, MMCObj},
    },
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
    /// MMC 格式，整包实例，供 MultiMC / Prism 导入
    MMC,
    /// HMCL 格式，整包实例，供 HMCL 导入
    HMCL,
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
            ExportPackType::MMC => mmc(self, arg),
            ExportPackType::HMCL => hmcl(self, arg),
            ExportPackType::Zip => zip_pack(self, arg),
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
fn zip_pack(game: &InstanceSettingObj, data: ExportArg) -> CoreResult<()> {
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

    // 没有 projectID、只有下载地址的条目：本地来源的模组、或用户自己填了地址的文件。
    // 它们照样能写进 index 的 files（downloads 给地址，project 留空），
    // 与 overrides 的区别是"由谁提供这份文件"
    for item in arg.files {
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

/// 把游戏目录下的一批文件按 `<prefix>/<相对游戏目录的路径>` 加进压缩包
///
/// 两种整包格式的差别只在包内前缀：MMC 是 `<实例名>/.minecraft`，HMCL 是 overrides。
///
/// - `zip`: 目标压缩包
/// - `base`: 游戏目录，即 `.minecraft`
/// - `prefix`: 包内前缀，末尾不带 `/`
/// - `files`: 待打包的文件
/// - `gui`: 进度回调，`None` 时不上报
/// - `offset`: 进度起点，回调里的序号加在它上面
///
/// # 返回值
///
/// 打包失败返回对应错误
fn add_game_files(
    zip: &mut BaseArchive,
    base: &Path,
    prefix: &str,
    files: &[PathBuf],
    gui: &BaseArchiveGui,
    offset: usize,
) -> CoreResult<()> {
    for (index, item) in files.iter().enumerate() {
        // 不在游戏目录下的文件直接跳过：包内路径只能相对它算
        let Ok(rel) = item.strip_prefix(base) else {
            continue;
        };
        let name = format!("{}/{}", prefix, rel.to_string_lossy().replace('\\', "/"));

        if let Some(gui) = gui {
            gui.update(Some(name.clone()), offset + index);
        }

        zip.add_file(&name, item)?;
    }

    Ok(())
}

/// 整包格式要打包的文件：没有下载清单，模组与文件一律进包
///
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 待打包的文件列表
fn pack_all_files(arg: &ExportArg) -> Vec<PathBuf> {
    let mut files = arg.select.clone();
    files.extend(arg.mods.iter().map(|item| item.file.clone()));
    files.extend(arg.files.iter().map(|item| item.file.clone()));
    files
}

/// 导出为 MMC 实例包：整包实例，供 MultiMC / Prism 导入
///
/// 包内是"一个实例目录"：`mmc-pack.json` 写组件、`instance.cfg` 写实例设置，
/// 游戏文件放在 `.minecraft/` 下。组件 uid 与导入侧 `other_launcher` 的
/// `to_instance` 判据一一对应，导出的包能被本启动器原样导回来。
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误。自定义加载器在 MMC 里没有对应组件，
/// 这时只导游戏版本、不写加载器组件
fn mmc(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let mut components = vec![ComponentsObj {
        cached_version: game.version.clone(),
        uid: "net.minecraft".to_string(),
        version: game.version.clone(),
    }];

    let uid = match game.loader {
        LoaderType::Forge => Some("net.minecraftforge"),
        LoaderType::NeoForge => Some("net.neoforged"),
        LoaderType::Fabric => Some("net.fabricmc.fabric-loader"),
        LoaderType::Quilt => Some("org.quiltmc.quilt-loader"),
        _ => None,
    };
    if let Some(uid) = uid {
        let version = game.loader_version.clone().unwrap_or_default();
        components.push(ComponentsObj {
            cached_version: version.clone(),
            uid: uid.to_string(),
            version,
        });
    }

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;
    // 顶层套一层实例目录：MMC / Prism 从 zip 导入时按它建实例
    let inst_dir = format!("{}/", game.name);
    let game_dir = format!("{inst_dir}{}", names::GAME_DIR);
    let files = pack_all_files(&arg);

    if let Some(gui) = &arg.gui {
        gui.start(files.len() + 2);
        gui.update(Some(names::MMCJSON_FILE.to_string()), 0);
    }

    let data = serialize_tools::json_to_bytes(&MMCObj { components })?;
    zip.add_data(&format!("{inst_dir}{}", names::MMCJSON_FILE), &data)?;

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MMCCFG_FILE.to_string()), 1);
    }

    zip.add_data(
        &format!("{inst_dir}{}", names::MMCCFG_FILE),
        instance_cfg(game).as_bytes(),
    )?;

    add_game_files(&mut zip, &game.get_game_path(), &game_dir, &files, &arg.gui, 2)
}

/// MMC 的 `instance.cfg` 内容：只写导入侧会读的几个键
///
/// - `game`: 目标实例
///
/// # 返回值
///
/// INI 文本，行尾是 `\n`
fn instance_cfg(game: &InstanceSettingObj) -> String {
    // InstanceType 必须给，缺了 MMC 会按老版本实例处理
    let mut text = String::from("InstanceType=OneSix\n");
    text.push_str(&format!("iconKey=default\nname={}\n", game.name));

    if let Some(jvm) = &game.jvm_arg {
        if let Some(max) = jvm.max_memory {
            text.push_str(&format!("MaxMemAlloc={max}\n"));
        }
        if let Some(min) = jvm.min_memory {
            text.push_str(&format!("MinMemAlloc={min}\n"));
        }
    }

    text
}

/// 导出为 HMCL 整合包：整包实例，供 HMCL 导入
///
/// 包内是根目录的 `mcbbs.packmeta`（HMCL 的组件表）加 `manifest.json`，
/// 游戏文件放在 `overrides/` 下。`manifest.json` 是 HMCL 自己不会写、但本启动器
/// 导入 HMCL 包时**必读**的一份，用它的 overrides 字段定位游戏文件，所以导出必须带上。
///
/// - `game`: 目标实例
/// - `arg`: 导出参数
///
/// # 返回值
///
/// 成功返回 `Ok(())`；打包失败返回对应错误。自定义加载器在 HMCL 里没有对应组件，
/// 这时只导游戏版本
fn hmcl(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
    let obj = HMCLObj {
        name: game.name.clone(),
        addons: hmcl_addons(game),
        launch_info: None,
    };

    let mut manifest = CurseForgePackObj {
        name: arg.name.clone(),
        author: arg.author.clone(),
        version: arg.version.clone(),
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
        manifest.minecraft.mod_loaders.push(ModLoadersObj {
            id: format!(
                "{}-{}",
                game.loader.to_string(),
                game.loader_version.clone().unwrap_or_default()
            ),
            primary: true,
        });
    }

    let mut zip = BaseArchive::create_empty(ArchiveType::Zip, &arg.file)?;
    let files = pack_all_files(&arg);

    if let Some(gui) = &arg.gui {
        gui.start(files.len() + 2);
        gui.update(Some(names::HMCLFILE.to_string()), 0);
    }

    zip.add_data(names::HMCLFILE, &serialize_tools::json_to_bytes(&obj)?)?;

    if let Some(gui) = &arg.gui {
        gui.update(Some(names::MANIFEST_FILE.to_string()), 1);
    }

    zip.add_data(
        names::MANIFEST_FILE,
        &serialize_tools::json_to_bytes(&manifest)?,
    )?;

    add_game_files(
        &mut zip,
        &game.get_game_path(),
        names::OVERRIDE_DIR,
        &files,
        &arg.gui,
        2,
    )
}

/// HMCL 的组件表：game 加加载器
///
/// 组件 id 复用 names 里那几个键，取值与导入侧 `to_instance` 的判据一致。
///
/// - `game`: 目标实例
///
/// # 返回值
///
/// 组件列表，game 在前
fn hmcl_addons(game: &InstanceSettingObj) -> Vec<AddonsObj> {
    let mut list = vec![AddonsObj {
        id: names::GAME_KEY.to_string(),
        version: game.version.clone(),
    }];

    let id = match game.loader {
        LoaderType::Forge => Some(names::FORGE_KEY),
        LoaderType::NeoForge => Some(names::NEOFORGE_KEY),
        LoaderType::Fabric => Some(names::FABRIC_KEY),
        LoaderType::Quilt => Some(names::QUILT_KEY),
        _ => None,
    };
    if let Some(id) = id {
        list.push(AddonsObj {
            id: id.to_string(),
            version: game.loader_version.clone().unwrap_or_default(),
        });
    }

    list
}
