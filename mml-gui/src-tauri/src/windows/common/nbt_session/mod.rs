//! 打开着的 NBT 文件（对外接口）
//!
//! 命令侧只经 `open_with` / `update_with` 读写；会话表在 `session`，区块字节在 `chunk`。

use std::borrow::Cow;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use mml_base::hash_helper::{self, HashType};
use mml_nbt::nbt_file::NbtFile;
use mml_sys::path_helper;
use uuid::Uuid;

use crate::err_keys;

mod chunk;
mod session;

pub(crate) use self::chunk::chunk_bytes;
use self::chunk::write_chunk_back;
use self::session::{NbtSession, SESSIONS, key_of, stamp_of};
pub(crate) use self::session::{close, close_all_of};

/// 打开会话的产物
pub(crate) struct OpenedNbt<T> {
    /// 回调把 NBT 转成的结果
    pub(crate) value: T,
    /// 打开时那份内容的 SHA1，整份文件或区块负载
    pub(crate) sha1: String,
}

/// 打开一个 NBT 文件并收进会话；已经在会话里就直接用会话里那份，不碰磁盘
///
/// - `owner`: 打开它的实例 uuid，关窗时按它清理
/// - `path`: 文件绝对路径
/// - `chunk`: 文件是区域文件时的区块下标，整份文件为 `None`
/// - `use_nbt`: 拿到内容后的回调，用来把它转成前端要的树
///
/// # 返回值
///
/// 见 [`OpenedNbt`]；读不开或解析不了时返回错误文案
pub(crate) fn open_with<T>(
    owner: &Uuid,
    path: &Path,
    chunk: Option<u32>,
    use_nbt: impl FnOnce(&NbtFile) -> T,
) -> Result<OpenedNbt<T>, String> {
    let key = key_of(path, chunk);
    if let Some(session) = SESSIONS.lock().unwrap().get(&key) {
        return Ok(OpenedNbt {
            value: use_nbt(&session.nbt),
            sha1: session.sha1.clone(),
        });
    }

    // 读写打开失败就退回只读：只读文件照样看得成，改的时候再报错
    let (mut file, writable) = match path_helper::open_read_write(path) {
        Ok(file) => (file, true),
        Err(_) => (
            path_helper::open_read(path).map_err(|err| err.to_string())?,
            false,
        ),
    };

    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|err| err.to_string())?;

    // 区块会话只拿这一块的字节：SHA1 与解析都只针对它，整份区域文件不参与
    let payload = match chunk {
        Some(index) => Cow::Owned(chunk_bytes(&data, index)?),
        None => Cow::Borrowed(data.as_slice()),
    };
    let mut cursor = Cursor::new(payload.as_ref());
    let nbt = NbtFile::read(&mut cursor).map_err(|err| err.to_string())?;

    let value = use_nbt(&nbt);
    let sha1 = hash_helper::gen_hash(HashType::Sha1, payload.as_ref());
    let session = NbtSession {
        owner: *owner,
        stamp: stamp_of(&file)?,
        writable,
        sha1: sha1.clone(),
        file,
        nbt,
    };
    SESSIONS.lock().unwrap().insert(key, session);

    Ok(OpenedNbt { value, sha1 })
}

/// 在已打开的会话上套改动并就地写回
///
/// 写之前两道核对：前端那份基准 SHA1 得与打开的这份一致，文件的状态戳也没被人动过
/// （长度或修改时间变了说明别处写过，内存里这棵树已经过期）。
///
/// - `path`: 文件绝对路径
/// - `chunk`: 文件是区域文件时的区块下标，整份文件为 `None`
/// - `base_sha1`: 前端手上那一份的 SHA1
/// - `apply`: 套改动的回调
///
/// # 返回值
///
/// 写回后内容的 SHA1，整份文件或区块负载；会话不在、基准对不上、文件被别处改过返回
/// `err.fileChanged`，只读文件返回 `err.fileReadOnly`，套改动或写盘失败返回各自的错误文案
pub(crate) fn update_with(
    path: &Path,
    chunk: Option<u32>,
    base_sha1: &str,
    apply: impl FnOnce(&mut NbtFile) -> Result<(), String>,
) -> Result<String, String> {
    let key = key_of(path, chunk);
    let mut sessions = SESSIONS.lock().unwrap();
    let session = sessions
        .get_mut(&key)
        .ok_or_else(|| String::from(err_keys::FILE_CHANGED))?;

    if session.sha1 != base_sha1 || stamp_of(&session.file)? != session.stamp {
        return Err(String::from(err_keys::FILE_CHANGED));
    }
    if !session.writable {
        return Err(String::from(err_keys::FILE_READ_ONLY));
    }

    if let Err(err) = apply(&mut session.nbt) {
        // 改动只套了一半，会话里这棵树已经不可信：扔掉，下次打开重新读
        sessions.remove(&key);
        return Err(err);
    }

    let result = write_session(session, chunk);
    if result.is_err() {
        // 写盘失败时内存树已经套过改动，但 sha1 / stamp 还是旧的：
        // 留着这份会话会让前端拿同一 base_sha1 重试时二次套用非幂等改动。
        sessions.remove(&key);
    }
    result
}

/// 把会话里已套好改动的树写回磁盘
///
/// 成功时同步更新会话的 SHA1 与状态戳；失败时调用方必须扔掉整份会话，
/// 因为此时内存树已经改过而磁盘上的基准没有跟上。
///
/// - `session`: 已套好改动的会话
/// - `chunk`: 区域文件的区块下标，整份文件为 `None`
///
/// # 返回值
///
/// 写回后内容的 SHA1，整份文件或区块负载；序列化或写盘失败返回错误文案
fn write_session(session: &mut NbtSession, chunk: Option<u32>) -> Result<String, String> {
    let mut out = Vec::new();
    session.nbt.write(&mut out).map_err(|err| err.to_string())?;
    let sha1 = hash_helper::gen_hash(HashType::Sha1, &out);

    match chunk {
        // 区块：整份区域重写回去，扇区与文件头交给 crate 自己排
        Some(index) => write_chunk_back(&mut session.file, index, &out)?,
        None => {
            // 就在这个句柄上写回：seek 回开头、先截断（写回的字节可能比原来少）再写
            session
                .file
                .seek(SeekFrom::Start(0))
                .map_err(|err| err.to_string())?;
            session.file.set_len(0).map_err(|err| err.to_string())?;
            session
                .file
                .write_all(&out)
                .map_err(|err| err.to_string())?;
            session.file.flush().map_err(|err| err.to_string())?;
        }
    }

    session.sha1 = sha1.clone();
    session.stamp = stamp_of(&session.file)?;
    Ok(sha1)
}

#[cfg(test)]
#[path = "../../../../tests/unit/windows_nbt_session_mod_tests.rs"]
mod tests;
