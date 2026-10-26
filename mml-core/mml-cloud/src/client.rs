//! HTTP 协议不把密钥放在 URL；所有响应均有大小和时间限制。

use crate::{Error, MAX_ARCHIVE, Manifest, Result, World, archive, revision};
use reqwest::{Client as HttpClient, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, time::Duration};
use url::Url;

/// 客户端连接设置，由启动器持久化。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Settings {
    /// 云同步 HTTP 根地址。
    pub server: String,
    /// 服务器共享密钥。
    pub server_key: String,
    /// 客户端独立密钥，必须在服务器授权。
    pub client_key: String,
}

/// 某个云端实例的启用状态与配置时间。
#[derive(Clone, Debug, Deserialize)]
pub struct InstanceState {
    /// 是否启用。
    pub have: bool,
    /// 配置更新时间，UTC。
    #[serde(default)]
    pub time: String,
}

/// 客户端的云端配额。
#[derive(Clone, Debug, Deserialize)]
pub struct Storage {
    /// 已用字节。
    #[serde(rename = "use")]
    pub used: u64,
    /// 总配额，单位字节。
    pub size: u64,
}

/// 含超时与鉴权头的云同步客户端。
pub struct Client {
    /// 规范化后的服务器设置。
    settings: Settings,
    /// 复用连接池。
    http: HttpClient,
}

impl Client {
    /// 校验连接参数并创建客户端。
    ///
    /// - `settings`: 服务器地址与密钥
    ///
    /// # 返回值
    ///
    /// 可发起请求的客户端，非法参数返回配置错误。
    pub fn new(mut settings: Settings) -> Result<Self> {
        settings.server = settings.server.trim().trim_end_matches('/').to_string();
        let url = Url::parse(&settings.server).map_err(|_| Error::new("err.cloudSettings"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || settings.server_key.is_empty()
            || settings.client_key.is_empty()
        {
            return Err(Error::new("err.cloudSettings"));
        }
        Ok(Self {
            settings,
            http: HttpClient::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(300))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }

    fn request(&self, action: &str, uuid: &str, name: &str) -> RequestBuilder {
        let name: String = url::form_urlencoded::byte_serialize(name.as_bytes()).collect();
        self.http
            .post(format!("{}/{}", self.settings.server, action))
            .header("ColorMC", "MML/0.1")
            .header("serverkey", &self.settings.server_key)
            .header("clientkey", &self.settings.client_key)
            .header("uuid", uuid)
            .header("name", name)
    }

    async fn response(request: RequestBuilder) -> Result<Vec<u8>> {
        let mut response = request.send().await?;
        if response
            .content_length()
            .is_some_and(|n| n > MAX_ARCHIVE as u64)
        {
            return Err(Error::new("err.cloudLimit"));
        }
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len().saturating_add(chunk.len()) > MAX_ARCHIVE {
                return Err(Error::new("err.cloudLimit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            let code = value["res"]
                .as_i64()
                .ok_or_else(|| Error::new("err.cloudProtocol"))?;
            if code != 100 && code != 103 {
                return Err(Error::protocol(code));
            }
        } else if !status.is_success() {
            return Err(Error::new("err.cloudNetwork"));
        }
        Ok(bytes)
    }

    async fn json(&self, action: &str, uuid: &str, name: &str) -> Result<Value> {
        Ok(serde_json::from_slice(
            &Self::response(self.request(action, uuid, name)).await?,
        )?)
    }

    /// 校验密钥并读取配额。
    ///
    /// # 返回值
    ///
    /// 当前客户端的存储用量。
    pub async fn storage(&self) -> Result<Storage> {
        self.json("check", "", "").await?;
        Ok(serde_json::from_value(
            self.json("getstate", "", "").await?,
        )?)
    }

    /// 查询实例启用状态。
    ///
    /// - `uuid`: 云端实例 UUID
    ///
    /// # 返回值
    ///
    /// 启用状态与配置更新时间。
    pub async fn state(&self, uuid: &str) -> Result<InstanceState> {
        Ok(serde_json::from_value(
            self.json("haveuuid", uuid, "").await?,
        )?)
    }

    /// 开启或暂停实例同步，暂停时保留云端数据。
    ///
    /// - `uuid`: 云端实例 UUID
    /// - `name`: 实例显示名
    /// - `enabled`: 是否启用
    ///
    /// # 返回值
    ///
    /// 请求成功时返回空结果。
    pub async fn set_enabled(&self, uuid: &str, name: &str, enabled: bool) -> Result<()> {
        self.json(if enabled { "startuuid" } else { "stopuuid" }, uuid, name)
            .await?;
        Ok(())
    }

    /// 获取云端存档列表。
    ///
    /// - `uuid`: 云端实例 UUID
    ///
    /// # 返回值
    ///
    /// 按目录名排序的存档元数据。
    pub async fn worlds(&self, uuid: &str) -> Result<Vec<World>> {
        let value = self.json("getworldlist", uuid, "").await?;
        Ok(serde_json::from_value(value["list"].clone())?)
    }

    /// 上传一个存档的新增、修改和删除文件。
    ///
    /// - `uuid`: 云端实例 UUID
    /// - `name`: 存档目录名
    /// - `root`: 本地存档目录
    ///
    /// # 返回值
    ///
    /// true 表示传输了差异，false 表示内容相同。
    pub async fn upload(&self, uuid: &str, name: &str, root: &Path) -> Result<bool> {
        archive::validate_name(name)?;
        if !root.is_dir() {
            return Err(Error::new("err.cloudMissing"));
        }
        let remote = match self.json("getworldfiles", uuid, name).await {
            Ok(value) => serde_json::from_value::<Manifest>(value["list"].clone())?,
            Err(err) if err.key == "err.cloudMissing" => Manifest::new(),
            Err(err) => return Err(err),
        };
        let local = archive::scan(root)?;
        let Some(bytes) = archive::delta(root, &local, &remote)? else {
            return Ok(false);
        };
        Self::response(
            self.request("putworld", uuid, name)
                .header("if-match", revision(&remote))
                .header("content-type", "application/zip")
                .body(bytes),
        )
        .await?;
        Ok(true)
    }

    /// 下载差异包并在本地事务替换，旧存档保留在备份目录。
    ///
    /// - `uuid`: 云端实例 UUID
    /// - `name`: 存档目录名
    /// - `root`: 本地存档目录
    /// - `backups`: 原存档的备份父目录
    ///
    /// # 返回值
    ///
    /// true 表示应用了差异，false 表示内容相同。
    pub async fn download(
        &self,
        uuid: &str,
        name: &str,
        root: &Path,
        backups: &Path,
    ) -> Result<bool> {
        archive::validate_name(name)?;
        let local = archive::scan(root)?;
        let bytes = Self::response(self.request("pullworld", uuid, name).json(&local)).await?;
        if serde_json::from_slice::<Value>(&bytes).is_ok() {
            return Ok(false);
        }
        // 网络等待期间本地有修改时拒绝覆盖。
        if archive::scan(root)? != local {
            return Err(Error::new("err.cloudConflict"));
        }
        archive::apply(root, &bytes, backups)?;
        Ok(true)
    }

    /// 删除一个云端存档。
    ///
    /// - `uuid`: 云端实例 UUID
    /// - `name`: 存档目录名
    ///
    /// # 返回值
    ///
    /// 删除成功时返回空结果。
    pub async fn delete(&self, uuid: &str, name: &str) -> Result<()> {
        archive::validate_name(name)?;
        self.json("deleteworld", uuid, name).await?;
        Ok(())
    }
}
