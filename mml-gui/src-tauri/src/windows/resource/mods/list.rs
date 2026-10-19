//! 模组列表：扫描与列表命令
//!
//! 扫描结果进窗口 model，列表命令只把内核对象转成 DTO。

use std::collections::HashMap;
use std::sync::Arc;

use mml_game::game_mods::{DependantType, ModObj, mods_duplicate_test};
use mml_game::gui_hook::{IProgressGui, ProgressGui};
use mml_game::launcher::file_online_info_obj::OnlineInfoObj;
use mml_game::loader::LoaderType;
use tauri::{Emitter, WebviewWindow};

use crate::dtos::{ModDuplicateDto, ModItemDto, ModScanProgressDto};

use crate::err_keys;
use crate::listens;

use crate::windows::resource::model::{ModCache, window_model};
use crate::windows::resource::{block_on_instance, parse_instance};

// ==================== 模组 ====================
use super::item::{ModCtx, mod_item};
use super::actions::{mod_duplicate_dto};


/// 模组扫描进度回调：把 `x/x` 直接 emit 给发起扫描的那个窗口
///
/// 与方块渲染的 `BlockRenderGui` 同一套做法，见 windows/block.rs：实现
/// [`IProgressGui`]，内核在扫描过程中回调，桌面壳负责转成前端事件。
pub(super) struct ModScanGui {
    /// 发起这次扫描的资源窗口，进度事件只发给它
    pub(super) window: WebviewWindow,
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

/// 模组重复安装事件，事件名 `resource-mods-duplicate`
///
/// - `window`: 接收事件的那个资源窗口，由 Tauri 注入
/// - `dto`: 重复的 modid 列表，非空
///
/// 重复不是依赖问题，列表加载完就报：前端自动弹窗列出每个 modid 与涉及的文件，
/// 不必用户自己点「依赖检测」。
#[gui_macros::emit]
pub fn emit_resource_mods_duplicate(window: &WebviewWindow, dto: Vec<ModDuplicateDto>) {
    let _ = window.emit(listens::RESOURCE_MODS_DUPLICATE, dto);
}

/// 模组列表：解析 jar 元数据，条目多时耗时数秒
///
/// 扫描结果会存进**窗口模型**（见 [`crate::windows::resource::model`]）：更新检查、依赖检测都按这份数据算，
/// 所以一个窗口里真正扫 mods 目录的只有这里。
///
/// - `window`: 发起这次扫描的那个资源窗口，由 Tauri 注入。扫描进度 `x/x` 只发给它，
///   不用全局广播：主窗口等也会收到，白跑一遍状态更新
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 模组条目，含 jar-in-jar 内置项；实例不存在或扫描失败返回错误串
#[gui_macros::ipc_group("resource")]
#[gui_macros::command]
pub async fn resource_list_mods(
    window: WebviewWindow,
    uuid: String,
) -> Result<Vec<ModItemDto>, String> {
    let instance = parse_instance(&uuid)?;

    // 窗口模型：这一趟的扫描结果存进去，更新检查 / 依赖检测直接读，不再重扫 mods 目录。
    // 存不进去（注册表异常）也不该拖垮列表，这里允许没有模型
    let model = window_model(&uuid).ok();
    let op = model.as_ref().map(|model| model.begin());

    // 进度回调交给内核，与方块渲染同一套 gui_hook 接口；
    // 回调是同步的，直接 emit，不需要额外缓冲
    let gui: ProgressGui = Some(Arc::new(ModScanGui {
        window: window.clone(),
    }) as Arc<dyn IProgressGui>);

    // 几份附带数据一起读，省掉前端额外往返与按文件名 / SHA1 的合并：
    // 备注 `Mod.ModName`、在线信息 `online_info.json`、游戏根目录，后者用来算相对路径
    let (list, notes, sources, online, game_path, loader, version) =
        block_on_instance(instance, move |game| {
            let mods = crate::gui_setting::load(&game).mods;
            let notes = mods.mod_name;
            let sources = mods.mod_source;
            let online = game.read_online_info();
            let game_path = game.get_game_path();
            let loader = game.loader;
            // 更新检查要按游戏版本与加载器过滤平台上的文件，一并存进缓存
            let version = game.version.clone();
            let list = tokio::runtime::Handle::current()
                .block_on(async { game.read_mod(false, gui).await });
            (list, notes, sources, online, game_path, loader, version)
        })
        .await?;

    // 扫描期间窗口被关了：不用再拼 DTO、也不必写缓存
    if op.as_ref().is_some_and(|op| op.is_cancelled()) {
        return Err(err_keys::CANCELLED.to_string());
    }

    // 按 SHA1 索引：在线信息表 OnlineInfoObj 的主键就是 SHA1，模组的 hash 正好是它。
    // 这里克隆一份（条目不大），原始表要留给窗口模型
    let online_of: HashMap<String, OnlineInfoObj> = online
        .values()
        .filter(|info| !info.sha1.is_empty())
        .map(|info| (info.sha1.clone(), info.clone()))
        .collect();

    let ctx = ModCtx {
        notes: &notes,
        sources: &sources,
        online: &online_of,
        game_path: &game_path,
        loader,
    };

    // 重复安装与依赖分开：列表照常返回，重复项另发事件给前端自动弹窗
    let duplicates: Vec<ModDuplicateDto> = mods_duplicate_test(&list)
        .into_iter()
        .map(mod_duplicate_dto)
        .collect();
    if !duplicates.is_empty() {
        emit_resource_mods_duplicate(&window, duplicates);
    }

    let items: Vec<ModItemDto> = list
        .iter()
        .filter_map(|item| mod_item(item, &ctx, false))
        .collect();

    // 扫描结果存进窗口模型：更新检查这类"按现有数据检查"的操作直接用这份，不再重扫
    if let Some(model) = &model {
        model.set_cache(ModCache {
            list,
            online,
            version,
            loader,
        });
    }

    Ok(items)
}

/// 一个模组条目声明的依赖
pub(crate) struct ModDependencyRef {
    /// 前置的 modid，原样大小写
    pub(crate) mod_id: String,
    /// 推荐安装（非强制）
    pub(crate) optional: bool,
    /// 版本要求原文，不限版本是 `*`
    pub(crate) required: String,
}

/// 取一个模组条目声明的依赖
///
/// 只收与实例加载器一致的那份元数据，与依赖检测同一口径；同一条前置可能被多份元数据
/// 重复声明，按 (modid, 版本范围, 是否可选) 去重。
///
/// - `item`: 模组条目
/// - `loader`: 实例的加载器
///
/// # 返回值
///
/// 依赖声明，按元数据顺序
pub(crate) fn mod_dependencies(item: &ModObj, loader: LoaderType) -> Vec<ModDependencyRef> {
    let mut list: Vec<ModDependencyRef> = Vec::new();
    for entry in item.info.iter().filter(|entry| entry.loaders == loader) {
        for dependant in entry.dependants.iter() {
            let (dep, optional) = match dependant {
                DependantType::Required(dep) => (dep, false),
                DependantType::Recommend(dep) => (dep, true),
            };
            if dep.modid.is_empty() {
                continue;
            }
            let required = dep.version.to_string();
            let exists = list.iter().any(|each| {
                each.mod_id == dep.modid && each.required == required && each.optional == optional
            });
            if !exists {
                list.push(ModDependencyRef {
                    mod_id: dep.modid.clone(),
                    optional,
                    required,
                });
            }
        }
    }
    list
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
