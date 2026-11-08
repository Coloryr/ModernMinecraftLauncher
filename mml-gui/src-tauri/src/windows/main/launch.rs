//! 主窗口的启动 / 停止与运行态查询
//!
//! 真实启动全在 `mml_game`：这里只取实例、取当前账户、组装 [`GameLaunchArg`]，
//! 然后把启动交给 `start_game`，命令自身立刻返回。
//!
//! 事件分工：启动阶段推进走 `launch-state`，游戏输出走 `game-log`（订阅在 lib.rs），
//! 进程退出走 `game-exit`（订阅在 lib.rs），启动失败走 `launch-error`。
//! 运行态一律以 `mml_game` 为准（`is_running` / `get_running_instances`），
//! 窗口模型不再自己记一份。
//!
//! 命令带分组标记，值填原始组名 main，把组键钉回 `main`，见 AGENTS.md §4。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use mml_auth::{AuthType, auths, lock_user, locked_by_other, unlock_user};
use mml_config::config_obj::RunArgObj;
use mml_game::game_launch::{AutoJoinType, GameLaunchArg, LaunchDebugger, MixinArg};
use mml_game::gui_hook::{ILaunchGui, LaunchState, ProcessRunType};
use mml_game::launcher::instance_setting_obj::{InstanceSettingObj, ServerObj};
use mml_game::{InstanceExit, game_count, get_running_instances, is_running, stop_game};
use mml_names::i18_items::error_type::ErrorType;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::dtos::{
    AutoSaveTargetDto, DebuggerInfoDto, ErrorEvent, ExitEvent, LaunchCommandDto, NsightInstallDto,
    RenderDocLaunchDto, StateEvent,
};
use crate::err_keys;
use crate::gui_config;
use crate::i18;
use crate::listens;
use crate::log_text::LogText;
use crate::windows::log::emit_log_line;

use super::args::core_instance;
use super::{emit_launch_error, launch_gate};
pub(crate) use launch_gate::import_guard;

/// 正在启动中的实例的取消令牌：界面点「取消启动」时置位，启动线程在检查点退出
///
/// 只覆盖"进程还没起来"那一段；进程起来后令牌就摘掉，要停游戏走 stop_game。
static LAUNCH_CANCEL: LazyLock<Mutex<HashMap<Uuid, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 服务器包同步自己的取消令牌：界面上点「跳过更新」时只置位这一份，启动继续
///
/// 生命周期与 [`LAUNCH_CANCEL`] 相同：启动线程收工就摘掉，整次启动被取消时两份一起置位。
static SERVER_PACK_CANCEL: LazyLock<Mutex<HashMap<Uuid, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 最近一次启动阶段：窗口重载 / 启动后才打开日志窗口时，靠它补回当前阶段
///
/// 只保留"进程还没起来"的阶段；end / post / cancelled / 失败 / 退出都会删掉。
static LAUNCH_STATES: LazyLock<Mutex<HashMap<Uuid, StateEvent>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 这一局用 RenderDoc 启动的实例：记下进程起来的时刻
///
/// 退出时按它挑出本次产生的捕获，再交给 Qt 版界面；不是 RenderDoc 启动的实例没有这一项。
static RENDERDOC_SESSIONS: LazyLock<Mutex<HashMap<Uuid, SystemTime>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 设置里的全局"自动进服"：实例没配自己的加入服务器时用它
///
/// 对应 ColorMC 的 `ServerCustom.JoinServer`，实例级设置优先。
///
/// # 返回值
///
/// 开关开着且地址非空时给出服务器；否则 `None`
fn global_auto_join() -> Option<ServerObj> {
    let config = crate::gui_config::get();
    if !config.client.auto_join {
        return None;
    }
    // 地址允许写成 host 或 host:port，端口缺省 25565，与 MOTD 那个框同一套解析
    let addr = super::catalog::parse_motd_addr(&config.client.auto_join_server);
    if addr.host.is_empty() {
        return None;
    }
    Some(ServerObj {
        enable: true,
        ip: Some(addr.host),
        port: Some(addr.port),
    })
}

/// 读取实例的 options.txt 语言；没有就按启动器语言补一条
///
/// 新实例第一次启动时还没有 options.txt，游戏会按英文起；这里补上启动器语言，
/// 之后玩家在游戏里改的语言也会留在 options.txt，再次启动以它为准。
/// 语言只走 options.txt，不进启动参数：`--userProperties` 的 JSON 形状按版本
/// 各不相同（1.7.10 要数组、1.8.x 要 authlib 的 PropertyMap），塞字符串会崩在 Main。
///
/// - `inst`: 实例配置
fn ensure_game_lang(inst: &InstanceSettingObj) {
    let options = match inst.get_minecraft_options() {
        Ok(options) => options,
        Err(err) => {
            mml_log::error_type(err);
            return;
        }
    };
    if let Some(current) = options.get("lang").filter(|lang| !lang.trim().is_empty()) {
        // 已经有值：大小写与该版本对不上时改写。旧版写 zh_CN、新版写 zh_cn，
        // 写错游戏直接不认，表现为"选了语言进去还是英文"
        if let Some(code) = mml_game::game_lang_code(&inst.version, current.trim())
            && code != current.trim()
            && let Err(err) = inst.set_minecraft_option("lang", Some(&code))
        {
            mml_log::error_type(err);
        }
        return;
    }

    // 同一种语言旧版写 zh_CN、新版写 zh_cn：以该版本资源索引里的写法为准，
    // 写错了游戏不认，进去还是英文
    let want = mml_names::get_lang(crate::gui_config::locale());
    let lang = mml_game::game_lang_code(&inst.version, want).unwrap_or_else(|| want.to_string());
    if let Err(err) = inst.set_minecraft_option("lang", Some(&lang)) {
        mml_log::error_type(err);
    }
}

/// 启动游戏
///
/// 账户取后端记的"当前账户"，实例取核心实例表。启动本身在后台任务里跑，命令立刻返回。
///
/// - `app`: 应用句柄，用于发启动状态 / 日志 / 退出事件
/// - `uuid`: 实例 uuid
/// - `world`: 启动后直接进入的存档目录名，空串或 `None` 为不自动进入
/// - `force`: 账户被占用时是否跳过占用检查，`Some(true)` 为用户在弹窗里选了继续
/// - `auto_save`: 实例配的自动进入存档这次用不用，`Some(false)` 为用户在弹窗里选了不进入
/// - `server_pack`: 在线服务器包这次怎么处理：`Some("skip")` 不比对清单直接启动，
///   其余（含 `None`）照旧按发布地址同步；要不要问用户由前端在启动前决定
/// - `server_pack_skip`: 这次同步里先不更新的文件，取清单里的相对路径
/// - `run_commands`: 启动前 / 后命令这次执不执行，`Some(false)` 为用户在弹窗里选了跳过
/// - `renderdoc`: 是否用 RenderDoc 启动
///
/// # 返回值
///
/// 已交给后台任务时返回 `Ok(())`；实例不存在 / 没有当前账户 / 已在运行 /
/// 没登录过正版账户却要用离线账户启动 / 账户已被其它实例占用 返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub async fn main_launch_game(
    app: AppHandle,
    uuid: String,
    world: Option<String>,
    force: Option<bool>,
    auto_save: Option<bool>,
    server_pack: Option<String>,
    server_pack_skip: Option<Vec<String>>,
    run_commands: Option<bool>,
    renderdoc: Option<bool>,
    nsight: Option<bool>,
) -> Result<(), String> {
    // Nsight Graphics 只能由用户自己装，启动时再确认一次本机还在
    let debugger = if nsight.unwrap_or(false) {
        let install = mml_tools::nsight::find().ok_or_else(|| err_keys::NSIGHT_MISSING.to_string())?;
        LaunchDebugger::Nsight(install.cli)
    } else if renderdoc.unwrap_or(false) {
        LaunchDebugger::RenderDoc
    } else {
        LaunchDebugger::None
    };
    launch_game(
        app,
        uuid,
        world,
        force,
        auto_save,
        server_pack,
        server_pack_skip.unwrap_or_default(),
        run_commands,
        debugger,
        None,
    )
    .await
}

/// 启动实例的完整实现，主窗口命令与大厅共用
///
/// 大厅传来的账户与进服参数只作用于本次启动，不改实例配置。
///
/// - `app`: 应用句柄，用于发启动状态 / 日志 / 退出事件
/// - `uuid`: 实例 uuid
/// - `world`: 启动后直接进入的存档目录名，`None` 或空串表示不指定
/// - `force`: 账户被占用时是否跳过占用检查
/// - `auto_save`: 实例配的自动进入存档这次用不用，`Some(false)` 为不进入
/// - `server_pack`: 服务器包处理方式，`Some("skip")` 表示不比对清单直接启动
/// - `server_pack_skip`: 这次同步里先不更新的文件，取清单里的相对路径
/// - `run_commands`: 启动前 / 后命令这次执不执行，`Some(false)` 为跳过；`None` 为执行
/// - `renderdoc`: 是否用 RenderDoc 启动
/// - `lobby`: 大厅的临时账户与进服方式，`None` 表示走当前账户与常规规则
///
/// # 返回值
///
/// 已交给后台线程时返回 `Ok(())`；其余错误与 `main_launch_game` 相同
pub(crate) async fn launch_game(
    app: AppHandle,
    uuid: String,
    world: Option<String>,
    force: Option<bool>,
    auto_save: Option<bool>,
    server_pack: Option<String>,
    server_pack_skip: Vec<String>,
    run_commands: Option<bool>,
    debugger: LaunchDebugger,
    lobby: Option<(mml_auth::UserKeyObj, AutoJoinType)>,
) -> Result<(), String> {
    // 批量资源映射正在搬模组文件，这时启动会和它抢同一批文件
    if crate::windows::settings::resource_link::running() {
        return Err(err_keys::RESOURCE_MAP_RUNNING.to_string());
    }

    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::GAME_NOT_FOUND.to_string())?;
    let reservation = launch_gate::begin_launch(id)?;
    let core = core_instance(&uuid).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;

    let auth = lobby
        .as_ref()
        .map(|(key, _)| key.clone())
        .or_else(auths::get_current)
        .and_then(|key| auths::get(&key.uuid, key.auth_type))
        .ok_or_else(|| err_keys::ACCOUNT_MISSING.to_string())?;

    // 离线账户的启动前提是本机登录过正版（微软）账户：皮肤站 / 外置登录不算证明，只认 OAuth。
    // 与 ColorMC 同一处（GameBinding.LaunchAsync → UserBinding.GetLaunchUser），
    // 那边拒绝时顺手把官网打开，引导用户去登录 / 购买
    if offline_needs_premium(auth.auth_type, auths::check_online_user()) {
        mml_sys::open_helper::open_url("https://www.minecraft.net/");
        return Err(err_keys::OFFLINE_NEED_PREMIUM.to_string());
    }

    // 账户占用检查：同一账户同时开两个实例可能导致登录凭据互相顶掉。
    // 用户可以在弹窗里选继续，并把检查关掉；`force` 是这一次的临时放行。
    // 占用表在 `mml_auth`，键是账户、值是占用它的实例，登录流程也能看到同一份状态
    let account = auth.get_key();
    if force != Some(true)
        && gui_config::get().client.account_check
        && locked_by_other(&account, &core.uuid)
    {
        return Err(err_keys::ACCOUNT_IN_USE.to_string());
    }

    // 实例快照 + 自启参数。优先级：右键指定的存档 → 实例配的"加入服务器"
    // → 实例配的"进入存档" → 设置里的全局自动进服
    let inst = core.instance.read().unwrap().clone();
    let explicit = world.filter(|dir| !dir.is_empty());
    let wanted = explicit.clone().or_else(|| {
        // 用户在"自动进入存档"弹窗里选了这次不进入
        if auto_save == Some(false) {
            None
        } else {
            inst.auto_save.clone().filter(|dir| !dir.is_empty())
        }
    });
    // 存档按目录名找回来：目录被删/改名时当作找不到，别让启动卡在一个不存在的存档上
    let save = match wanted {
        Some(dir) => inst.get_saves().await.into_iter().find(|item| {
            item.path
                .file_name()
                .is_some_and(|name| name == dir.as_str())
        }),
        None => None,
    };
    let auto = if explicit.is_some() {
        // 明确点了"进入存档"就只认这个存档：找不到时直接报错，别悄悄换成进服 / 什么都不进
        match save {
            Some(save) => AutoJoinType::Save(Arc::new(save)),
            None => return Err(err_keys::WORLD_GONE.to_string()),
        }
    } else {
        match &inst.start_server {
            Some(server) if server.enable => AutoJoinType::Server(Arc::new(server.clone())),
            _ => match save {
                Some(save) => AutoJoinType::Save(Arc::new(save)),
                None => match global_auto_join() {
                    Some(server) => AutoJoinType::Server(Arc::new(server)),
                    None => AutoJoinType::None,
                },
            },
        }
    };
    let auto = lobby.map(|(_, auto)| auto).unwrap_or(auto);
    let setting = inst.clone();
    let link_instance = core.instance.clone();

    lock_user(&account, core.uuid);

    emit_launch_state(
        &app,
        StateEvent {
            uuid: uuid.clone(),
            state: "launching".into(),
            progress: None,
        },
    );
    emit_log_line(&app, &uuid, &LogText::LaunchStarting.text(), true);

    let gui = Arc::new(LaunchGuiHook {
        app: app.clone(),
        uuid: core.uuid,
        run_commands: run_commands.unwrap_or(true),
        // 只有 RenderDoc 那一局要在退出后打开捕获；Nsight 由它自己的界面看
        renderdoc: matches!(debugger, LaunchDebugger::RenderDoc),
    });
    // IPC 服务没起来时不注入 ColorASM，免得游戏连到一个不存在的端口
    let mixin = match (mml_ipc::get_port(), mml_ipc::get_auth_key()) {
        (Some(port), Some(key)) => Some(MixinArg {
            port,
            key: key.to_string(),
        }),
        _ => None,
    };
    // options.txt 里还没有语言时按启动器语言补一条
    ensure_game_lang(&inst);
    // 整次启动的取消令牌与服务器包同步那份令牌要一起建：后者是前者的子令牌，
    // 取消启动会自动带上它，而界面上点「跳过更新」只撤销它，同步停下、启动继续
    let cancel = CancellationToken::new();
    let pack_cancel = cancel.child_token();
    SERVER_PACK_CANCEL
        .lock()
        .unwrap()
        .insert(core.uuid, pack_cancel.clone());
    let mut arg = GameLaunchArg {
        auth,
        auto,
        admin: gui_config::get().experimental.instance_admin,
        mixin,
        // 用户在这次升级弹窗里选了「直接启动」：启动时不再比对服务器包清单
        skip_server_pack: server_pack.as_deref() == Some("skip"),
        // 同步时这条清单里没勾的模组不下载，旧文件留着
        server_pack_skip,
        // 只跳过这次服务器包同步的令牌；整次启动的取消走 LAUNCH_CANCEL
        server_pack_cancel: Some(pack_cancel),
        debugger,
    };

    // 命令返回后这次启动还要跑完：实例快照与 app 都搬进后台线程，
    // 进度 / 日志 / 退出继续走事件。
    //
    // 用独立线程 + 自己的 current-thread 运行时，而不是 tauri 的 async 运行时：
    // `start_game` 的 future 不是 `Send`（内核里跨 await 持有 std 锁的读守卫），
    // 进不了 `tauri::async_runtime::spawn`。线程名带上实例 uuid，排查时认得出来。
    let launch_uuid = core.uuid;
    // 取消令牌与线程一起登记：界面上点「取消启动」时，命令拿到的就是这一份
    LAUNCH_CANCEL
        .lock()
        .unwrap()
        .insert(launch_uuid, cancel.clone());
    let spawned = std::thread::Builder::new()
        .name(format!("Mml Game Launch {launch_uuid}"))
        .spawn(move || {
            let _reservation = reservation;
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    mml_log::error(
                        LogText::LaunchRuntimeFailed {
                            error: err.to_string(),
                        }
                        .text(),
                    );
                    LAUNCH_CANCEL.lock().unwrap().remove(&launch_uuid);
                    SERVER_PACK_CANCEL.lock().unwrap().remove(&launch_uuid);
                    unlock_user(&launch_uuid);
                    return;
                }
            };

            let uuid = core.uuid.to_string();
            // 启动 future 把 app 移走了，panic 收尾这条路径还得用它发错误事件
            let panic_app = app.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                runtime.block_on(async move {
                    // 启动前检查资源映射：缺文件先补下载 / 硬链接，失败就中止这次启动
                    if let Err(err) =
                        mml_game::game_link::check_instance_links(&link_instance, cancel.clone())
                            .await
                    {
                        if !matches!(err, ErrorType::TaskCancel) {
                            game_count::launch_error(&launch_uuid);
                        }
                        unlock_user(&launch_uuid);
                        if matches!(err, ErrorType::TaskCancel) {
                            emit_launch_state(
                                &app,
                                StateEvent {
                                    uuid: uuid.clone(),
                                    state: "cancelled".into(),
                                    progress: None,
                                },
                            );
                            return;
                        }
                        mml_log::error_type(err.clone());
                        let text = i18::get_error(err);
                        emit_log_line(&app, &uuid, &text, false);
                        emit_launch_error(
                            &app,
                            ErrorEvent {
                                uuid: Some(uuid.clone()),
                                message: text,
                            },
                        );
                        return;
                    }

                    match setting.start_game(&mut arg, Some(gui), cancel).await {
                        Ok(()) => {
                            emit_log_line(&app, &uuid, &LogText::LaunchProcessStarted.text(), false)
                        }
                        Err(err) => {
                            // 进程已经起来、只是启动后命令失败时不算启动失败，否则会多记一次
                            let started = is_running(&launch_uuid);
                            if !started {
                                unlock_user(&launch_uuid);
                            }
                            // 用户取消不算启动失败：不记统计、不弹错误，只推一条状态让各窗口收回启动态
                            if matches!(err, ErrorType::TaskCancel) {
                                if !started {
                                    emit_launch_state(
                                        &app,
                                        StateEvent {
                                            uuid: uuid.clone(),
                                            state: "cancelled".into(),
                                            progress: None,
                                        },
                                    );
                                }
                                return;
                            }
                            if !started {
                                game_count::launch_error(&launch_uuid);
                            }
                            mml_log::error_type(err.clone());
                            let text = i18::get_error(err.clone());
                            emit_log_line(&app, &uuid, &text, false);
                            emit_launch_error(
                                &app,
                                ErrorEvent {
                                    uuid: Some(uuid),
                                    message: text,
                                },
                            );
                        }
                    }
                });
            }));

            if let Err(payload) = result {
                let reason = panic_reason(payload);
                let text = LogText::LaunchPanicked { reason }.text();
                mml_log::error(text.clone());
                // 进程没起来才放开账户；已经起来的交给退出事件解锁
                if !is_running(&launch_uuid) {
                    unlock_user(&launch_uuid);
                    emit_launch_error(
                        &panic_app,
                        ErrorEvent {
                            uuid: Some(launch_uuid.to_string()),
                            message: text,
                        },
                    );
                }
            }

            // 线程收工：成败都已定，把"启动中"与两个取消令牌摘掉（失败后要允许重试）
            LAUNCH_CANCEL.lock().unwrap().remove(&launch_uuid);
            SERVER_PACK_CANCEL.lock().unwrap().remove(&launch_uuid);
        });

    if let Err(err) = spawned {
        LAUNCH_CANCEL.lock().unwrap().remove(&launch_uuid);
        SERVER_PACK_CANCEL.lock().unwrap().remove(&launch_uuid);
        unlock_user(&launch_uuid);
        return Err(err.to_string());
    }

    Ok(())
}

/// 取消正在启动中的实例
///
/// 只在进程起来之前有效：取消令牌在启动线程收工时就摘掉，进游戏后要停请用 main_stop_game。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 已发出取消返回 `true`；进程已经起来或不在启动中返回 `false`，uuid 非法返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_cancel_launch(uuid: String) -> Result<bool, String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::UUID.to_string());
    };
    // 进程已经起来就不叫"取消启动"了，前端应改用关闭游戏
    if is_running(&id) {
        return Ok(false);
    }
    // 整次启动取消：服务器包同步那份是它的子令牌，会跟着一起停
    let token = LAUNCH_CANCEL.lock().unwrap().get(&id).cloned();
    match token {
        Some(token) => {
            token.cancel();
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 跳过这次服务器包更新，继续启动
///
/// 只置位服务器包同步那份令牌：下载停下，已经落地的文件保持现状，启动照常进游戏。
/// 下次启动还会按发布地址重新比对，把这次没同步完的补上。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 已发出跳过返回 `true`；不在启动中或进程已经起来返回 `false`，uuid 非法返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_skip_server_pack_update(uuid: String) -> Result<bool, String> {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return Err(err_keys::UUID.to_string());
    };
    // 进程已经起来就没什么可跳过的了
    if is_running(&id) {
        return Ok(false);
    }
    match SERVER_PACK_CANCEL.lock().unwrap().get(&id).cloned() {
        Some(token) => {
            token.cancel();
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 启动前取实例配的自动进入存档，给前端弹窗确认用
///
/// 自动进入提醒被关掉、实例没配、版本低于 1.20（没有 quick play）、
/// 或那个存档已经不在时都返回 `None`，前端直接启动即可。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 需要确认时返回存档目录名与显示名；实例不存在返回 `err.gameNotFound`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub async fn main_auto_save_target(uuid: String) -> Result<Option<AutoSaveTargetDto>, String> {
    if !gui_config::get().client.auto_save_notice {
        return Ok(None);
    }
    let core = core_instance(&uuid).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;
    let inst = core.instance.read().unwrap().clone();
    let Some(dir) = inst.auto_save.clone().filter(|dir| !dir.is_empty()) else {
        return Ok(None);
    };
    // 低版本没有 --quickPlaySingleplayer，配置了也进不去，别白问一次
    if !inst.is_game_version_120() {
        return Ok(None);
    }
    let Some(save) = inst.get_saves().await.into_iter().find(|save| {
        save.path
            .file_name()
            .is_some_and(|name| name == dir.as_str())
    }) else {
        return Ok(None);
    };

    let level_name = if save.level_name.is_empty() {
        dir.clone()
    } else {
        save.level_name
    };
    Ok(Some(AutoSaveTargetDto { dir, level_name }))
}

/// 取实例这次启动会执行的启动前 / 后命令，给前端弹窗确认用
///
/// 实例自己没配时回落到启动器设置里的全局命令，口径与内核 `start_game` 一致。
/// 提醒被关掉、两种命令都没配时返回 `None`，前端直接启动即可。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 需要确认时返回命令文本；实例不存在返回 `err.gameNotFound`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_launch_commands(uuid: String) -> Result<Option<LaunchCommandDto>, String> {
    if !gui_config::get().client.command_notice {
        return Ok(None);
    }
    let core = core_instance(&uuid).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;
    let inst = core.instance.read().unwrap().clone();
    let commands = effective_commands(&inst, &mml_config::read_config().jvm_arg);
    Ok((commands.pre.is_some() || commands.post.is_some()).then_some(commands))
}

/// 实例这次启动会执行的启动前 / 后命令
///
/// 与内核 `start_game` 同一口径：实例设了用实例的，没设回落到全局；
/// 开关打开且命令非空才算会执行。
///
/// - `inst`: 实例配置
/// - `fallback`: 全局启动参数
///
/// # 返回值
///
/// 见 [`LaunchCommandDto`]；不会执行的命令是 `None`
fn effective_commands(inst: &InstanceSettingObj, fallback: &RunArgObj) -> LaunchCommandDto {
    let run = inst.jvm_arg.as_ref().unwrap_or(fallback);
    let pre_on = run
        .launch_pre_run
        .or(fallback.launch_pre_run)
        .unwrap_or(false);
    let pre_cmd = run
        .pre_run_arg
        .as_deref()
        .or(fallback.pre_run_arg.as_deref())
        .unwrap_or("");
    let post_on = run
        .launch_post_run
        .or(fallback.launch_post_run)
        .unwrap_or(false);
    let post_cmd = run
        .post_run_arg
        .as_deref()
        .or(fallback.post_run_arg.as_deref())
        .unwrap_or("");
    LaunchCommandDto {
        pre: (pre_on && !pre_cmd.is_empty()).then(|| pre_cmd.to_string()),
        post: (post_on && !post_cmd.is_empty()).then(|| post_cmd.to_string()),
    }
}

/// 停止游戏
///
/// 退出事件由内核的退出轮询线程发出，这里只负责结束进程；没在跑时静默返回。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 始终返回 `Ok(())`；uuid 非法返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_stop_game(uuid: String) -> Result<(), String> {
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    stop_game(&id);
    Ok(())
}

/// 获取运行中实例
///
/// # 返回值
///
/// 运行中实例的 uuid 列表，取自内核的实例句柄表
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_get_running() -> Vec<String> {
    get_running_instances()
        .iter()
        .map(|uuid| uuid.to_string())
        .collect()
}

/// 转发内核的实例退出事件
///
/// 订阅在 lib.rs 的启动流程里，与 `game-log` 的转发同一处。
///
/// - `app`: 应用句柄，事件的发送方
/// - `event`: 内核的实例退出事件
pub(crate) fn forward_game_exit(app: &AppHandle, event: &InstanceExit) {
    // 进程退出，账户不再被这个实例占用
    unlock_user(&event.uuid);
    // 先结掉这一局的运行时长，再通知前端；退出轮询线程不是 tokio 线程，
    // 所以统计落盘走 game_count 自己的普通线程，见 save_async
    game_count::game_close(&event.uuid);
    emit_game_exit(
        app,
        ExitEvent {
            uuid: event.uuid.to_string(),
            code: event.code,
        },
    );
    // RenderDoc 启动的这一局结束了：把本次的捕获交给 Qt 版界面
    open_renderdoc_capture(app, &event.uuid);
    // 运行态变了：托盘跟着显隐。最后一个实例退出、主界面又已经关掉时，
    // 托盘收掉之后就再没有唤回入口了，直接退出，见 tray::quit_idle
    crate::tray::sync(app);
    if app
        .get_webview_window(crate::windows::main_window_label())
        .is_none()
    {
        crate::tray::quit_idle(app);
    }
}

/// RenderDoc 启动的这一局结束了：用 Qt 版界面打开本次产生的捕获
///
/// 没按过 F12、或便携包里没带界面程序时只记一行日志，不当成错误。
///
/// - `app`: 应用句柄，用于写日志
/// - `uuid`: 实例 uuid
fn open_renderdoc_capture(app: &AppHandle, uuid: &Uuid) {
    let Some(since) = RENDERDOC_SESSIONS.lock().unwrap().remove(uuid) else {
        return;
    };
    let uuid = uuid.to_string();
    let app = app.clone();
    // qrenderdoc 起界面要一两秒，别占着退出轮询线程
    let _ = std::thread::Builder::new()
        .name("Mml RenderDoc Capture".to_string())
        .spawn(move || {
            let text = match (
                mml_tools::renderdoc::find_gui(),
                mml_tools::renderdoc::latest_capture(since),
            ) {
                (Some(gui), Some(capture)) => {
                    let path = capture.to_string_lossy().into_owned();
                    match std::process::Command::new(&gui).arg(&capture).spawn() {
                        Ok(_) => LogText::RenderDocOpened { path }.text(),
                        Err(err) => LogText::RenderDocOpenFailed {
                            path,
                            error: err.to_string(),
                        }
                        .text(),
                    }
                }
                _ => LogText::RenderDocNoCapture.text(),
            };
            emit_log_line(&app, &uuid, &text, false);
        });
}

/// 从 panic 载荷里取可读文本，取不到时返回空串
///
/// - `payload`: `catch_unwind` 给的原载荷
///
/// # 返回值
///
/// panic 时传的 `String` 或 `&'static str`；其它类型转不出来
fn panic_reason(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(text) => *text,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(text) => (*text).to_string(),
            Err(_) => String::new(),
        },
    }
}

/// 启动过程的界面回调：把内核的阶段推成 `launch-state`
struct LaunchGuiHook {
    /// 应用句柄，用于发状态事件
    app: AppHandle,
    /// 实例 uuid
    uuid: Uuid,
    /// 这次启动是否执行启动前 / 后命令，用户在弹窗里选了跳过时为 false
    run_commands: bool,
    /// 这次是否用 RenderDoc 启动：进程起来时登记，退出时按它决定要不要打开捕获
    renderdoc: bool,
}

#[async_trait::async_trait]
impl ILaunchGui for LaunchGuiHook {
    /// 阶段推进：转成前端认的状态名与进度
    fn update_state(&self, _setting: &InstanceSettingObj, state: LaunchState) {
        // 进程真正起来时内核推 End；统计的「启动成功」以这一刻为准，
        // 与 ColorMC 在拿到进程句柄后调 LaunchDone 同一位置
        if matches!(&state, LaunchState::End) {
            game_count::launch_done(&self.uuid);
            // RenderDoc 启动的这一局：记下进程起来的时刻，退出时按它挑本次的捕获
            if self.renderdoc {
                RENDERDOC_SESSIONS
                    .lock()
                    .unwrap()
                    .insert(self.uuid, SystemTime::now());
            }
            // 进程真正起来了：托盘这时才出现，关窗之后靠它唤回界面
            crate::tray::sync(&self.app);
        }
        emit_launch_state(
            &self.app,
            StateEvent {
                uuid: self.uuid.to_string(),
                state: state_key(&state).to_string(),
                progress: Some(state.progress()),
            },
        );
    }

    /// 登录失败：不重登，错因由启动结果统一讲
    async fn login_fail(&self, _auth: &mml_auth::LoginObj) -> bool {
        false
    }

    /// 缺文件要不要下：内核只在配置里关掉自动下载时才问，这里一律同意
    async fn request_download_file(&self) -> bool {
        true
    }

    /// 没有可用的 Java：写进启动日志，让原因在界面上看得到
    fn no_java(&self, java: i32) {
        emit_log_line(
            &self.app,
            &self.uuid.to_string(),
            &LogText::LaunchNoJava { java }.text(),
            false,
        );
    }

    /// 启动前 / 后命令：按用户在弹窗里的选择放行或跳过
    fn launch_process(&self, _run_type: ProcessRunType) -> bool {
        self.run_commands
    }
}

/// 启动阶段对应的前端状态名，与 i18n 的 `state.*` 一一对应
///
/// - `state`: 内核给的阶段
///
/// # 返回值
///
/// 返回前端状态名；新增阶段时这里的 match 会因非穷尽而编译失败，强制同步
fn state_key(state: &LaunchState) -> &'static str {
    match state {
        LaunchState::Login => "login",
        LaunchState::Check => "check",
        LaunchState::ReadInfo => "readinfo",
        LaunchState::Download => "download",
        LaunchState::Jvm => "jvm",
        LaunchState::RenderDoc => "renderdoc",
        LaunchState::Pre => "pre",
        LaunchState::Post => "post",
        LaunchState::End => "end",
        LaunchState::LoadServerPack => "loadserverpack",
        LaunchState::CheckServerPack => "checkserverpack",
        LaunchState::DownloadServerPack => "downloadserverpack",
    }
}

/// 启动状态事件
///
/// - `app`: 应用句柄，事件的发送方
/// - `event`: 状态快照，含实例 uuid、状态标识与进度百分比；进度可缺省
#[gui_macros::emit]
fn emit_launch_state(app: &AppHandle, event: StateEvent) {
    if let Ok(uuid) = Uuid::parse_str(&event.uuid) {
        // End 表示进程已经拉起：启动后命令还没跑完，但界面不再显示启动中
        if event.state == "end" {
            launch_gate::mark_started(uuid);
        }
        let mut states = LAUNCH_STATES.lock().unwrap();
        if matches!(event.state.as_str(), "end" | "post" | "cancelled") {
            states.remove(&uuid);
        } else {
            states.insert(uuid, event.clone());
        }
    }
    let _ = app.emit(listens::LAUNCH_STATE, event);
}

/// 检测本机可用的图形调试器
///
/// RenderDoc 由启动器自带，不用检测；这里只找 Nsight Graphics——它得用户自己装，
/// 而且只认 NVIDIA 的默认安装位置。
///
/// # 返回值
///
/// 见 [`DebuggerInfoDto`]；没装时 `nsight` 为 `None`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_detect_debuggers() -> DebuggerInfoDto {
    DebuggerInfoDto {
        nsight: mml_tools::nsight::find().map(|install| NsightInstallDto {
            cli: install.cli.to_string_lossy().into_owned(),
            gui: install.gui.map(|path| path.to_string_lossy().into_owned()),
        }),
    }
}

/// 生成 RenderDoc 界面启动要填的参数
///
/// 与正常启动走同一套参数生成：登录、挑 Java、拼 JVM 参数与环境变量，只是不拉起进程。
/// 生成期间占住实例的启动位，免得和真正的启动同时进行。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`RenderDocLaunchDto`]；实例不存在、没有当前账户或生成参数失败时返回错误
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub async fn main_renderdoc_launch_info(uuid: String) -> Result<RenderDocLaunchDto, String> {
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    let _reservation = launch_gate::begin_launch(id)?;
    let core = core_instance(&uuid).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;
    let auth = auths::get_current()
        .and_then(|key| auths::get(&key.uuid, key.auth_type))
        .ok_or_else(|| err_keys::ACCOUNT_MISSING.to_string())?;
    if offline_needs_premium(auth.auth_type, auths::check_online_user()) {
        return Err(err_keys::OFFLINE_NEED_PREMIUM.to_string());
    }

    let inst = core.instance.read().unwrap().clone();
    let mut arg = GameLaunchArg {
        auth,
        auto: AutoJoinType::None,
        admin: false,
        mixin: None,
        // 只是生成命令：不比对服务器包，也没有启动线程要取消
        skip_server_pack: true,
        server_pack_skip: Vec::new(),
        server_pack_cancel: None,
        debugger: LaunchDebugger::None,
    };
    // 参数生成要跨 await 持有配置的 std 读锁，future 不是 Send，不能直接在命令里跑；
    // 与启动线程同一个办法：另开一个 current-thread 运行时，在阻塞线程里跑完再把结果带回来
    let cmd = tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| err.to_string())?;
        runtime
            .block_on(inst.create_game_cmd(&mut arg, &CancellationToken::new(), None))
            .map_err(i18::get_error)
    })
    .await
    .map_err(|err| err.to_string())??;

    // qrenderdoc 的环境变量框是一行一条 KEY=VALUE，排序后两次生成的结果也一致
    let mut environment: Vec<String> = cmd
        .env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    environment.sort();

    Ok(RenderDocLaunchDto {
        executable: cmd.java.to_string_lossy().into_owned(),
        working_dir: cmd.dir.to_string_lossy().into_owned(),
        arguments: quote_args(&cmd.arg),
        environment,
    })
}

/// 把参数列表拼成一行命令行文本
///
/// - `args`: 已拆好的参数
///
/// # 返回值
///
/// 每个参数按 Windows 命令行规则加引号后用空格连接，可直接粘进 qrenderdoc 的「启动参数」
fn quote_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 单个参数按 Windows 命令行规则加引号
///
/// 不含空白与引号的参数原样返回；引号与末尾反斜杠按 CRT 规则转义，粘进别的程序解析回来才是原值。
///
/// - `arg`: 参数原文
///
/// # 返回值
///
/// 加好引号的文本
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut pending = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => pending += 1,
            '"' => {
                out.push_str(&"\\".repeat(pending * 2 + 1));
                out.push('"');
                pending = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(pending));
                out.push(ch);
                pending = 0;
            }
        }
    }
    // 末尾反斜杠要加倍，否则会把收尾的引号转义掉
    out.push_str(&"\\".repeat(pending * 2));
    out.push('"');
    out
}

/// 清掉某个实例的启动阶段快照
///
/// 启动失败的事件没有阶段状态，由 `emit_launch_error` 调这里收尾。
///
/// - `uuid`: 实例 uuid
pub(crate) fn clear_launch_state(uuid: &str) {
    if let Ok(id) = Uuid::parse_str(uuid) {
        LAUNCH_STATES.lock().unwrap().remove(&id);
    }
}

/// 当前实例还没走完的启动阶段
///
/// 窗口重载或日志窗口在启动中途才打开时，前端先用它补一次状态；没有进行中的启动返回 `None`。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 见 [`StateEvent`]；不在启动中返回 `None`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_launch_state(uuid: String) -> Option<StateEvent> {
    let id = Uuid::parse_str(&uuid).ok()?;
    // 进程已经起来或预约已释放时快照一定是旧的：顺手清掉，别让按钮停在启动中
    if !launch_gate::is_starting(id) {
        LAUNCH_STATES.lock().unwrap().remove(&id);
        return None;
    }
    LAUNCH_STATES.lock().unwrap().get(&id).cloned()
}

/// 游戏退出事件
///
/// - `app`: 应用句柄，事件的发送方
/// - `event`: 退出信息，含实例 uuid 与进程退出码
#[gui_macros::emit]
fn emit_game_exit(app: &AppHandle, event: ExitEvent) {
    if let Ok(uuid) = Uuid::parse_str(&event.uuid) {
        LAUNCH_STATES.lock().unwrap().remove(&uuid);
    }
    let _ = app.emit(listens::GAME_EXIT, event);
}

/// 离线账户是否缺少正版登录前提
///
/// - `auth_type`: 本次启动使用的账户类型
/// - `has_premium`: 本机是否至少登录过一个微软正版账户
///
/// # 返回值
///
/// 离线账户且没有正版登录记录时返回 `true`
fn offline_needs_premium(auth_type: AuthType, has_premium: bool) -> bool {
    auth_type == AuthType::Offline && !has_premium
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_main_launch_tests.rs"]
mod tests;
