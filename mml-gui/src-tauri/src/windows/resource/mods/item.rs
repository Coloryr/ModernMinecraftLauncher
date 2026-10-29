//! 单个模组的读取与描述
//!
//! 模组对象、备注与 SHA1 在这里，兄弟模块与依赖树共用。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mml_base::file_item::FileHash;
use mml_game::GameInstance;
use mml_game::game_mods::{LoadSideType, ModObj};
use mml_game::loader::LoaderType;
use mml_names::names;
use mml_sys::path_helper;

use crate::dtos::{ModItemDto, ModRenameDto};

use crate::err_keys;

use crate::windows::resource::{block_on_instance, data_url};

// ==================== 模组 ====================
use super::list::mod_dependencies;

/// `mod_item` 需要的几份外部数据，打包传，省得一路加参数
pub(super) struct ModCtx<'a> {
    /// `gui_setting.json` 的 `Mod.ModName`，文件名 → 说明
    pub(super) notes: &'a HashMap<String, Option<String>>,
    /// `gui_setting.json` 的 `Mod.ModSource`，文件名 → 手动指定的下载源
    pub(super) sources: &'a HashMap<String, String>,
    /// 实例的在线信息表，按 **SHA1** 索引
    pub(super) online: &'a HashMap<String, mml_game::launcher::file_online_info_obj::OnlineInfoObj>,
    /// 实例的游戏根目录 `.minecraft`，用来算相对路径
    pub(super) game_path: &'a Path,
    /// 实例的加载器：前置声明只收与它一致的那份元数据，与依赖检测同一口径
    pub(super) loader: LoaderType,
}

/// 一个模组条目转成 DTO，条目本身可能带内置模组
///
/// - `item`: 内核扫描出来的模组条目
/// - `ctx`: 备注 / 在线信息 / 游戏根目录，见 [`ModCtx`]
/// - `nested`: 是不是内置 jar，即 `jar_in_jar` 里的那一层；用来区分"库"与"读不出元数据的顶层包"
///
/// # 返回值
///
/// 文件名、模组 id、名字都空时返回 `None`，只可能是"不在 mods 目录下的东西"。
/// 顶层条目取文件名；内置模组没有独立文件路径，用**归档内的条目名**当 `file`，
/// 由后端 `read_jar_in_jar` 填，只用于展示，列表里缩进一层，不提供启用 / 删除
pub(super) fn mod_item(item: &ModObj, ctx: &ModCtx, nested: bool) -> Option<ModItemDto> {
    let file = item
        .file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let info = item.info.first();
    let mod_id = info.map(|i| i.mod_id.clone()).unwrap_or_default();
    let name = info.map(|i| i.name.clone()).unwrap_or_default();
    // 文件名、modid、显示名三样都拿不到才跳过：只可能是"不在 mods 目录下的东西"
    if file.is_empty() && mod_id.is_empty() && name.is_empty() {
        return None;
    }
    // **库**：内置 jar 里没有元数据的那些，asm / mixinextras 之类依赖，不是模组。
    // 只对内置 jar 判定 —— `mods/` 下没有元数据的是"坏包"，另有 `fail` 标记
    let library = nested && item.info.is_empty();
    let icon = item.info.iter().find_map(|i| i.icon.as_ref());

    // 支持的加载器：汇总**所有**元数据条目去重，一个 jar 可能同时带 fabric 与 forge 元数据，
    // 按加载器声明顺序排；`normal` 只表示"没说是哪个加载器"，要滤掉，否则像"支持原版"
    let loaders: Vec<String> = {
        let mut list: Vec<(u8, String)> = Vec::new();
        for entry in &item.info {
            let name = entry.loaders.id();
            if name == "normal" {
                continue;
            }
            if list.iter().any(|(_, existing)| existing == name) {
                continue;
            }
            list.push((entry.loaders as u8, name.to_string()));
        }
        list.sort_by_key(|(order, _)| *order);
        list.into_iter().map(|(_, name)| name).collect()
    };

    // 在线信息按 SHA1 关联：手动放进 mods 的模组不在表里，那三列就空着
    let sha1 = sha1_of(&item.hash);
    let online = ctx.online.get(&sha1);
    let project_id = online.map(|i| i.modid.clone()).unwrap_or_default();
    let file_id = online.map(|i| i.fileid.clone()).unwrap_or_default();
    // 下载源：手动指定的优先（`gui_setting.json` 的 `Mod.ModSource`），没指定就按项目号 +
    // 文件号的形态判定，见 mml_game::launcher::get_source_type —— 两边都有反例，
    // 所以给用户留一个能盖住的地方；两个号都没有时是"不是从平台下的"，显示空串
    let source = ctx
        .sources
        .get(mod_note_key(&file))
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            if project_id.is_empty() && file_id.is_empty() {
                String::new()
            } else {
                mml_game::launcher::get_source_type(&project_id, &file_id).to_string()
            }
        });

    // 前置本身不进前端，只报"有没有"：右键菜单靠它决定「依赖图」给不给点
    let has_dependencies = !mod_dependencies(item, ctx.loader).is_empty();

    Some(ModItemDto {
        uuid: item.uuid.to_string(),
        sha1,
        path: mod_rel_path(&item.file, ctx.game_path),
        disable: item.disable,
        fail: item.fail,
        core: item.core,
        library,
        mod_id,
        name,
        version: info.and_then(|i| i.version.clone()).unwrap_or_default(),
        author: info.map(|i| i.author.join(", ")).unwrap_or_default(),
        description: info.and_then(|i| i.description.clone()).unwrap_or_default(),
        loaders,
        side: mod_side_name(info.map(|i| i.side)),
        url: info.and_then(|i| i.url.clone()).unwrap_or_default(),
        source,
        project_id,
        file_id,
        note: mod_note(ctx.notes, &file),
        icon: data_url(icon),
        jar_in_jar: item
            .jar_in_jar
            .iter()
            .filter_map(|child| mod_item(child, ctx, true))
            .collect(),
        has_dependencies,
        file,
    })
}

/// 相对实例的路径：`.minecraft\mods\sodium.jar` 这种形态
///
/// - `file`: 模组文件的绝对路径
/// - `game_path`: 实例的游戏根目录，即 `.minecraft`
///
/// # 返回值
///
/// 相对游戏根目录的路径；裁不掉时退回完整路径，至少还能看
///
/// 与游戏根目录做前缀裁剪：整条绝对路径太长，而"游戏目录下的哪一层"这点信息是必要的，
/// 比如 mods / config 之类。
pub(super) fn mod_rel_path(file: &Path, game_path: &Path) -> String {
    let rel = file.strip_prefix(game_path).unwrap_or(file);
    rel.to_string_lossy().to_string()
}

/// 加载侧的名字，前端按 i18n 显示，这里只出稳定枚举名
///
/// - `side`: 内核给的加载侧；`None` 或未知值都算 `unknown`
///
/// # 返回值
///
/// `client` / `server` / `both` / `unknown`
pub(super) fn mod_side_name(side: Option<LoadSideType>) -> String {
    match side {
        Some(LoadSideType::Client) => "client",
        Some(LoadSideType::Server) => "server",
        Some(LoadSideType::Both) => "both",
        _ => "unknown",
    }
    .to_string()
}

/// 备注的定位键：**去掉禁用后缀**的模组文件名
///
/// - `file`: 模组文件名，可能带禁用后缀
///
/// # 返回值
///
/// 剥掉 `.disable` / `.disabled` 后缀的文件名；没有后缀时原样返回
///
/// `Mod.ModName` 的键是文件名，沿用 ColorMC 的写法，而启用 / 禁用只给文件名加减
/// `.disabled`，见 `mml_game::game_mods::add_disable_suffix` —— 不归一的话
/// "禁用一下就找不到自己的备注了"。
pub(crate) fn mod_note_key(file: &str) -> &str {
    file.strip_suffix(names::DISABLE_DOT_EXT)
        .or_else(|| file.strip_suffix(names::DISABLED_DOT_EXT))
        .unwrap_or(file)
}

/// 取某个模组的备注，没有则返回空串
///
/// - `notes`: `Mod.ModName` 表，文件名 → 备注
/// - `file`: 模组文件名，可能带禁用后缀
///
/// # 返回值
///
/// 备注文本；没有备注时是空串
///
/// 先按归一后的键找，找不到再按**原始**文件名找一次：ColorMC 在禁用状态下写的备注
/// 就存在带后缀的那个键上，别让它读不出来。
pub(crate) fn mod_note(notes: &HashMap<String, Option<String>>, file: &str) -> String {
    if file.is_empty() {
        return String::new();
    }
    notes
        .get(mod_note_key(file))
        .or_else(|| notes.get(file))
        .cloned()
        .flatten()
        .unwrap_or_default()
}

/// 取文件 SHA1，没有哈希时返回空串
///
/// - `hash`: 内核算出来的文件哈希
///
/// # 返回值
///
/// SHA1 十六进制串；只有 SHA256 / SHA512 时返回空串
///
/// 模组的自定义分组按 SHA1 记，与 `gui_setting.json` 的 `Mod.Groups` 一致：
/// 它是**内容哈希**，启用 / 禁用改文件名之后不变，而 uuid 会跟着路径变。
pub(crate) fn sha1_of(hash: &FileHash) -> String {
    match hash {
        FileHash::Sha1(value) | FileHash::Sha1Sha256(value, _) | FileHash::Sha1Sha512(value, _) => {
            value.clone()
        }
        _ => String::new(),
    }
}

/// 在 mods 目录里按 uuid 找到那个文件，启用 / 禁用 / 删除要用
///
/// - `instance`: 实例句柄
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// 只填了 `file` 与 `disable` 的模组对象；找不到返回 `err.fileNotFound`
///
/// **只列目录，不读文件内容**：uuid 是**文件路径**的 v5，见 `gen_mod_uuid`，
/// 所以按 uuid 定位根本用不上哈希，更用不上解析元数据。
///
/// 以前这里走 `read_mod_fast`，那会把**每个 jar 完整读一遍算 SHA1** ——
/// 几百个包要好几秒，而"禁用"只是给一个文件改名。用户要求"禁用之后不要重新读取"，
/// 这条是后端那一半：真正贵的就是它。
pub(crate) async fn find_mod(instance: GameInstance, mod_uuid: &str) -> Result<ModObj, String> {
    let mod_uuid = mod_uuid.to_string();
    let found = block_on_instance(instance, move |game| {
        path_helper::get_files(game.get_mods_path())
            .into_iter()
            .find(|path| mml_game::game_mods::gen_mod_uuid(path).to_string() == mod_uuid)
    })
    .await?;

    let path = found.ok_or_else(|| err_keys::FILE_NOT_FOUND.to_string())?;
    // 禁用状态由后缀决定，不需要读文件
    let disable = path.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case(names::DISABLE_EXT)
            || ext.eq_ignore_ascii_case(names::DISABLED_EXT)
    });

    Ok(ModObj {
        file: path,
        disable,
        ..Default::default()
    })
}

/// 把改名后的路径转成给前端的**新身份**
///
/// 启用 / 禁用只改文件名，元数据没动 —— 所以前端不必重扫，拿这三个值就地更新那一行即可。
///
/// - `instance`: 实例句柄
/// - `path`: 改名后的模组文件路径
///
/// # 返回值
///
/// 新的 uuid / 文件名 / 相对路径；取游戏根目录失败时返回错误串
pub(super) async fn mod_rename_dto(
    instance: GameInstance,
    path: PathBuf,
) -> Result<ModRenameDto, String> {
    let game_path = block_on_instance(instance, |game| game.get_game_path()).await?;

    Ok(ModRenameDto {
        // uuid 是路径的 v5，文件名变了就得重算
        uuid: mml_game::game_mods::gen_mod_uuid(&path).to_string(),
        file: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: mod_rel_path(&path, &game_path),
    })
}
