//! 每个客户端单独存储；差异更新在副本验证并检查配额后提交。

use crate::Config;
use axum::http::HeaderMap;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use mml_cloud::{Error, Manifest, Result, World, archive, revision, sha1};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// HTTP 响应内容。
pub(super) enum Reply {
    /// ColorMC JSON 返回值。
    Json(Value),
    /// 成功下载时的 ZIP。
    Zip(Vec<u8>),
}

/// 一个实例的持久化元数据。
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct Instance {
    /// 是否开放同步。
    enabled: bool,
    /// 显示名。
    name: String,
    /// 最近配置上传时间。
    time: String,
}

/// 已绑定私有数据目录的存储。
pub(super) struct Store {
    /// 数据根目录。
    root: PathBuf,
    /// 阻止两个进程同时写同一数据目录。
    _lock: File,
}

/// 旧副本仍待恢复时保留事务目录。
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        if self.0.join("transaction.json").exists() && self.0.join("old").exists() {
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 替换提交前写入的恢复标记。
#[derive(Serialize, Deserialize)]
struct Transaction {
    /// 相对数据根目录的目标路径。
    target: String,
}

fn remove(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn header(headers: &HeaderMap, key: &str) -> Result<String> {
    Ok(headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Error::new("err.cloudProtocol"))?
        .to_string())
}

fn name(headers: &HeaderMap) -> Result<String> {
    let encoded = format!("name={}", header(headers, "name")?);
    let decoded = url::form_urlencoded::parse(encoded.as_bytes())
        .next()
        .map(|v| v.1.to_string())
        .ok_or_else(|| Error::new("err.cloudPath"))?;
    archive::validate_name(&decoded)?;
    Ok(decoded)
}

fn load<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if !path.try_exists()? && path.with_extension("json.old").exists() {
        fs::rename(path.with_extension("json.old"), path)?;
    }
    if !path.try_exists()? {
        return Ok(T::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    let staging = path.with_extension("json.new");
    fs::write(&staging, bytes)?;
    let backup = path.with_extension("json.old");
    if backup.exists() {
        fs::remove_file(&backup)?;
    }
    if path.exists() {
        fs::rename(path, &backup)?;
    }
    if let Err(err) = fs::rename(&staging, path) {
        if backup.exists() {
            fs::rename(&backup, path)?;
        }
        return Err(err.into());
    }
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    Ok(())
}

impl Store {
    pub(super) fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        archive::disk_usage(root)?;
        let lock = File::options()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(root.join(".lock"))?;
        lock.try_lock().map_err(|_| Error::new("err.cloudBusy"))?;
        fs::create_dir_all(root.join("accounts"))?;
        fs::create_dir_all(root.join("staging"))?;
        let store = Self {
            root: root.to_path_buf(),
            _lock: lock,
        };
        store.recover()?;
        Ok(store)
    }

    fn recover(&self) -> Result<()> {
        for entry in fs::read_dir(self.root.join("staging"))? {
            let path = entry?.path();
            if path.join("transaction.json").exists() {
                let transaction: Transaction =
                    serde_json::from_slice(&fs::read(path.join("transaction.json"))?)?;
                let relative = Self::transaction_path(&transaction.target)?;
                let target = self.root.join(relative);
                let old = path.join("old");
                if !target.exists() && old.exists() {
                    fs::rename(&old, &target)?;
                }
            }
            remove(&path)?;
        }
        Self::recover_metadata(&self.root.join("accounts"))?;
        Ok(())
    }

    fn transaction_path(value: &str) -> Result<PathBuf> {
        let relative = archive::relative_path(value)?;
        let parts: Vec<_> = value.split('/').collect();
        let digest = |s: &str| s.len() == 40 && s.bytes().all(|c| c.is_ascii_hexdigit());
        if parts.len() < 4
            || parts[0] != "accounts"
            || !digest(parts[1])
            || Uuid::parse_str(parts[2]).is_err()
            || !(parts.len() == 4 && parts[3] == "config.zip"
                || parts.len() == 5 && parts[3] == "worlds" && digest(parts[4]))
        {
            return Err(Error::new("err.cloudPath"));
        }
        Ok(relative)
    }

    fn recover_metadata(accounts: &Path) -> Result<()> {
        // 只处理账号下实例这一层，存档内允许出现同名文件。
        for account in fs::read_dir(accounts)? {
            let account = account?;
            if !account.file_type()?.is_dir() {
                continue;
            }
            for instance in fs::read_dir(account.path())? {
                let instance = instance?;
                if !instance.file_type()?.is_dir()
                    || Uuid::parse_str(&instance.file_name().to_string_lossy()).is_err()
                {
                    continue;
                }
                let target = instance.path().join("instance.json");
                let old = target.with_extension("json.old");
                if old.exists() {
                    if !target.exists() {
                        fs::rename(&old, &target)?;
                    } else {
                        fs::remove_file(&old)?;
                    }
                }
                let pending = target.with_extension("json.new");
                if pending.exists() {
                    fs::remove_file(pending)?;
                }
            }
        }
        Ok(())
    }

    fn replace(&self, stage: &Staging, source: &Path, target: &Path) -> Result<()> {
        let transaction = Transaction {
            target: target
                .strip_prefix(&self.root)
                .map_err(|_| Error::new("err.cloudPath"))?
                .to_str()
                .ok_or_else(|| Error::new("err.cloudPath"))?
                .replace('\\', "/"),
        };
        let mut marker = File::create(stage.0.join("transaction.json"))?;
        use std::io::Write;
        marker.write_all(&serde_json::to_vec(&transaction)?)?;
        marker.sync_all()?;
        let old = stage.0.join("old");
        if target.exists() {
            fs::rename(target, &old)?;
        }
        if let Err(error) = fs::rename(source, target) {
            if old.exists() {
                fs::rename(&old, target)?;
            }
            fs::remove_file(stage.0.join("transaction.json"))?;
            return Err(error.into());
        }
        // 新副本已提交，启动恢复会清理未删完的旧目录。
        remove(&old)?;
        fs::remove_file(stage.0.join("transaction.json"))?;
        Ok(())
    }

    fn staging(&self) -> Result<Staging> {
        let path = self.root.join("staging").join(Uuid::new_v4().to_string());
        fs::create_dir_all(&path)?;
        Ok(Staging(path))
    }

    pub(super) fn handle(
        &mut self,
        config: &Config,
        action: &str,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<Reply> {
        let client = header(headers, "clientkey")?;
        let quota = *config
            .clients
            .get(&client)
            .ok_or_else(|| Error::new("err.cloudAuth"))?;
        let account = self.root.join("accounts").join(sha1(client.as_bytes()));
        fs::create_dir_all(&account)?;
        let ok = || Reply::Json(json!({"res":100}));
        match action {
            "check" => return Ok(ok()),
            "getstate" => {
                return Ok(Reply::Json(
                    json!({"res":100,"use":archive::disk_usage(&account)?,"size":quota}),
                ));
            }
            "getlist" => {
                let mut list = Vec::new();
                for entry in fs::read_dir(&account)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir()
                        && Uuid::parse_str(&entry.file_name().to_string_lossy()).is_ok()
                    {
                        let item: Instance = load(&entry.path().join("instance.json"))?;
                        list.push(
                            json!({"UUID":entry.file_name().to_string_lossy(),"Name":item.name}),
                        );
                    }
                }
                return Ok(Reply::Json(json!({"res":100,"list":list})));
            }
            _ => {}
        }
        let uuid = Uuid::parse_str(&header(headers, "uuid")?)
            .map_err(|_| Error::new("err.cloudProtocol"))?;
        let instance = account.join(uuid.to_string());
        let meta_path = instance.join("instance.json");
        let mut meta: Instance = load(&meta_path)?;
        match action {
            "haveuuid" => {
                return Ok(Reply::Json(
                    json!({"res":100,"have":meta.enabled,"time":meta.time}),
                ));
            }
            "startuuid" => {
                meta.enabled = true;
                meta.name = name(headers)?;
                self.check_quota(
                    &account,
                    quota,
                    meta_path.metadata().map(|m| m.len()).unwrap_or(0),
                    serde_json::to_vec(&meta)?.len() as u64,
                )?;
                fs::create_dir_all(&instance)?;
                save(&meta_path, &meta)?;
                return Ok(ok());
            }
            "stopuuid" => {
                if !meta_path.exists() {
                    return Err(Error::new("err.cloudMissing"));
                }
                meta.enabled = false;
                save(&meta_path, &meta)?;
                return Ok(ok());
            }
            _ => {}
        }
        if !meta.enabled {
            return Err(Error::new("err.cloudNotEnabled"));
        }
        if action == "putconfig" {
            let stage = self.staging()?;
            let files = stage.0.join("files");
            archive::apply(&files, body, &stage.0.join("backup"))?;
            let manifest = archive::scan(&files)?;
            let bytes = archive::delta(&files, &manifest, &Manifest::new())?
                .ok_or_else(|| Error::new("err.cloudArchive"))?;
            let target = instance.join("config.zip");
            meta.time = Utc::now().to_rfc3339();
            self.check_quota(
                &account,
                quota,
                target.metadata().map(|m| m.len()).unwrap_or(0)
                    + meta_path.metadata().map(|m| m.len()).unwrap_or(0),
                bytes.len() as u64 + serde_json::to_vec(&meta)?.len() as u64,
            )?;
            fs::write(stage.0.join("config.zip"), bytes)?;
            self.replace(&stage, &stage.0.join("config.zip"), &target)?;
            save(&meta_path, &meta)?;
            return Ok(Reply::Json(json!({"res":100,"time":meta.time})));
        }
        if action == "pullconfig" {
            let target = instance.join("config.zip");
            if !target.is_file() {
                return Err(Error::new("err.cloudMissing"));
            }
            return Ok(Reply::Zip(fs::read(target)?));
        }
        let worlds = instance.join("worlds");
        fs::create_dir_all(&worlds)?;
        if action == "getworldlist" {
            let mut list = Vec::new();
            for entry in fs::read_dir(&worlds)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    list.push(self.world(&entry.path())?);
                }
            }
            list.sort_by(|a, b| a.name.cmp(&b.name));
            return Ok(Reply::Json(json!({"res":100,"list":list})));
        }
        let name = name(headers)?;
        let target = worlds.join(sha1(name.as_bytes()));
        let files = target.join("files");
        match action {
            "getworldfiles" => {
                if !target.is_dir() {
                    return Err(Error::new("err.cloudMissing"));
                }
                Ok(Reply::Json(
                    json!({"res":100,"list":archive::scan(&files)?}),
                ))
            }
            "putworld" => {
                let current = archive::scan(&files)?;
                if let Some(expected) = headers.get("if-match").and_then(|v| v.to_str().ok())
                    && expected != revision(&current)
                {
                    return Err(Error::new("err.cloudConflict"));
                }
                let stage = self.staging()?;
                let new_world = stage.0.join("world");
                archive::copy_tree(&files, &new_world.join("files"))?;
                archive::apply(&new_world.join("files"), body, &stage.0.join("backup"))?;
                let info = World {
                    name,
                    time: Utc::now().to_rfc3339(),
                    icon: String::new(),
                    files: Manifest::new(),
                };
                fs::write(new_world.join("world.json"), serde_json::to_vec(&info)?)?;
                self.check_quota(
                    &account,
                    quota,
                    archive::disk_usage(&target)?,
                    archive::disk_usage(&new_world)?,
                )?;
                self.replace(&stage, &new_world, &target)?;
                Ok(ok())
            }
            "pullworld" => {
                if !target.is_dir() {
                    return Err(Error::new("err.cloudMissing"));
                }
                let local: Manifest = serde_json::from_slice(body)?;
                let current = archive::scan(&files)?;
                match archive::delta(&files, &current, &local)? {
                    Some(bytes) => Ok(Reply::Zip(bytes)),
                    None => Ok(Reply::Json(json!({"res":103}))),
                }
            }
            "deleteworld" => {
                if !target.is_dir() {
                    return Err(Error::new("err.cloudMissing"));
                }
                fs::remove_dir_all(target)?;
                Ok(ok())
            }
            _ => Err(Error::new("err.cloudProtocol")),
        }
    }

    fn check_quota(&self, account: &Path, quota: u64, old: u64, new: u64) -> Result<()> {
        if archive::disk_usage(account)?
            .saturating_sub(old)
            .saturating_add(new)
            > quota
        {
            return Err(Error::new("err.cloudQuota"));
        }
        Ok(())
    }

    fn world(&self, path: &Path) -> Result<World> {
        let mut world: World = serde_json::from_slice(&fs::read(path.join("world.json"))?)?;
        world.files = archive::scan(&path.join("files"))?;
        let icon = path.join("files/icon.png");
        if icon.metadata().is_ok_and(|m| m.len() <= 1024 * 1024) {
            world.icon = STANDARD.encode(fs::read(icon)?);
        }
        Ok(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        mml_testutil::temp_path(format!("cloud-recovery-{}", Uuid::new_v4()))
    }

    #[test]
    fn interrupted_replacement_restores_old_snapshot() {
        let root = root();
        let store = Store::new(&root).unwrap();
        let relative = format!(
            "accounts/{}/{}/worlds/{}",
            sha1(b"client"),
            Uuid::new_v4(),
            sha1(b"world")
        );
        let target = root.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let stage = store.staging().unwrap();
        fs::create_dir_all(stage.0.join("old/files")).unwrap();
        fs::write(stage.0.join("old/files/level.dat"), b"old").unwrap();
        fs::write(
            stage.0.join("transaction.json"),
            serde_json::to_vec(&Transaction { target: relative }).unwrap(),
        )
        .unwrap();
        let stage_path = stage.0.clone();
        drop(stage);
        assert!(
            stage_path.exists(),
            "retain the only old snapshot until recovery"
        );
        drop(store);
        let _store = Store::new(&root).unwrap();
        assert_eq!(fs::read(target.join("files/level.dat")).unwrap(), b"old");
        assert!(!stage_path.exists());
    }

    #[test]
    fn committed_replacement_keeps_new_snapshot() {
        let root = root();
        let store = Store::new(&root).unwrap();
        let relative = format!("accounts/{}/{}/config.zip", sha1(b"client"), Uuid::new_v4());
        let target = root.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"new").unwrap();
        let stage = store.staging().unwrap();
        fs::write(stage.0.join("old"), b"old").unwrap();
        fs::write(
            stage.0.join("transaction.json"),
            serde_json::to_vec(&Transaction { target: relative }).unwrap(),
        )
        .unwrap();
        let stage_path = stage.0.clone();
        drop(stage);
        drop(store);
        let _store = Store::new(&root).unwrap();
        assert_eq!(fs::read(target).unwrap(), b"new");
        assert!(!stage_path.exists());
    }

    #[test]
    fn metadata_recovery_never_changes_world_content() {
        let root = root();
        let store = Store::new(&root).unwrap();
        let instance = root
            .join("accounts")
            .join(sha1(b"client"))
            .join(Uuid::new_v4().to_string());
        let world = instance.join("worlds").join(sha1(b"world")).join("files");
        fs::create_dir_all(&world).unwrap();
        fs::write(instance.join("instance.json.old"), br#"{"Enabled":true}"#).unwrap();
        fs::write(instance.join("instance.json.new"), b"incomplete").unwrap();
        fs::write(world.join("instance.json.old"), b"world old").unwrap();
        fs::write(world.join("instance.json.new"), b"world new").unwrap();
        drop(store);
        let _store = Store::new(&root).unwrap();
        let meta: Instance = load(&instance.join("instance.json")).unwrap();
        assert!(meta.enabled);
        assert!(!instance.join("instance.json.new").exists());
        assert_eq!(
            fs::read(world.join("instance.json.old")).unwrap(),
            b"world old"
        );
        assert_eq!(
            fs::read(world.join("instance.json.new")).unwrap(),
            b"world new"
        );
    }

    #[test]
    fn recovery_marker_only_accepts_snapshot_targets() {
        let prefix = format!("accounts/{}/{}", sha1(b"client"), Uuid::new_v4());
        for value in [
            "../outside".to_string(),
            format!("{prefix}/instance.json"),
            format!("{prefix}/worlds/{}/files", sha1(b"world")),
            "accounts/config.zip".into(),
        ] {
            assert!(Store::transaction_path(&value).is_err(), "{value}");
        }
    }
}
