//! 全局客户端
//!
//! 工作与登录两个客户端实例、初始化与重建。

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, RwLock};

use mml_config::config_obj::{HttpObj, ProxyState, ProxyType};
use mml_names::i18;
use mml_names::i18_items::info_type::InfoType;

use super::abort::{ABORT_GEN, abort_all_tasks, abort_tx};
use super::client::Client;

/// 全局通用 HTTP 客户端，用于资源下载与一般 API 调用
static WORK_CLIENT: OnceLock<RwLock<Arc<Client>>> = OnceLock::new();

/// 全局登录 HTTP 客户端，用于 OAuth、Yggdrasil 认证
static LOGIN_CLIENT: OnceLock<RwLock<Arc<Client>>> = OnceLock::new();

/// 按当前配置构建一个客户端
///
/// 抽出来是为了让 [`init`] 与 [`rebuild`] 共用同一套「读配置 → 选代理」逻辑：
/// `User` 用用户填的地址，其余策略交给 `Client::new`。
///
/// - `state`: 代理策略，决定走不走用户填的地址
/// - `ptype`: 用户代理的类型，仅在 `state` 为 `User` 时使用
/// - `http`: 网络配置，代理地址与认证信息都在里面
///
/// # 返回值
///
/// 返回建好的客户端
pub(super) fn build_client(state: ProxyState, ptype: ProxyType, http: &HttpObj) -> Client {
    if state == ProxyState::User {
        Client::new_proxy(
            ptype,
            &http.proxy_ip,
            http.proxy_port,
            &http.proxy_user,
            &http.proxy_password,
        )
    } else {
        Client::new(state)
    }
}

/// 把代理配置描述成一行日志文本，不含密码
///
/// - `state`: 代理策略
/// - `ptype`: 代理类型，仅在 `state` 为 `User` 时体现
/// - `http`: 网络配置，只读其中的代理地址与用户名
///
/// # 返回值
///
/// 返回形如 `User/http 127.0.0.1:7890 认证=有` 的描述
pub(super) fn describe_proxy(state: ProxyState, ptype: ProxyType, http: &HttpObj) -> String {
    match state {
        ProxyState::User => i18::get_info(InfoType::ProxyUser {
            kind: match ptype {
                ProxyType::Http => "http",
                ProxyType::Sock4 => "socks4",
                ProxyType::Sock5 => "socks5",
            }
            .to_string(),
            ip: http.proxy_ip.clone(),
            port: http.proxy_port,
            auth: !http.proxy_user.is_empty(),
        }),
        ProxyState::None => i18::get_info(InfoType::ProxyNone),
        ProxyState::Auto => i18::get_info(InfoType::ProxyAuto),
    }
}

/// 初始化 HTTP 客户端
///
/// 根据配置中的代理设置分别创建通用客户端和登录客户端。
/// 应在程序启动时调用一次；之后改代理设置请用 [`rebuild`]。
pub fn init() {
    let config = mml_config::read_config();
    let http = &config.http;

    WORK_CLIENT.get_or_init(|| {
        RwLock::new(Arc::new(build_client(
            http.work_proxy,
            http.work_proxy_type,
            http,
        )))
    });
    LOGIN_CLIENT.get_or_init(|| {
        RwLock::new(Arc::new(build_client(
            http.login_proxy,
            http.login_proxy_type,
            http,
        )))
    });

    mml_log::info_type(InfoType::HttpClientInit {
        work: describe_proxy(http.work_proxy, http.work_proxy_type, http),
        login: describe_proxy(http.login_proxy, http.login_proxy_type, http),
    });
}

/// 按当前配置**重建**两个 HTTP 客户端，改代理设置后调用
///
/// 客户端存在 `RwLock<Arc<Client>>` 里，保存网络设置后调本函数即可立刻换上新配置，
/// 不必重启启动器。已经取走的 `Arc` 不受影响，仍指向旧客户端，但在途请求会先被中断，
/// 之后新取的请求一律用新客户端。
///
/// [`init`] 尚未调用时什么都不做：配置还没读，此时也没有请求在跑。
pub fn rebuild() {
    // 先中断所有在途请求，含限速等待，再换客户端：顺序反了的话，
    // 旧请求可能刚醒过来又用旧客户端发出去
    let aborted_gen = abort_all();
    mml_log::info_type(InfoType::HttpClientRestart {
        generation: aborted_gen,
    });

    let config = mml_config::read_config();
    let http = &config.http;

    if let Some(lock) = WORK_CLIENT.get() {
        let client = Arc::new(build_client(http.work_proxy, http.work_proxy_type, http));
        if let Ok(mut guard) = lock.write() {
            *guard = client;
        }
    }
    if let Some(lock) = LOGIN_CLIENT.get() {
        let client = Arc::new(build_client(http.login_proxy, http.login_proxy_type, http));
        if let Ok(mut guard) = lock.write() {
            *guard = client;
        }
    }

    mml_log::info_type(InfoType::HttpClientRestartDone {
        work: describe_proxy(http.work_proxy, http.work_proxy_type, http),
        login: describe_proxy(http.login_proxy, http.login_proxy_type, http),
    });
}

/// 中断所有在途请求
///
/// 自增世代号并**广播**给所有正在等待的请求，被中断的请求返回 [`ErrorType::Aborted`]
/// 对应的错误，调用方按"请求失败"处理即可，用户改代理后重试就是想要的结果。
/// 用 `watch` 广播存值、不丢信号，理由见 [`ABORT_GEN`]。
///
/// 单独暴露出来是为了让 [`rebuild`] 之外的场景，如退出登录、切源，也能用。
///
/// # 返回值
///
/// 返回自增后的世代号，用于日志排查"到底有没有真的中断"
pub fn abort_all() -> u64 {
    let generation = ABORT_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    // 广播最新世代，watch 存值，晚订阅的请求也能读到；接收端已全部丢弃时返回 Err，忽略即可
    let _ = abort_tx().send(generation);
    // **强制取消全部在途任务**：这是不依赖"请求被 poll"的硬手段
    abort_all_tasks();

    generation
}

/// 获取全局通用 HTTP 客户端
///
/// # 返回值
///
/// 返回当前生效的通用客户端，用于资源下载和一般 API 请求
pub fn get_work_client() -> Arc<Client> {
    WORK_CLIENT
        .get()
        .and_then(|lock| lock.read().ok().map(|guard| guard.clone()))
        .unwrap()
}

/// 获取全局登录 HTTP 客户端
///
/// # 返回值
///
/// 返回当前生效的登录客户端，用于 OAuth / Yggdrasil 认证请求
pub fn get_login_client() -> Arc<Client> {
    LOGIN_CLIENT
        .get()
        .and_then(|lock| lock.read().ok().map(|guard| guard.clone()))
        .unwrap()
}
