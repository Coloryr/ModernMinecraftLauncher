//! 网络请求模块
//!
//! 本模块是启动器的 HTTP 客户端层，封装了 `reqwest` 库，
//! 提供统一的网络请求接口，支持代理配置和多种请求方式。
//!
//! # 双客户端设计
//!
//! 本模块维护两个独立的 HTTP 客户端实例：
//!
//! - **WORK_CLIENT** — 用于一般网络请求，如下载资源、调用 API
//! - **LOGIN_CLIENT** — 用于登录相关请求，如 OAuth、Yggdrasil 认证
//!
//! 两者可独立配置不同的代理策略，确保登录流量和下载流量可以走不同的网络通道。
//!
//! # 子模块
//!
//! | 模块 | 用途 |
//! |------|------|
//! | [`mojang_api`] | Mojang 官方 API |
//! | [`curseforge_api`] | CurseForge API |
//! | [`modrinth_api`] | Modrinth API |
//! | [`fabric_api`] / [`quilt_api`] | 模组加载器 API |
//! | [`optifine_api`] | OptiFine 下载 |
//! | [`authlib_api`] | Authlib-Injector 下载 |
//! | [`adoptium_api`] | Adoptium Java 下载 |
//! | [`nide8_api`] | 统一通行证 API |
//! | [`liteloader_api`] | LiteLoader API |
//! | [`openfrp_api`] / [`sakurafrp_api`] | 联机平台 OpenFrp / SakuraFrp 的隧道与客户端信息 |
//! | [`chunkbase_api`] | Chunkbase 相关接口 |
//! | [`urls`] | URL 常量定义 |
//! | [`url_helper`] | URL 构建辅助函数 |
//! | [`maven_utils`] | Maven 坐标工具 |
//! | [`input_file`] | 输入文件抽象 |

use mml_names::i18_items::error_type::ErrorType;

pub mod adoptium_api;
pub mod authlib_api;
pub mod chunkbase_api;
pub mod curseforge_api;
pub mod fabric_api;
pub mod foojay_api;
mod html_scan;
pub mod input_file;
pub mod liteloader_api;
pub mod maven_utils;
pub mod mclo_api;
pub mod modrinth_api;
pub mod mojang_api;
pub mod nide8_api;
pub mod openfrp_api;
pub mod openj9_api;
pub mod optifine_api;
pub mod quilt_api;
pub mod sakurafrp_api;
pub mod url_helper;
pub mod urls;
pub mod zulu_api;

mod abort;
mod client;
mod error;
mod form;
mod global;
mod post;
mod ratelimit;
mod response;

/// 默认 HTTP 超时时间，单位秒：连接超时与单次读取超时共用
///
/// 注意是 `read_timeout` 单次读空闲超时，而非请求总超时：总超时会掐断
/// 慢速网络下的大文件下载，客户端 jar 有 40+MB
const DEFAULT_TIMEOUT: u64 = 10;

/// 默认 User-Agent 标识
const DEFAULT_USER_AGENT: &str = "mml/1.0.0";

/// 记录一次失败的请求
///
/// 请求失败一律在这里落一条错误日志，`logs.log` 里就能看到地址与原因。
///
/// - `err`: 失败原因
pub(crate) fn log_request_error(err: &ErrorType) {
    mml_log::error_type(err.clone());
}

pub(crate) use self::error::{http_error, serializer_err};

pub use self::abort::is_aborted;
pub use self::client::Client;
pub use self::form::FormField;
pub use self::global::{abort_all, get_login_client, get_work_client, init, rebuild};
pub use self::response::{handle_response, serialize_err_context};
