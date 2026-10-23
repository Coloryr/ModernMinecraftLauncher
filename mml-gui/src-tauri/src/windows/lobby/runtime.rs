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
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub(super) struct Session {
    client: reqwest::Client,
    base: String,
    owned: bool,
    room_owned: AtomicBool,
}

#[derive(Deserialize)]
struct PortFile {
    port: u16,
}

#[derive(Deserialize)]
struct Meta {
    version: String,
}

#[derive(Deserialize)]
struct Profile {
    machine_id: String,
    name: String,
    kind: String,
    vendor: String,
}

#[derive(Deserialize)]
struct State {
    state: String,
    index: u64,
    room: Option<String>,
    url: Option<String>,
    difficulty: Option<String>,
    #[serde(rename = "type")]
    exception: Option<u32>,
    #[serde(default)]
    profiles: Vec<Profile>,
}

struct PortGuard(PathBuf);
impl Drop for PortGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(format!("{}.tmp", self.0.display()));
    }
}

impl Session {
    pub async fn start(model: &Arc<LobbyModel>) -> Result<(Arc<Self>, String), String> {
        let executable = super::install::executable(model).await?;
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
                    for _ in 0..80 {
                        if let Ok(data) = std::fs::read(&port_file.0)
                            && let Ok(port) = serde_json::from_slice::<PortFile>(&data)
                        {
                            if port.port != 0 && existing != Some(port.port) {
                                let _ = cleanup_client
                                    .get(format!(
                                        "http://127.0.0.1:{}/panic?peaceful=true",
                                        port.port
                                    ))
                                    .send()
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

    pub async fn action(&self, path: &str, query: &[(String, String)]) -> Result<(), String> {
        let mut url =
            reqwest::Url::parse(&format!("{}{path}", self.base)).map_err(|e| e.to_string())?;
        url.query_pairs_mut().extend_pairs(
            query
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        if path != "/state/ide" {
            self.room_owned.store(true, Ordering::Release);
        }
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        self.room_owned
            .store(path != "/state/ide", Ordering::Release);
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.owned && !self.room_owned.load(Ordering::Acquire) {
            return;
        }
        let client = self.client.clone();
        let url = format!(
            "{}{}",
            self.base,
            if self.owned {
                "/panic?peaceful=true"
            } else {
                "/state/ide"
            }
        );
        tauri::async_runtime::spawn(async move {
            let _ = client.get(url).send().await;
        });
    }
}

fn existing_port() -> Option<u16> {
    #[cfg(target_os = "macos")]
    let directory = dirs::home_dir()?.join("terracotta");
    #[cfg(not(target_os = "macos"))]
    let directory = std::env::temp_dir().join("terracotta");
    let data = std::fs::read(directory.join("terracotta.lock")).ok()?;
    let port = u16::from_be_bytes(data.as_slice().try_into().ok()?);
    (port != 0).then_some(port)
}

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
mod tests {
    use super::*;
    #[test]
    fn room_state_and_local_join_address_follow_protocol() {
        let state: State = serde_json::from_str(r#"{"state":"guest-ok","index":7,"url":"127.0.0.1:30000","profiles":[{"machine_id":"one","name":"player","kind":"LOCAL","vendor":"Terracotta"}]}"#).unwrap();
        assert_eq!(state.index, 7);
        assert_eq!(
            join_server(state.url.as_deref().unwrap()).unwrap().port,
            Some(30000)
        );
        assert_eq!(join_server("127.0.0.1").unwrap().port, Some(25565));
        for address in [
            "example.com",
            "127.0.0.1:0",
            "127.0.0.1:65536",
            "127.0.0.1:22/path",
        ] {
            assert!(join_server(address).is_err());
        }
    }
    #[tokio::test]
    async fn http_adapter_encodes_room_nodes_checks_status_and_closes_owned_worker() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let worker = std::thread::spawn(move || {
            for step in 0..4 {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(10))
                        }
                        Err(error) => panic!("HTTP request {step} missing: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let target = line.split_whitespace().nth(1).unwrap();
                let url = reqwest::Url::parse(&format!("http://localhost{target}")).unwrap();
                let (status, body) = match step {
                    0 => {
                        assert_eq!(url.path(), "/state");
                        (
                            "200 OK",
                            r#"{"state":"guest-ok","index":8,"url":"127.0.0.1:30000","profiles":[]}"#,
                        )
                    }
                    1 => {
                        assert_eq!(url.path(), "/state/guesting");
                        let pairs = url.query_pairs().collect::<Vec<_>>();
                        assert!(pairs.iter().any(|(k, v)| k == "room" && v == "U/TEST+CODE"));
                        assert!(pairs.iter().any(|(k, v)| k == "player" && v == "玩家 name"));
                        assert_eq!(pairs.iter().filter(|(k, _)| k == "public_nodes").count(), 2);
                        ("200 OK", "")
                    }
                    2 => ("400 Bad Request", "invalid room"),
                    _ => {
                        assert_eq!(target, "/panic?peaceful=true");
                        ("200 OK", "")
                    }
                };
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let session = Session {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            base: format!("http://{address}"),
            owned: true,
            room_owned: AtomicBool::new(false),
        };
        assert_eq!(
            session.state().await.unwrap().address.as_deref(),
            Some("127.0.0.1:30000")
        );
        session
            .action(
                "/state/guesting",
                &[
                    ("room".into(), "U/TEST+CODE".into()),
                    ("player".into(), "玩家 name".into()),
                    ("public_nodes".into(), "tcp://one:1234".into()),
                    ("public_nodes".into(), "wss://two/ws".into()),
                ],
            )
            .await
            .unwrap();
        assert!(session.action("/state/guesting", &[]).await.is_err());
        drop(session);
        tokio::task::spawn_blocking(move || worker.join().unwrap())
            .await
            .unwrap();
    }
}
