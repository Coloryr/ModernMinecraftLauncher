//! HMCL 格式导出
//!
//! 生成 HMCL 的整合包元数据与 addons 列表。

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    serialize_tools,
};
use mml_names::{i18_items::error_type::CoreResult, names};

use super::ExportArg;
use super::archive::{add_game_files, pack_all_files};

use crate::{
    curseforge::pack_obj::{CurseForgePackObj, MinecraftObj, ModLoadersObj},
    launcher::instance_setting_obj::InstanceSettingObj,
    loader::LoaderType,
    other_launcher::hmcl_obj::{AddonsObj, HMCLObj},
};

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
pub(super) fn hmcl(game: &InstanceSettingObj, arg: ExportArg) -> CoreResult<()> {
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
                game.loader.id(),
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
        &arg.cancel,
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
pub(super) fn hmcl_addons(game: &InstanceSettingObj) -> Vec<AddonsObj> {
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
