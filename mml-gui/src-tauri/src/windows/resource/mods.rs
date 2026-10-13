//! 资源管理窗口的模组分类：扫描列表 / 启用禁用 / 删除 / 分组用的行数据
//!
//! 列表由内核 `read_mod` 扫描，含 jar-in-jar 的嵌套项与模组元数据；进度经
//! `resource-list-mods-progress` 事件上报；启用 / 禁用只改文件名，加减 `.disabled` 后缀，
//! 删除进回收站。
//!
//! `mod_note` / `mod_note_key` 是 `pub(super)`：模组备注的读写命令在 `mod.rs`
//! 的分组段里，测试也在那边。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mml_base::file_item::FileHash;
use mml_game::GameInstance;
use mml_game::game_mods::{LoadSideType, ModObj};
use mml_game::gui_hook::{IProgressGui, ProgressGui};
use mml_names::names;
use mml_sys::path_helper;
use tauri::{Emitter, WebviewWindow};

use crate::dtos::{ModItemDto, ModRenameDto, ModScanProgressDto};
use crate::err_keys;
use crate::listens;

use super::{block_on_instance, data_url, parse_instance};

// ==================== 模组 ====================

/// 模组扫描进度回调：把 `x/x` 直接 emit 给发起扫描的那个窗口
///
/// 与方块渲染的 `BlockRenderGui` 同一套做法，见 windows/block.rs：实现
/// [`IProgressGui`]，内核在扫描过程中回调，桌面壳负责转成前端事件。
struct ModScanGui {
    /// 发起这次扫描的资源窗口，进度事件只发给它
    window: WebviewWindow,
}

impl IProgressGui for ModScanGui {
    /// 扫描用不到进度文字，忽略，trait 要求实现
    ///
    /// - `_text`: 进度文字，本实现不用
    fn set_progress_text(&self, _text: Option<String>) {}

    /// 每解析完一个 jar 报一次 `(已完成, 总数)` → 转发为事件
    ///
    /// - `value`: 已完成的 jar 数
    /// - `all`: 总数；内核给不出时是 `None`，按 0 上报
    fn set_progress_now(&self, value: usize, all: Option<usize>) {
        emit_resource_list_mods_progress(
            &self.window,
            ModScanProgressDto {
                done: value,
                total: all.unwrap_or(0),
            },
        );
    }
}

/// 模组扫描进度事件，事件名 `resource-list-mods-progress`
///
/// - `window`: 接收事件的那个资源窗口，由 Tauri 注入
/// - `dto`: 进度载荷，已完成与总数
///
/// 解析一个 jar 要读元数据 / 图标 / 扫 class，几百个包要好几秒 ——
/// 界面靠它显示 `x/x`，否则用户面对空列表不知道是在跑还是卡住了。
#[gui_macros::emit]
pub fn emit_resource_list_mods_progress(window: &WebviewWindow, dto: ModScanProgressDto) {
    let _ = window.emit(listens::RESOURCE_LIST_MODS_PROGRESS, dto);
}

/// 模组列表：解析 jar 元数据，条目多时耗时数秒
///
/// - `window`: 发起这次扫描的那个资源窗口，由 Tauri 注入。扫描进度 `x/x` 只发给它，
///   不用全局广播：主窗口等也会收到，白跑一遍状态更新
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 模组条目，含 jar-in-jar 内置项；实例不存在或扫描失败返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_list_mods(
    window: WebviewWindow,
    uuid: String,
) -> Result<Vec<ModItemDto>, String> {
    let instance = parse_instance(&uuid)?;

    // 进度回调交给内核，与方块渲染同一套 gui_hook 接口；
    // 回调是同步的，直接 emit，不需要额外缓冲
    let gui: ProgressGui = Some(Arc::new(ModScanGui {
        window: window.clone(),
    }) as Arc<dyn IProgressGui>);

    // 四份附带数据一起读，省掉前端额外往返与按文件名 / SHA1 的合并：
    // 备注 `Mod.ModName`、在线信息 `online_info.json`、游戏根目录，后者用来算相对路径
    let (list, notes, online, game_path) = block_on_instance(instance, move |game| {
        let notes = crate::gui_setting::load(&game).mods.mod_name;
        let online = game.read_online_info();
        let game_path = game.get_game_path();
        let list =
            tokio::runtime::Handle::current().block_on(async { game.read_mod(false, gui).await });
        (list, notes, online, game_path)
    })
    .await?;

    // 按 SHA1 索引：在线信息表 OnlineInfoObj 的主键就是 SHA1，模组的 hash 正好是它
    let online_of: HashMap<String, mml_game::launcher::file_online_info_obj::OnlineInfoObj> =
        online
            .into_iter()
            .filter(|(_, info)| !info.sha1.is_empty())
            .map(|(_, info)| (info.sha1.clone(), info))
            .collect();

    let ctx = ModCtx {
        notes: &notes,
        online: &online_of,
        game_path: &game_path,
    };

    Ok(list
        .iter()
        .filter_map(|item| mod_item(item, &ctx, false))
        .collect())
}

/// `mod_item` 需要的几份外部数据，打包传，省得一路加参数
struct ModCtx<'a> {
    /// `gui_setting.json` 的 `Mod.ModName`，文件名 → 说明
    notes: &'a HashMap<String, Option<String>>,
    /// 实例的在线信息表，按 **SHA1** 索引
    online: &'a HashMap<String, mml_game::launcher::file_online_info_obj::OnlineInfoObj>,
    /// 实例的游戏根目录 `.minecraft`，用来算相对路径
    game_path: &'a Path,
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
fn mod_item(item: &ModObj, ctx: &ModCtx, nested: bool) -> Option<ModItemDto> {
    let file = item
        .file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let info = item.info.first();
    let mod_id = info.map(|i| i.mod_id.clone()).unwrap_or_default();
    let name = info.map(|i| i.name.clone()).unwrap_or_default();
    // 拿不到文件名才跳过，`file` 为空只可能是"不在 mods 目录下的东西"
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
            let name = entry.loaders.to_string();
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
    // 下载源由项目号 + 文件号的形态判定，见 mml_game::launcher::get_source_type：
    // 两个都没有就是"不是从平台下的"，显示空串而不是硬判成 CurseForge
    let source = if project_id.is_empty() && file_id.is_empty() {
        String::new()
    } else {
        mml_game::launcher::get_source_type(&project_id, &file_id).to_string()
    };

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
fn mod_rel_path(file: &Path, game_path: &Path) -> String {
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
fn mod_side_name(side: Option<LoadSideType>) -> String {
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
pub(super) fn mod_note_key(file: &str) -> &str {
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
pub(super) fn mod_note(notes: &HashMap<String, Option<String>>, file: &str) -> String {
    if file.is_empty() {
        return String::new();
    }
    notes
        .get(mod_note_key(file))
        .or_else(|| notes.get(file))
        .and_then(|value| value.clone())
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
fn sha1_of(hash: &FileHash) -> String {
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
async fn find_mod(instance: GameInstance, mod_uuid: &str) -> Result<ModObj, String> {
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
async fn mod_rename_dto(instance: GameInstance, path: PathBuf) -> Result<ModRenameDto, String> {
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

/// 启用模组，去掉 .disable / .disabled 后缀
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// **改名后的新身份**，前端拿它就地更新那一行，不重扫整个 mods 目录；
/// 找不到模组返回 `err.fileNotFound`，其余是改名失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mod_enable(uuid: String, mod_uuid: String) -> Result<ModRenameDto, String> {
    let instance = parse_instance(&uuid)?;
    let obj = find_mod(instance.clone(), &mod_uuid).await?;
    let new_path = obj.enable().map_err(|err| err.to_string())?;
    mod_rename_dto(instance, new_path).await
}

/// 禁用模组，追加 .disable 后缀
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// 改名后的新身份，同上；已禁用或文件不存在时返回错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_mod_disable(uuid: String, mod_uuid: String) -> Result<ModRenameDto, String> {
    let instance = parse_instance(&uuid)?;
    let obj = find_mod(instance.clone(), &mod_uuid).await?;
    let new_path = obj.disable().map_err(|err| err.to_string())?;
    mod_rename_dto(instance, new_path).await
}

/// 删除模组，文件进回收站
///
/// - `uuid`: 实例 uuid
/// - `mod_uuid`: 模组 uuid，由文件路径算出
///
/// # 返回值
///
/// 只表示成功 / 失败：找不到模组返回 `err.fileNotFound`，其余是删除失败的错误串
#[gui_macros::ipc_group("resource")]
#[tauri::command]
pub async fn resource_delete_mod(uuid: String, mod_uuid: String) -> Result<(), String> {
    let instance = parse_instance(&uuid)?;
    let obj = find_mod(instance, &mod_uuid).await?;
    obj.delete().map_err(|err| err.to_string())
}
