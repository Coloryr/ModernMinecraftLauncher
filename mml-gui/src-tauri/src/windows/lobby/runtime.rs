//! 陶瓦 HTTP 适配，控制流始终限定在本机回环地址。

use super::model::LobbyModel;
use crate::{
    dtos::{LobbyProfileDto, LobbyStateDto},
    err_keys,
};
use serde::Deserialize;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio_util::sync::CancellationToken;

/// 陶瓦进程租约锁：同一时间只允许一个会话安装、启动并退出工作进程
static WORKER: LazyLock<Arc<Mutex<()>>> = LazyLock::new(|| Arc::new(Mutex::new(())));

/// 陶瓦控制接口的查询参数，允许公共节点同名重复。
pub(super) struct QueryParameter {
    /// 协议参数名。
    pub name: &'static str,
    /// 尚未进行 URL 编码的参数值。
    pub value: String,
}

/// 从安装到退出清理独占陶瓦进程，阻止新大厅越过旧会话的后台任务。
pub(super) struct WorkerLease {
    /// 解包线程和退出任务全部结束后才释放。
    _guard: OwnedMutexGuard<()>,
}

impl WorkerLease {
    /// 等上一份租约释放，窗口关闭时不再等
    ///
    /// - `gate`: 全局租约锁
    /// - `cancel`: 窗口取消令牌
    ///
    /// # 返回值
    ///
    /// 租约；等待期间窗口关闭返回 `err.lobbyClosed`
    async fn acquire(
        gate: Arc<Mutex<()>>,
        cancel: &CancellationToken,
    ) -> Result<Arc<Self>, String> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(err_keys::LOBBY_CLOSED.into()),
            guard = gate.lock_owned() => Ok(Arc::new(Self { _guard: guard })),
        }
    }
}

/// 大厅持有的陶瓦 HTTP 会话及退出责任。
pub(super) struct Session {
    /// 不经过代理的本机 HTTP 客户端。
    client: reqwest::Client,
    /// 陶瓦回环地址。
    base: String,
    /// 是否由本次会话新建工作进程。
    owned: bool,
    /// 是否由本大厅发起了尚未退出的房间请求。
    room_owned: AtomicBool,
    /// 释放会话时移交后台清理任务。
    worker: Option<Arc<WorkerLease>>,
}

/// 陶瓦写出的端口文件
#[derive(Deserialize)]
struct PortFile {
    /// 监听端口；启动还没完成时是 0
    port: u16,
}

/// `/meta` 的响应
#[derive(Deserialize)]
struct Meta {
    /// 陶瓦版本
    version: String,
}

/// `/state` 里的一个联机档案
#[derive(Deserialize)]
struct Profile {
    /// 档案所属机器的标识
    machine_id: String,
    /// 玩家名
    name: String,
    /// 档案类型，如 `LOCAL`
    kind: String,
    /// 提供该档案的陶瓦实现
    vendor: String,
}

/// `/state` 的响应
#[derive(Deserialize)]
struct State {
    /// 房间状态，如 `waiting` / `guest-ok`
    state: String,
    /// 状态序号，越大越新
    index: u64,
    /// 房间号，还没进房间时为空
    room: Option<String>,
    /// 加入方要连的地址 `host:port`
    url: Option<String>,
    /// 房间难度
    difficulty: Option<String>,
    /// 异常码，协议里的 `type`
    exception: Option<u32>,
    /// 联机档案列表
    #[serde(default)]
    profiles: Vec<Profile>,
}

/// 会话结束时删掉陶瓦写的端口文件
struct PortGuard(PathBuf);
impl Drop for PortGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(format!("{}.tmp", self.0.display()));
    }
}

impl Session {
    /// 安装并启动陶瓦，或复用用户已经打开的那个实例
    ///
    /// - `model`: 大厅模型，下载进度写进它的视图
    ///
    /// # 返回值
    ///
    /// 会话与陶瓦版本；安装、启动、取版本失败返回错误
    pub async fn start(model: &Arc<LobbyModel>) -> Result<(Arc<Self>, String), String> {
        let worker = WorkerLease::acquire(WORKER.clone(), &model.cancel).await?;
        let executable = super::install::executable(model, worker.clone()).await?;
        model.check()?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| e.to_string())?;
        // --hmcl 可能复用用户已打开的陶瓦，此时关大厅只退出房间，不结束外部进程。
        let old_port = existing_port();
        let existing = if let Some(port) = old_port {
            client
                .get(format!("http://127.0.0.1:{port}/meta"))
                .send()
                .await
                .ok()
                .filter(|r| r.status().is_success())
                .map(|_| port)
        } else {
            None
        };
        let port_file = PortGuard(
            executable
                .parent()
                .unwrap()
                .join(format!("session-{}.json", uuid::Uuid::new_v4())),
        );
        let mut command = Command::new(&executable);
        command
            .arg("--hmcl")
            .arg(&port_file.0)
            .current_dir(executable.parent().unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let ready = async {
            for _ in 0..150 {
                if let Ok(data) = std::fs::read(&port_file.0)
                    && let Ok(port) = serde_json::from_slice::<PortFile>(&data)
                    && port.port != 0
                {
                    return Ok(port.port);
                }
                if child
                    .try_wait()
                    .map_err(|e| e.to_string())?
                    .is_some_and(|s| !s.success())
                {
                    return Err(err_keys::LOBBY_START.into());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(err_keys::LOBBY_START.into())
        };
        let port = tokio::select! {
            _ = model.cancel.cancelled() => Err(err_keys::LOBBY_CLOSED.to_string()),
            result = ready => result,
        };
        let port = match port {
            Ok(port) => port,
            Err(error) => {
                let cleanup_client = client.clone();
                tauri::async_runtime::spawn(async move {
                    let _worker = worker;
                    for _ in 0..80 {
                        if let Ok(data) = std::fs::read(&port_file.0)
                            && let Ok(port) = serde_json::from_slice::<PortFile>(&data)
                        {
                            if port.port != 0 && existing != Some(port.port) {
                                cleanup(
                                    &cleanup_client,
                                    &format!("http://127.0.0.1:{}", port.port),
                                    true,
                                )
                                .await;
                            }
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                });
                return Err(error);
            }
        };
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        let session = Arc::new(Self {
            client,
            base: format!("http://127.0.0.1:{port}"),
            owned: existing != Some(port),
            room_owned: AtomicBool::new(false),
            worker: Some(worker),
        });
        let meta: Meta = session
            .client
            .get(format!("{}/meta", session.base))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        model.check()?;
        Ok((session, meta.version))
    }

    /// 拉一次陶瓦状态
    ///
    /// # 返回值
    ///
    /// 见 [`LobbyStateDto`]；请求或解析失败返回错误文案
    pub async fn state(&self) -> Result<LobbyStateDto, String> {
        let state: State = self
            .client
            .get(format!("{}/state", self.base))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(LobbyStateDto {
            state: state.state,
            index: state.index,
            room: state.room,
            address: state.url,
            difficulty: state.difficulty,
            exception: state.exception,
            profiles: state
                .profiles
                .into_iter()
                .map(|p| LobbyProfileDto {
                    id: p.machine_id,
                    name: p.name,
                    kind: p.kind,
                    vendor: p.vendor,
                })
                .collect(),
        })
    }

    /// 发一条状态切换请求
    ///
    /// - `path`: 控制接口路径，如 `/state/scanning`
    /// - `query`: 查询参数，同名参数允许重复
    ///
    /// # 返回值
    ///
    /// 陶瓦接受返回 `Ok(())`；请求失败或被拒绝返回错误文案
    pub async fn action(&self, path: &str, query: &[QueryParameter]) -> Result<(), String> {
        let mut url =
            reqwest::Url::parse(&format!("{}{path}", self.base)).map_err(|e| e.to_string())?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(
                query
                    .iter()
                    .map(|parameter| (parameter.name, parameter.value.as_str())),
            );
        }
        let was_owned = self.room_owned.load(Ordering::Acquire);
        if path != "/state/ide" {
            // 请求途中取消时仍需清理；明确被拒绝才恢复原有归属。
            self.room_owned.store(true, Ordering::Release);
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if let Err(error) = response.error_for_status_ref() {
            self.room_owned.store(was_owned, Ordering::Release);
            return Err(error.to_string());
        }
        self.room_owned
            .store(path != "/state/ide", Ordering::Release);
        Ok(())
    }
}

impl Drop for Session {
    /// 按归属清理：自带的工作进程退出，外部实例只退出房间
    fn drop(&mut self) {
        if !self.owned && !self.room_owned.load(Ordering::Acquire) {
            return;
        }
        let client = self.client.clone();
        let base = self.base.clone();
        let owned = self.owned;
        let worker = self.worker.take();
        tauri::async_runtime::spawn(async move {
            let _worker = worker;
            cleanup(&client, &base, owned).await;
        });
    }
}

/// 退出请求完成后再等自有进程释放 HTTP 端口
///
/// - `client`: 不经过代理的 HTTP 客户端
/// - `base`: 陶瓦回环地址
/// - `owned`: 进程是不是本次会话启动的；外部实例只退房间
async fn cleanup(client: &reqwest::Client, base: &str, owned: bool) {
    let path = if owned {
        "/panic?peaceful=true"
    } else {
        "/state/ide"
    };
    let _ = client.get(format!("{base}{path}")).send().await;
    if owned {
        let wait = async {
            while client.get(format!("{base}/meta")).send().await.is_ok() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(3), wait).await;
    }
}

/// 读陶瓦的 lock 文件，取用户已经打开的实例端口
///
/// lock 文件里是大端两字节端口。
///
/// # 返回值
///
/// 端口；没有 lock 文件、内容不是两字节或端口为 0 时返回 `None`
fn existing_port() -> Option<u16> {
    #[cfg(target_os = "macos")]
    let directory = dirs::home_dir()?.join("terracotta");
    #[cfg(not(target_os = "macos"))]
    let directory = std::env::temp_dir().join("terracotta");
    let data = std::fs::read(directory.join("terracotta.lock")).ok()?;
    let port = u16::from_be_bytes(data.as_slice().try_into().ok()?);
    (port != 0).then_some(port)
}

/// 拉公共节点：内置两个，再从节点列表服务补充
///
/// # 返回值
///
/// 去重后的节点地址，形如 `tcp://host:port`；拉取失败只留内置的两个
pub(super) async fn nodes() -> Vec<String> {
    let mut result = vec![
        "tcp://public.easytier.top:11010".into(),
        "tcp://public2.easytier.cn:54321".into(),
    ];
    let client = mml_net::get_work_client();
    for url in [
        "https://etnode.zkitefly.eu.org/node1",
        "https://etnode.zkitefly.eu.org/node2",
    ] {
        if let Ok(Ok(text)) =
            tokio::time::timeout(Duration::from_secs(8), client.get_text_success(url)).await
        {
            for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
                if let Ok(url) = reqwest::Url::parse(line)
                    && matches!(url.scheme(), "tcp" | "udp" | "ws" | "wss")
                    && url.host_str().is_some()
                    && !result.iter().any(|s| s == line)
                {
                    result.push(line.into());
                }
            }
        }
    }
    result
}

/// 把加入方要连的地址转成本地实例的服务器项
///
/// 只接受回环地址，避免大厅把客户端引到别的机器。
///
/// - `address`: 陶瓦给的 `host:port`，没写端口按 25565
///
/// # 返回值
///
/// 服务器项；地址不是 `127.0.0.1` 或端口非法时返回 `err.lobbyState`
pub(super) fn join_server(
    address: &str,
) -> Result<mml_game::launcher::instance_setting_obj::ServerObj, String> {
    let (host, port) = address.split_once(':').unwrap_or((address, "25565"));
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
        .ok_or(err_keys::LOBBY_STATE)?;
    if host != "127.0.0.1" {
        return Err(err_keys::LOBBY_STATE.into());
    }
    Ok(mml_game::launcher::instance_setting_obj::ServerObj {
        enable: true,
        ip: Some(host.into()),
        port: Some(port),
    })
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
