//! 独立云同步服务，兼容 ColorMC 的 POST 端点与返回码。

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use mml_cloud::{Error, MAX_ARCHIVE, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

mod store;

/// 服务器配置文件。
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    /// 绑定地址，如 127.0.0.1:48080。
    pub listen: String,
    /// 私有数据目录，不能由静态 HTTP 服务暴露。
    pub data_dir: PathBuf,
    /// 所有客户端共有的服务器密钥。
    pub server_key: String,
    /// 授权客户端密钥到配额字节数的映射。
    pub clients: BTreeMap<String, u64>,
}

/// 正在运行的服务状态，磁盘操作在阻塞线程串行提交。
struct Service {
    /// 鉴权配置。
    config: Config,
    /// 防止并发更新绕过配额或覆盖快照。
    store: Mutex<store::Store>,
}

fn same_secret(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |value, (a, b)| value | (a ^ b))
        == 0
}

fn authorized(config: &Config, headers: &HeaderMap) -> bool {
    let server = headers
        .get("serverkey")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let client = headers
        .get("clientkey")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    same_secret(server, &config.server_key)
        && config.clients.keys().any(|key| same_secret(client, key))
}

async fn authenticate(
    State(state): State<Arc<Service>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !authorized(&state.config, request.headers()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"res":300}))).into_response();
    }
    next.run(request).await
}

async fn dispatch(
    State(state): State<Arc<Service>>,
    Path(action): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        let mut store = state.store.lock().map_err(|_| Error::new("err.cloudIo"))?;
        store.handle(&state.config, &action, &headers, &body)
    })
    .await;
    match result {
        Ok(Ok(store::Reply::Json(value))) => {
            let status = if value["res"] == 100 {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(value)).into_response()
        }
        Ok(Ok(store::Reply::Zip(bytes))) => (
            [(axum::http::header::CONTENT_TYPE, "application/zip")],
            bytes,
        )
            .into_response(),
        Ok(Err(err)) => {
            let code = match err.key {
                "err.cloudNotEnabled" => 101,
                "err.cloudMissing" => 102,
                "err.cloudQuota" | "err.cloudLimit" => 104,
                "err.cloudConflict" => 105,
                "err.cloudIo" => 500,
                _ => 400,
            };
            (StatusCode::BAD_REQUEST, Json(json!({"res":code}))).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"res":500}))).into_response(),
    }
}

/// 初始化数据目录并构造路由，不绑定网络端口。
///
/// - `config`: 授权列表、配额和存储目录
///
/// # 返回值
///
/// 可交给 axum::serve 的 Router，配置或数据目录非法时返回错误。
pub fn router(config: Config) -> Result<Router> {
    if config.server_key.len() < 16
        || config.clients.is_empty()
        || config
            .clients
            .iter()
            .any(|(key, quota)| key.len() < 16 || *quota == 0)
    {
        return Err(Error::new("err.cloudSettings"));
    }
    let store = store::Store::new(&config.data_dir)?;
    let state = Arc::new(Service {
        config,
        store: Mutex::new(store),
    });
    Ok(Router::new()
        .route("/{action}", post(dispatch))
        .layer(DefaultBodyLimit::max(MAX_ARCHIVE))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state))
}

/// 读取无 BOM 的 JSON 配置，相对数据目录按配置文件所在目录解析。
///
/// - `path`: 配置文件路径
///
/// # 返回值
///
/// 规范化数据目录后的配置。
pub fn read_config(path: &std::path::Path) -> Result<Config> {
    let mut config: Config = serde_json::from_slice(&fs::read(path)?)?;
    if config.data_dir.is_relative() {
        config.data_dir = path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(config.data_dir);
    }
    Ok(config)
}
