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

use mml_auth::{AuthType, auths, lock_user, locked_by_other, unlock_user};
use mml_game::game_launch::{AutoJoinType, GameLaunchArg};
use mml_game::gui_hook::{ILaunchGui, LaunchState, ProcessRunType};
use mml_game::launcher::instance_setting_obj::{InstanceSettingObj, ServerObj};
use mml_game::{InstanceExit, game_count, get_running_instances, is_running, stop_game};
use mml_names::i18_items::error_type::ErrorType;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::dtos::{AutoSaveTargetDto, ErrorEvent, ExitEvent, StateEvent};
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

/// 启动游戏
///
/// 账户取后端记的"当前账户"，实例取核心实例表。启动本身在后台任务里跑，命令立刻返回。
///
/// - `app`: 应用句柄，用于发启动状态 / 日志 / 退出事件
/// - `uuid`: 实例 uuid
/// - `world`: 启动后直接进入的存档目录名，空串或 `None` 为不自动进入
/// - `force`: 账户被占用时是否跳过占用检查，`Some(true)` 为用户在弹窗里选了继续
/// - `auto_save`: 实例配的自动进入存档这次用不用，`Some(false)` 为用户在弹窗里选了不进入
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
) -> Result<(), String> {
    launch_game(app, uuid, world, force, auto_save, None).await
}

/// 大厅的临时进服参数只作用于本次启动，不改实例配置。
pub(crate) async fn launch_game(
    app: AppHandle, uuid: String, world: Option<String>, force: Option<bool>, auto_save: Option<bool>,
    lobby: Option<(mml_auth::UserKeyObj, AutoJoinType)>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::GAME_NOT_FOUND.to_string())?;
    let reservation = launch_gate::begin_launch(id)?;
    let core = core_instance(&uuid).ok_or_else(|| err_keys::GAME_NOT_FOUND.to_string())?;

    let auth = lobby.as_ref().map(|(key, _)| key.clone()).or_else(auths::get_current)
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
        Some(dir) => inst
            .get_saves()
            .await
            .into_iter()
            .find(|item| item.path.file_name().is_some_and(|name| name == dir.as_str())),
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
    });
    let mut arg = GameLaunchArg {
        auth,
        auto,
        admin: gui_config::get().experimental.instance_admin,
        mixin: None,
        lang: None,
    };

    // 命令返回后这次启动还要跑完：实例快照与 app 都搬进后台线程，
    // 进度 / 日志 / 退出继续走事件。
    //
    // 用独立线程 + 自己的 current-thread 运行时，而不是 tauri 的 async 运行时：
    // `start_game` 的 future 不是 `Send`（内核里跨 await 持有 std 锁的读守卫），
    // 进不了 `tauri::async_runtime::spawn`。线程名带上实例 uuid，排查时认得出来。
    let launch_uuid = core.uuid;
    // 取消令牌与线程一起登记：界面上点「取消启动」时，命令拿到的就是这一份
    let cancel = CancellationToken::new();
    LAUNCH_CANCEL
        .lock()
        .unwrap()
        .insert(launch_uuid, cancel.clone());
    let spawned = std::thread::Builder::new()
        .name(format!("Mml Game Launch {launch_uuid}"))
        .spawn(move || {
            let _reservation = reservation;
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => {
                    mml_log::error(
                        LogText::LaunchRuntimeFailed {
                            error: err.to_string(),
                        }
                        .text(),
                    );
                    LAUNCH_CANCEL.lock().unwrap().remove(&launch_uuid);
                    unlock_user(&launch_uuid);
                    return;
                }
            };

            let uuid = core.uuid.to_string();
            runtime.block_on(async move {
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

            // 线程收工：成败都已定，把"启动中"与取消令牌摘掉（失败后要允许重试）
            LAUNCH_CANCEL.lock().unwrap().remove(&launch_uuid);
        });

    if let Err(err) = spawned {
        LAUNCH_CANCEL.lock().unwrap().remove(&launch_uuid);
        unlock_user(&launch_uuid);
        return Err(err.to_string());
    }

    Ok(())
}

/// 取消正在启动中的实例
///
/// 只在进程起来之前有效：取消令牌在启动线程收工时就摘掉，进游戏后要停请用 main_stop_game。
///
/// - uuid: 实例 uuid
///
/// # 返回值
///
/// 已发出取消返回 true；进程已经起来或不在启动中返回 false，uuid 非法返回 Err
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
    let token = LAUNCH_CANCEL.lock().unwrap().get(&id).cloned();
    match token {
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
    let Some(save) = inst
        .get_saves()
        .await
        .into_iter()
        .find(|save| save.path.file_name().is_some_and(|name| name == dir.as_str()))
    else {
        return Ok(None);
    };

    let level_name = if save.level_name.is_empty() {
        dir.clone()
    } else {
        save.level_name
    };
    Ok(Some(AutoSaveTargetDto { dir, level_name }))
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

/// 启动过程的界面回调：把内核的阶段推成 `launch-state`
struct LaunchGuiHook {
    /// 应用句柄，用于发状态事件
    app: AppHandle,
    /// 实例 uuid
    uuid: Uuid,
}

#[async_trait::async_trait]
impl ILaunchGui for LaunchGuiHook {
    /// 阶段推进：转成前端认的状态名与进度
    fn update_state(&self, _setting: &InstanceSettingObj, state: LaunchState) {
        // 进程真正起来时内核推 End；统计的「启动成功」以这一刻为准，
        // 与 ColorMC 在拿到进程句柄后调 LaunchDone 同一位置
        if matches!(&state, LaunchState::End) {
            game_count::launch_done(&self.uuid);
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

    /// 启动前 / 后命令：都放行
    fn launch_process(&self, _run_type: ProcessRunType) -> bool {
        true
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
    let _ = app.emit(listens::LAUNCH_STATE, event);
}

/// 游戏退出事件
///
/// - `app`: 应用句柄，事件的发送方
/// - `event`: 退出信息，含实例 uuid 与进程退出码
#[gui_macros::emit]
fn emit_game_exit(app: &AppHandle, event: ExitEvent) {
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
mod tests {
    use super::*;

    #[test]
    fn offline_launch_requires_a_premium_login() {
        assert!(offline_needs_premium(AuthType::Offline, false));
        assert!(!offline_needs_premium(AuthType::Offline, true));
        assert!(!offline_needs_premium(AuthType::OAuth, false));
        assert!(!offline_needs_premium(AuthType::Nide8, false));
    }
}
