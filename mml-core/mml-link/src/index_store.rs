//! 下载索引的存储：fjall 的三个键空间
//!
//! - 主索引 `类型\0项目 ID\0SHA1` → 记录
//! - 哈希反查 `类型\0SHA1` → 项目 ID：查"公共目录里有没有这个文件"是点查，不再整表扫
//! - 项目最新版本 `类型\0项目 ID` → SHA1：取某个项目最近一次记录的版本
//!
//! 有了这三份键，内存里那层 `HashMap` 就不需要了：读直接查库，写由批次合并。
//! 旧的 JSON 索引整份重写，写到一半崩掉就是半截文件；LSM 引擎是追加写，崩溃不会留下坏数据。

use std::sync::Mutex;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};
use serde_json::Value;
use std::path::Path;

/// 主索引键空间
const INDEX: &str = "download_index";
/// 哈希反查键空间
const HASH_INDEX: &str = "download_hash";
/// 项目最新版本键空间
const LATEST: &str = "download_latest";
/// 反查键是否已经补齐的标记
const READY_KEY: &[u8] = b"meta\0ready";

/// 攒够这么多条写一次
const BATCH: usize = 256;

/// 公共下载索引里的一类资源
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexKind {
    /// 模组
    Mod,
    /// 资源包
    ResourcePack,
}

impl IndexKind {
    /// 落库时写进键里的类型标识
    fn id(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::ResourcePack => "resourcepack",
        }
    }
}

/// 等待写入的一条记录
struct PendingRow {
    /// 资源类型
    kind: IndexKind,
    /// 项目 ID
    pid: String,
    /// 文件 SHA1，小写
    hash: String,
    /// 记录字节
    payload: Vec<u8>,
}

/// 下载索引库
pub struct Store {
    /// 数据库句柄
    db: Database,
    /// 主索引
    index: Keyspace,
    /// 哈希反查
    hashes: Keyspace,
    /// 项目最新版本
    latest: Keyspace,
    /// 还没写进库的记录
    pending: Mutex<Vec<PendingRow>>,
}

impl Store {
    /// 打开（或新建）索引库
    ///
    /// - `path`: 库目录
    ///
    /// # 返回值
    ///
    /// 打不开时返回错误描述
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Database::builder(path).open().map_err(|err| err.to_string())?;
        let index = db
            .keyspace(INDEX, KeyspaceCreateOptions::default)
            .map_err(|err| err.to_string())?;
        let hashes = db
            .keyspace(HASH_INDEX, KeyspaceCreateOptions::default)
            .map_err(|err| err.to_string())?;
        let latest = db
            .keyspace(LATEST, KeyspaceCreateOptions::default)
            .map_err(|err| err.to_string())?;
        let store = Self {
            db,
            index,
            hashes,
            latest,
            pending: Mutex::new(Vec::new()),
        };
        store.ensure_secondary()?;
        Ok(store)
    }

    /// 读出某一类资源的全部记录
    ///
    /// - `kind`: 资源类型
    ///
    /// # 返回值
    ///
    /// `(项目 ID, 记录字节)` 列表
    pub fn load(&self, kind: IndexKind) -> Result<Vec<(String, Vec<u8>)>, String> {
        self.commit_pending()?;
        let mut out = Vec::new();
        for guard in self.index.prefix(prefix(kind)) {
            let (key, value) = guard.into_inner().map_err(|err| err.to_string())?;
            let (_, pid, _) = decode(&key).ok_or_else(|| String::from("bad index key"))?;
            out.push((pid, value.to_vec()));
        }
        Ok(out)
    }

    /// 读出某一类资源已有的键
    ///
    /// 升级中途停下后再跑时用来跳过已导入的记录
    ///
    /// - `kind`: 资源类型
    ///
    /// # 返回值
    ///
    /// `(项目 ID, 文件 SHA1)` 列表
    pub fn keys(&self, kind: IndexKind) -> Result<Vec<(String, String)>, String> {
        self.commit_pending()?;
        let mut out = Vec::new();
        for guard in self.index.prefix(prefix(kind)) {
            let key = guard.key().map_err(|err| err.to_string())?;
            let (_, pid, hash) = decode(&key).ok_or_else(|| String::from("bad index key"))?;
            out.push((pid, hash));
        }
        Ok(out)
    }

    /// 按文件 SHA1 查记录
    ///
    /// - `kind`: 资源类型
    /// - `hash`: 文件 SHA1，小写
    ///
    /// # 返回值
    ///
    /// `(项目 ID, 记录字节)`；没有这个文件时返回 `None`
    pub fn find(&self, kind: IndexKind, hash: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        self.commit_pending()?;
        let Some(pid) = self
            .hashes
            .get(hash_key(kind.id(), hash))
            .map_err(|err| err.to_string())?
        else {
            return Ok(None);
        };
        let pid = String::from_utf8_lossy(&pid).to_string();
        let payload = self
            .index
            .get(key(&kind, &pid, hash))
            .map_err(|err| err.to_string())?;
        Ok(payload.map(|payload| (pid, payload.to_vec())))
    }

    /// 取某个项目最近一次写入的记录
    ///
    /// - `kind`: 资源类型
    /// - `pid`: 项目 ID
    ///
    /// # 返回值
    ///
    /// `(文件 SHA1, 记录字节)`；没有记录时返回 `None`
    pub fn latest_of(&self, kind: IndexKind, pid: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        self.commit_pending()?;
        let Some(hash) = self
            .latest
            .get(pid_key(kind.id(), pid))
            .map_err(|err| err.to_string())?
        else {
            return Ok(None);
        };
        let hash = String::from_utf8_lossy(&hash).to_string();
        let payload = self
            .index
            .get(key(&kind, pid, &hash))
            .map_err(|err| err.to_string())?;
        Ok(payload.map(|payload| (hash, payload.to_vec())))
    }

    /// 写入一条记录，键相同的覆盖
    ///
    /// 先进批次，攒够 [`BATCH`] 条才写一次；要保证落盘见 [`Store::flush`]
    ///
    /// - `kind`: 资源类型
    /// - `pid`: 项目 ID
    /// - `hash`: 文件 SHA1，小写
    /// - `payload`: 记录字节
    ///
    /// # 返回值
    ///
    /// 写入失败时返回错误描述
    pub fn put(&self, kind: IndexKind, pid: &str, hash: &str, payload: &[u8]) -> Result<(), String> {
        let full = {
            let mut pending = self.pending.lock().unwrap();
            pending.push(PendingRow {
                kind,
                pid: pid.to_string(),
                hash: hash.to_string(),
                payload: payload.to_vec(),
            });
            pending.len() >= BATCH
        };
        if full {
            self.commit_pending()?;
        }
        Ok(())
    }

    /// 把待写批次写进库，并把日志刷到磁盘
    ///
    /// 用 [`Store::put`] 写进来的记录要等这里才持久
    ///
    /// # 返回值
    ///
    /// 写入或刷盘失败时返回错误描述
    pub fn flush(&self) -> Result<(), String> {
        self.commit_pending()?;
        self.db
            .persist(PersistMode::SyncAll)
            .map_err(|err| err.to_string())
    }

    /// 把索引里指向旧文件名的记录改成新文件名
    ///
    /// - `kind`: 资源类型
    /// - `old`: 旧公共文件名
    /// - `new`: 新公共文件名
    ///
    /// # 返回值
    ///
    /// 改写失败时返回错误描述
    pub fn rename_file(&self, kind: IndexKind, old: &str, new: &str) -> Result<(), String> {
        self.commit_pending()?;
        let mut batch = self.db.batch();
        let mut changed = 0usize;
        for guard in self.index.prefix(prefix(kind)) {
            let (key, value) = guard.into_inner().map_err(|err| err.to_string())?;
            let mut record: Value =
                serde_json::from_slice(&value).map_err(|err| err.to_string())?;
            if record.get("file").and_then(Value::as_str) != Some(old) {
                continue;
            }
            record["file"] = Value::String(new.to_string());
            let payload = serde_json::to_vec(&record).map_err(|err| err.to_string())?;
            batch.insert(&self.index, key.to_vec(), payload);
            changed += 1;
        }
        if changed > 0 {
            batch.commit().map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    /// 补齐旧库缺的反查键
    ///
    /// 上一版只有主索引；打开时按主索引重建反查与最新版本指针，补过一次就写标记
    ///
    /// # 返回值
    ///
    /// 重建失败时返回错误描述
    fn ensure_secondary(&self) -> Result<(), String> {
        if self
            .hashes
            .get(READY_KEY)
            .map_err(|err| err.to_string())?
            .is_some()
        {
            return Ok(());
        }
        let mut batch = self.db.batch();
        for guard in self.index.prefix([]) {
            let (key, _) = guard.into_inner().map_err(|err| err.to_string())?;
            let Some((kind, pid, hash)) = decode(&key) else {
                continue;
            };
            batch.insert(&self.hashes, hash_key(&kind, &hash), pid.as_bytes());
            batch.insert(&self.latest, pid_key(&kind, &pid), hash.as_bytes());
        }
        batch.insert(&self.hashes, READY_KEY, b"1".as_slice());
        batch.commit().map_err(|err| err.to_string())
    }

    /// 把批次里攒下的记录写进库
    ///
    /// # 返回值
    ///
    /// 写入失败时返回错误描述
    fn commit_pending(&self) -> Result<(), String> {
        let mut pending = self.pending.lock().unwrap();
        if pending.is_empty() {
            return Ok(());
        }
        let mut batch = self.db.batch();
        for row in pending.iter() {
            let id = row.kind.id();
            batch.insert(&self.index, key(&row.kind, &row.pid, &row.hash), row.payload.as_slice());
            batch.insert(&self.hashes, hash_key(id, &row.hash), row.pid.as_bytes());
            batch.insert(&self.latest, pid_key(id, &row.pid), row.hash.as_bytes());
        }
        batch.commit().map_err(|err| err.to_string())?;
        pending.clear();
        Ok(())
    }
}

/// 主索引键：`类型 \0 项目 ID \0 SHA1`
fn key(kind: &IndexKind, pid: &str, hash: &str) -> Vec<u8> {
    let id = kind.id();
    let mut key = Vec::with_capacity(id.len() + pid.len() + hash.len() + 2);
    key.extend_from_slice(id.as_bytes());
    key.push(0);
    key.extend_from_slice(pid.as_bytes());
    key.push(0);
    key.extend_from_slice(hash.as_bytes());
    key
}

/// 某一类资源的主索引前缀
fn prefix(kind: IndexKind) -> Vec<u8> {
    let mut prefix = kind.id().as_bytes().to_vec();
    prefix.push(0);
    prefix
}

/// 哈希反查键：`类型 \0 SHA1`
fn hash_key(kind: &str, hash: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(kind.len() + hash.len() + 1);
    key.extend_from_slice(kind.as_bytes());
    key.push(0);
    key.extend_from_slice(hash.as_bytes());
    key
}

/// 项目最新版本键：`类型 \0 项目 ID`
fn pid_key(kind: &str, pid: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(kind.len() + pid.len() + 1);
    key.extend_from_slice(kind.as_bytes());
    key.push(0);
    key.extend_from_slice(pid.as_bytes());
    key
}

/// 拆主索引键
fn decode(key: &[u8]) -> Option<(String, String, String)> {
    let mut parts = key.split(|byte| *byte == 0);
    let kind = String::from_utf8(parts.next()?.to_vec()).ok()?;
    let pid = String::from_utf8(parts.next()?.to_vec()).ok()?;
    let hash = String::from_utf8(parts.next()?.to_vec()).ok()?;
    Some((kind, pid, hash))
}
