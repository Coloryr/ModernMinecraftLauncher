//! 陶瓦官方 v0.4.2 发布包；固定摘要避免下载到错误平台或未验证的程序。

use super::{model::LobbyModel, runtime::WorkerLease};
use crate::err_keys;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

/// 固定使用的陶瓦版本，与下面的摘要一一对应
const VERSION: &str = "0.4.2";

/// 一个平台的发布包
struct Asset {
    /// 发布包平台名，如 `windows-x86_64`
    platform: &'static str,
    /// 发布包的 SHA-256
    digest: &'static str,
}

/// 当前平台对应的发布包与摘要
///
/// # 返回值
///
/// 见 [`Asset`]；平台没有预置包时返回 `err.lobbyPlatform`
fn asset() -> Result<Asset, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok(Asset {
            platform: "windows-x86_64",
            digest: "07ebe139e3ca5f74576e58b1a96efe59abdfbe148d3f1a49bfdca8b6f70745f0",
        }),
        ("windows", "aarch64") => Ok(Asset {
            platform: "windows-arm64",
            digest: "acfab0a87a02dedc6dab7c05303186c8907f56f815548b693fb3324358da7d14",
        }),
        ("linux", "x86_64") => Ok(Asset {
            platform: "linux-x86_64",
            digest: "675c4fd6c74d49ed8165151ba2be5b6582e0af20fb6d912074543c2484b1e10a",
        }),
        ("linux", "aarch64") => Ok(Asset {
            platform: "linux-arm64",
            digest: "845285ff264ac5fbc16db1a1605ad190e7fa64196516068cc309de5a1d2bf66d",
        }),
        ("macos", "x86_64") => Ok(Asset {
            platform: "macos-x86_64",
            digest: "16306157d89423ce79fa901cdb75a6386ec1a9b1bd43a5d47c2c47cf01a16b86",
        }),
        ("macos", "aarch64") => Ok(Asset {
            platform: "macos-arm64",
            digest: "13de7f9ce8733971b23493fabbe7e16d480f1e0d16a6265b4861f5a01bbecb60",
        }),
        _ => Err(err_keys::LOBBY_PLATFORM.into()),
    }
}

/// 确保陶瓦可执行文件就位且摘要匹配，没有就下载并解包
///
/// - `model`: 大厅模型，下载进度写进它的视图
/// - `worker`: 进程租约，解包线程活到它结束才释放
///
/// # 返回值
///
/// 可执行文件路径；平台不支持、下载、校验或解包失败返回错误
pub(super) async fn executable(
    model: &Arc<LobbyModel>,
    worker: Arc<WorkerLease>,
) -> Result<PathBuf, String> {
    let Asset { platform, digest } = asset()?;
    let folder = crate::gui_config::dir()
        .ok_or(err_keys::LOBBY_CLOSED)?
        .join("terracotta")
        .join(format!("{VERSION}-{platform}"));
    let name = format!(
        "terracotta-{VERSION}-{platform}{}",
        if cfg!(windows) { ".exe" } else { "" }
    );
    let exe = folder.join(&name);
    if exe.is_file()
        && std::fs::read_to_string(folder.join("verified.sha256"))
            .ok()
            .as_deref()
            == Some(digest)
    {
        return Ok(exe);
    }
    {
        let mut view = model.view.lock().unwrap();
        view.downloading = true;
        view.download_percent = 0;
    }
    let cancel = model.cancel.child_token();
    let _cancel_on_drop = cancel.clone().drop_guard();
    let work = async {
        let url = format!(
            "https://github.com/burningtnt/Terracotta/releases/download/v{VERSION}/terracotta-{VERSION}-{platform}-pkg.tar.gz"
        );
        let client = mml_net::get_work_client();
        let mut response = client
            .get(&url)
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let total = response.content_length().unwrap_or(0);
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if bytes.len() + chunk.len() > 128 * 1024 * 1024 {
                return Err(err_keys::LOBBY_DOWNLOAD.into());
            }
            bytes.extend_from_slice(&chunk);
            if total > 0 {
                model.view.lock().unwrap().download_percent =
                    ((bytes.len() as u64 * 100 / total).min(100)) as u32;
            }
        }
        if Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            != digest
        {
            return Err(err_keys::LOBBY_DOWNLOAD.into());
        }
        let folder = folder.clone();
        let token = cancel.clone();
        tauri::async_runtime::spawn_blocking(move || {
            // 取消 HTTP 等待不会停止阻塞解包，目录仍须由旧任务独占。
            let _worker = worker;
            unpack(&bytes, &folder, &name, digest, &token)
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok(exe)
    };
    let result = tokio::select! {
        _ = model.cancel.cancelled() => Err(err_keys::LOBBY_CLOSED.into()),
        result = tokio::time::timeout(std::time::Duration::from_secs(180), work) => result.unwrap_or_else(|_| Err(err_keys::LOBBY_DOWNLOAD.to_string())),
    };
    model.view.lock().unwrap().downloading = false;
    result
}

/// 解包发布包，拒绝链接、越界条目与超限文件
///
/// - `bytes`: 已校验摘要的 tar.gz 内容
/// - `folder`: 安装目录
/// - `name`: 可执行文件名
/// - `digest`: 已校验的摘要，最后写进 `verified.sha256`
/// - `token`: 取消令牌，逐条检查
///
/// # 返回值
///
/// 解包并标记完成返回 `Ok(())`；取消、条目非法或缺少可执行文件返回错误
fn unpack(
    bytes: &[u8],
    folder: &Path,
    name: &str,
    digest: &str,
    token: &tokio_util::sync::CancellationToken,
) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    for entry in archive.entries().map_err(|e| e.to_string())? {
        if token.is_cancelled() {
            return Err(err_keys::LOBBY_CLOSED.into());
        }
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if !matches!(
            path.components().collect::<Vec<_>>().as_slice(),
            [std::path::Component::Normal(_)]
        ) || !entry.header().entry_type().is_file()
            || entry.size() > 128 * 1024 * 1024
        {
            return Err(err_keys::LOBBY_DOWNLOAD.into());
        }
        // 发布包只安装程序及其同目录运行库，完整包摘要已在解包前核对。
        let target = folder.join(path);
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        std::fs::write(&target, data).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
    }
    if token.is_cancelled() {
        return Err(err_keys::LOBBY_CLOSED.into());
    }
    if !folder.join(name).is_file() {
        return Err(err_keys::LOBBY_DOWNLOAD.into());
    }
    std::fs::write(folder.join("verified.sha256"), digest).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use tokio_util::sync::CancellationToken;

    /// 造一个只含单文件的 tar.gz，`link` 决定那一条是不是符号链接
    fn fixture(link: bool) -> Vec<u8> {
        let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        header.set_size(if link { 0 } else { 7 });
        if link {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("../outside").unwrap();
        }
        header.set_cksum();
        tar.append_data(
            &mut header,
            "terracotta-test.exe",
            if link { &b""[..] } else { &b"program"[..] },
        )
        .unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn installer_rejects_links_and_cancelled_sessions_before_marking_verified() {
        let root = crate::test_support::temp_dir("terracotta-installer");
        let token = CancellationToken::new();
        assert!(
            unpack(
                &fixture(true),
                &root.join("link"),
                "terracotta-test.exe",
                "digest",
                &token
            )
            .is_err()
        );
        assert!(!root.join("link/verified.sha256").exists());
        token.cancel();
        assert!(
            unpack(
                &fixture(false),
                &root.join("cancelled"),
                "terracotta-test.exe",
                "digest",
                &token
            )
            .is_err()
        );
        assert!(!root.join("cancelled/verified.sha256").exists());
        unpack(
            &fixture(false),
            &root.join("valid"),
            "terracotta-test.exe",
            "digest",
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(root.join("valid/terracotta-test.exe")).unwrap(),
            b"program"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("valid/verified.sha256")).unwrap(),
            "digest"
        );
        crate::test_support::cleanup(&[&root]);
    }
}
