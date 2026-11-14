//! 实例文件补全：实例建好后自动把缺的版本 json、核心、运行库与资源文件下下来
//!
//! 补全直接复用内核的 `check_game_file`：它只做「检查 + 补齐」，不起游戏进程，
//! 缺的文件仍进全局下载器，下载管理窗口与标题栏指示器都能看进度。
//! 补全期间实例挂 `instance_task` 占用，删除 / 改名会被拒绝；实例列表的 `preparing`
//! 与主窗口启动按钮都读这里登记的阶段。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use mml_auth::LoginObj;
use mml_game::game_launch::{AutoJoinType, GameLaunchArg, LaunchDebugger};
use mml_game::gui_hook::{ILaunchGui, LaunchGui, LaunchState, ProcessRunType};
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use mml_names::i18_items::error_type::ErrorType;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::emit_instance_change;
use crate::dtos::ErrorEvent;
use crate::err_keys;
use crate::i18;
use crate::listens;
use crate::log_text::LogText;

/// 补全阶段，按「检查 → 下载」推进
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// 正在检查缺哪些文件
    Check,
    /// 正在下载缺的文件
    Download,
}

impl Phase {
    /// 给前端的状态名，与实例列表 DTO 的 `preparing` 取值一致
    ///
    /// # 返回值
    ///
    /// `check` 或 `download`
    fn id(self) -> &'static str {
        match self {
            Phase::Check => "check",
            Phase::Download => "download",
        }
    }
}

/// 每个实例当前的补全阶段；不在表里就是没在补全
static PHASE: LazyLock<Mutex<HashMap<Uuid, Phase>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// 补全任务的取消令牌，取消后线程在下一个检查点退出
static CANCEL: LazyLock<Mutex<HashMap<Uuid, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 取消实例的文件补全
///
/// 只置位取消令牌：线程在下一个检查点退出，已经下完的文件留在原地，下次启动接着补。
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 取消已发出返回 `true`；没有在补全返回 `false`，uuid 非法返回 `Err`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_cancel_prepare(uuid: String) -> Result<bool, String> {
    let id = Uuid::parse_str(&uuid).map_err(|_| err_keys::UUID.to_string())?;
    Ok(cancel(&id))
}

/// 开始补全实例缺失的文件
///
/// 补全失败后前端点「重试」走这里；已经在补全或实例正忙时不会重复开。
///
/// - `app`: 应用句柄，阶段变化与列表刷新都靠它发事件
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 已经开始或已在补全返回 `true`；实例不存在、正忙或 uuid 非法返回 `false`
#[gui_macros::ipc_group("main")]
#[gui_macros::command]
pub fn main_start_prepare(app: AppHandle, uuid: String) -> bool {
    let Ok(id) = Uuid::parse_str(&uuid) else {
        return false;
    };
    start(&app, &uuid);
    is_preparing(&id)
}

/// 补全失败事件：前端据此弹错误框，并提供「重试」
///
/// - `app`: 应用句柄，事件的发送方
/// - `event`: 失败详情，含实例 uuid 与错误文案
#[gui_macros::emit]
fn emit_prepare_error(app: &AppHandle, event: ErrorEvent) {
    let _ = app.emit(listens::PREPARE_ERROR, event);
}

/// 取实例当前的补全阶段
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// `check` / `download`；没在补全返回 `None`
pub(crate) fn phase_of(uuid: &Uuid) -> Option<&'static str> {
    let phase = PHASE.lock().ok()?;
    phase.get(uuid).map(|item| item.id())
}

/// 实例是否正在补全文件
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 检查或下载中返回 `true`
pub(crate) fn is_preparing(uuid: &Uuid) -> bool {
    PHASE.lock().is_ok_and(|phase| phase.contains_key(uuid))
}

/// 取消实例的补全
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 有任务被置位返回 `true`；没在补全返回 `false`
pub(crate) fn cancel(uuid: &Uuid) -> bool {
    let token = CANCEL.lock().ok().and_then(|map| map.get(uuid).cloned());
    match token {
        Some(token) => {
            token.cancel();
            true
        }
        None => false,
    }
}

/// 开始补全实例缺失的文件
///
/// 只在确实需要时启动：实例不存在、正在运行 / 启动 / 删除、已经在补全，
/// 或设置里关掉了自动下载都直接返回。补全跑在独立线程上，本函数不等它结束。
///
/// - `app`: 应用句柄，阶段变化与列表刷新都靠它发事件
/// - `uuid`: 实例 uuid 字符串，即创建命令拿到的那个
pub(crate) fn start(app: &AppHandle, uuid: &str) {
    let Ok(id) = Uuid::parse_str(uuid) else {
        return;
    };
    if is_preparing(&id) || !should_prepare(&id) {
        return;
    }
    let Some(core) = super::core_instance(uuid) else {
        return;
    };
    // 取到快照就放开实例锁：补全线程只读这份配置，不再碰内核实例表
    let inst = core.instance.read().unwrap().clone();
    // 占用从登记这一刻就算上：列表马上会把它标成忙，删除 / 改名随即被拒。
    // 守卫跟着线程走，补全收工才释放
    let Ok(busy) = crate::windows::instance_task::begin(uuid) else {
        return;
    };
    let Some(token) = begin(id) else {
        return;
    };

    emit_instance_change(app, "edit");

    let app_task = app.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("Mml Prepare Instance {id}"))
        // 补全 future 跨 await 持有内核的 std 锁守卫，不是 Send，只能在自己的线程上跑
        .spawn(move || {
            // 补全期间实例一直按「忙」处理，见 busy_of
            let _busy = busy;
            // 补全会碰内核的加载器 / 版本代码，panic 也不能把阶段留在表里，
            // 否则按钮永远停在"下载文件中"、还挡住启动：先收好结果，收尾照常跑
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                // 阶段回调要带走一份句柄；另一份留给线程收尾时刷新列表
                let gui_app = app_task.clone();
                match runtime {
                    Ok(runtime) => Some(runtime.block_on(async move {
                        let gui: LaunchGui = Some(Arc::new(PrepareGui {
                            app: gui_app,
                            uuid: id,
                        }));
                        let mut arg = GameLaunchArg {
                            // 补全不下发登录：真实凭据用不上，离线身份也不会去拉外置登录的库
                            auth: LoginObj::default(),
                            auto: AutoJoinType::None,
                            admin: false,
                            mixin: None,
                            // 服务器包同步留给启动：那一步会整份替换实例文件，不该在后台悄悄跑
                            skip_server_pack: true,
                            server_pack_skip: Vec::new(),
                            server_pack_cancel: None,
                            debugger: LaunchDebugger::None,
                            jvm_debug: None,
                        };
                        inst.check_game_file(&mut arg, gui, token).await
                    })),
                    Err(error) => {
                        let message = error.to_string();
                        mml_log::error(
                            LogText::PrepareFailed {
                                error: message.clone(),
                            }
                            .text(),
                        );
                        emit_prepare_error(
                            &app_task,
                            ErrorEvent {
                                uuid: Some(id.to_string()),
                                message,
                            },
                        );
                        None
                    }
                }
            }));

            finish(&id);
            emit_instance_change(&app_task, "edit");

            match outcome {
                // 用户取消不算失败：文件留在原地，下次启动接着补
                Ok(Some(Err(err))) if !matches!(err, ErrorType::TaskCancel) => {
                    let message = i18::get_error(err.clone());
                    mml_log::error_type(err);
                    emit_prepare_error(
                        &app_task,
                        ErrorEvent {
                            uuid: Some(id.to_string()),
                            message,
                        },
                    );
                }
                Ok(_) => {}
                Err(payload) => {
                    let message = panic_text(&payload);
                    mml_log::error(
                        LogText::Panic {
                            thread: format!("Mml Prepare Instance {id}"),
                            message: message.clone(),
                            location: String::new(),
                        }
                        .text(),
                    );
                    emit_prepare_error(
                        &app_task,
                        ErrorEvent {
                            uuid: Some(id.to_string()),
                            message,
                        },
                    );
                }
            }
        });

    if let Err(error) = spawned {
        finish(&id);
        emit_instance_change(app, "edit");
        let message = error.to_string();
        mml_log::error(
            LogText::PrepareFailed {
                error: message.clone(),
            }
            .text(),
        );
        emit_prepare_error(
            app,
            ErrorEvent {
                uuid: Some(id.to_string()),
                message,
            },
        );
    }
}

/// 取 panic 载荷里的文本
///
/// - `payload`: `catch_unwind` 给的原载荷
///
/// # 返回值
///
/// 载荷是 `&str` / `String` 时取其内容，其它类型给一句占位说明
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&'static str>() {
        return (*text).to_string();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    String::from("non-string panic payload")
}

/// 这次要不要给实例开补全
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 空闲、没在启动或删除、且设置允许自动下载时返回 `true`
fn should_prepare(uuid: &Uuid) -> bool {
    // 设置里关掉自动下载：不主动连网，留给启动时那条询问
    if !mml_config::read_config().http.auto_download {
        return false;
    }
    if mml_game::is_running(uuid) {
        return false;
    }
    if super::launch_gate::is_launching(*uuid) {
        return false;
    }
    !super::deleting::is_deleting(uuid)
}

/// 登记一次补全
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 本次任务的取消令牌；该实例已经在补全时返回 `None`
fn begin(uuid: Uuid) -> Option<CancellationToken> {
    let mut phase = PHASE.lock().unwrap();
    if phase.contains_key(&uuid) {
        return None;
    }
    phase.insert(uuid, Phase::Check);
    let token = CancellationToken::new();
    CANCEL.lock().unwrap().insert(uuid, token.clone());
    Some(token)
}

/// 结束补全登记，幂等
///
/// - `uuid`: 实例 uuid
fn finish(uuid: &Uuid) {
    PHASE.lock().unwrap().remove(uuid);
    CANCEL.lock().unwrap().remove(uuid);
}

/// 把阶段推进到「下载中」
///
/// - `uuid`: 实例 uuid
///
/// # 返回值
///
/// 阶段确实变了返回 `true`；没在补全或已经是下载中返回 `false`
fn set_download(uuid: &Uuid) -> bool {
    let mut phase = PHASE.lock().unwrap();
    match phase.get_mut(uuid) {
        Some(item) if *item != Phase::Download => {
            *item = Phase::Download;
            true
        }
        _ => false,
    }
}

/// 补全用的启动界面回调
///
/// 只认「进入下载」这一刻：把阶段翻到 `download` 并刷新实例列表，
/// 启动按钮随之从「检查文件中」换成「下载文件中」。
/// 这里**不**发 launch-state：那条事件只属于真正的启动，日志窗口与启动按钮按它画进度。
struct PrepareGui {
    /// 应用句柄，阶段变化时广播实例变更
    app: AppHandle,
    /// 补全中的实例
    uuid: Uuid,
}

#[async_trait::async_trait]
impl ILaunchGui for PrepareGui {
    /// 阶段推进：只有下载要标出来，检查阶段在登记补全时就已经记下了
    fn update_state(&self, _setting: &InstanceSettingObj, state: LaunchState) {
        if matches!(state, LaunchState::Download) && set_download(&self.uuid) {
            emit_instance_change(&self.app, "edit");
        }
    }

    /// 补全不登录，也就没有重试
    async fn login_fail(&self, _auth: &LoginObj) -> bool {
        false
    }

    /// 缺文件要不要下载：补全就是为了下载，恒同意
    async fn request_download_file(&self) -> bool {
        true
    }

    /// 补全不挑 Java
    fn no_java(&self, _java: i32) {}

    /// 补全不跑启动前 / 后命令
    fn launch_process(&self, _run_type: ProcessRunType) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/windows_main_prepare_tests.rs"]
mod tests;
