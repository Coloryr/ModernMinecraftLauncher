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
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::sync::Mutex;

    use mml_base::hash_helper::{self, HashType};
    use mml_nbt::NbtType;
    use mml_nbt::nbt_file::{CompressType, NbtFile};
    use mml_nbt::nbt_types::{compound, int};
    use uuid::Uuid;

    use super::{close, close_all_of, open_with, update_with};
    use crate::err_keys;

    /// 会话表是进程级的、用例之间共用：这些用例必须串行跑，
    /// 否则一个用例的 `close_all_of` 会把另一个用例刚打开的会话清掉
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 用例里的实例 uuid，即会话的归属；用例之间互不相同，清理不会串台
    fn owner() -> Uuid {
        Uuid::new_v4()
    }

    /// 造一个样本根标签：{ GameType: 1 }
    fn sample_root() -> NbtType {
        let mut root = compound();
        root.data.insert("GameType".to_string(), int(1).to_nbt());
        root.to_nbt()
    }

    /// 在临时目录里造一个 gzip 的 NBT 文件
    fn write_sample(tag: &str) -> std::path::PathBuf {
        let dir =
            mml_testutil::temp_dir().join(format!("mml-nbt-session-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("level.dat");

        let mut bytes = Vec::new();
        NbtFile::new(sample_root(), CompressType::GZip)
            .write(&mut bytes)
            .unwrap();
        fs::write(&path, &bytes).unwrap();

        path
    }

    /// 读出文件里的 GameType
    fn game_type_of(nbt: &NbtFile) -> i32 {
        match &nbt.nbt {
            NbtType::Compound(root) => root.data["GameType"].as_int().unwrap().data,
            other => panic!("根不是复合：{other}"),
        }
    }

    /// 会话在的时候再打开不碰磁盘：盘上被别处改过，拿到的还是会话里那一份
    #[test]
    fn open_keeps_the_file_open() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = write_sample("keep");
        let owner = owner();

        assert_eq!(
            open_with(&owner, &path, None, |nbt| game_type_of(nbt))
                .unwrap()
                .value,
            1
        );

        // 绕过会话直接改盘上的字节：长度与修改时间都会变
        fs::write(&path, b"not nbt at all").unwrap();

        // 会话还在：再打开给的还是原来那份，说明没重新读
        assert_eq!(
            open_with(&owner, &path, None, |nbt| game_type_of(nbt))
                .unwrap()
                .value,
            1
        );

        // 关掉之后才是重新读 —— 这时盘上那份已经不是合法 NBT 了
        close(&path, None);
        assert!(open_with(&owner, &path, None, |nbt| game_type_of(nbt)).is_err());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// 会话按实例归属：关掉一个实例的窗口只清它那几份，别的实例还开着的不受影响
    #[test]
    fn close_all_of_keeps_other_owners() {
        let _guard = TEST_LOCK.lock().unwrap();
        let first = write_sample("owner-first");
        let second = write_sample("owner-second");
        let (owner_a, owner_b) = (owner(), owner());

        open_with(&owner_a, &first, None, |_| ()).unwrap();
        open_with(&owner_b, &second, None, |_| ()).unwrap();

        close_all_of(&owner_a);
        // 第一份会话没了：盘上写坏之后重新读会失败
        fs::write(&first, b"not nbt at all").unwrap();
        assert!(open_with(&owner_a, &first, None, |nbt| game_type_of(nbt)).is_err());
        // 第二份还在：拿到的仍是会话里那份，说明没被一起清掉
        fs::write(&second, b"not nbt at all").unwrap();
        assert_eq!(
            open_with(&owner_b, &second, None, |nbt| game_type_of(nbt))
                .unwrap()
                .value,
            1
        );

        for path in [&first, &second] {
            let _ = fs::remove_dir_all(path.parent().unwrap());
        }
    }

    /// 保存就在会话上套改动、由原句柄写回：盘上的字节与返回的 SHA1 都要对得上
    #[test]
    fn update_writes_through_the_same_handle() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = write_sample("update");
        open_with(&owner(), &path, None, |_| ()).unwrap();
        let sha1 = hash_helper::gen_hash(HashType::Sha1, &fs::read(&path).unwrap());

        let new_sha1 = update_with(&path, None, &sha1, |nbt| {
            match &mut nbt.nbt {
                NbtType::Compound(root) => {
                    root.data.insert("GameType".to_string(), int(7).to_nbt());
                }
                _ => panic!("根不是复合"),
            }
            Ok(())
        })
        .unwrap();

        // 返回的 SHA1 就是写回后盘上那份的 SHA1
        let on_disk = fs::read(&path).unwrap();
        assert_eq!(new_sha1, hash_helper::gen_hash(HashType::Sha1, &on_disk));

        // 重新读一遍（关掉会话）确认值真的落到盘上，压缩方式也还是 gzip
        close(&path, None);
        let mut cursor = Cursor::new(&on_disk);
        let back = NbtFile::read(&mut cursor).unwrap();
        assert_eq!(back.compress, CompressType::GZip);
        assert_eq!(game_type_of(&back), 7);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// 基准 SHA1 对不上、或文件被别处改过，都要拒掉
    #[test]
    fn update_rejects_stale_writes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = write_sample("stale");
        let owner = owner();
        open_with(&owner, &path, None, |_| ()).unwrap();

        // 基准 SHA1 不是这一份
        let err = update_with(&path, None, "deadbeef", |_| Ok(())).unwrap_err();
        assert_eq!(err, err_keys::FILE_CHANGED);

        // 盘上被别处改过：状态戳变了
        fs::write(&path, b"changed elsewhere").unwrap();
        let err = update_with(&path, None, "deadbeef", |_| Ok(())).unwrap_err();
        assert_eq!(err, err_keys::FILE_CHANGED);

        // 没打开过就改：让他重新打开
        close_all_of(&owner);
        let err = update_with(&path, None, "deadbeef", |_| Ok(())).unwrap_err();
        assert_eq!(err, err_keys::FILE_CHANGED);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// 套改动失败时把会话扔掉：那棵树已经只套了一半，下次打开必须重新读
    #[test]
    fn failed_update_drops_the_session() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = write_sample("failed");
        let owner = owner();
        open_with(&owner, &path, None, |_| ()).unwrap();
        let sha1 = hash_helper::gen_hash(HashType::Sha1, &fs::read(&path).unwrap());

        let err = update_with(&path, None, &sha1, |_| Err(String::from("boom"))).unwrap_err();
        assert_eq!(err, "boom");

        // 会话没了：紧接着的保存应当按"重新打开"拒掉，而打开会重新读盘
        assert_eq!(
            update_with(&path, None, &sha1, |_| Ok(())).unwrap_err(),
            err_keys::FILE_CHANGED
        );
        assert_eq!(
            open_with(&owner, &path, None, |nbt| game_type_of(nbt))
                .unwrap()
                .value,
            1
        );

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
