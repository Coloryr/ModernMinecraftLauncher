//! 打开着的 NBT 文件
//!
//! 配置编辑器打开一个 NBT 文件时把它**真的打开**：文件句柄留着，解析出的树也留着。之后的保存
//! 直接在这份状态上套改动、就地写回（同一个句柄 seek 回开头、截断、重写），不再每次重新打开、
//! 重新读、重新解析。同一份文件在会话还在时再打开一次，也是直接给会话里那棵树，不碰磁盘。
//!
//! 会话活到配置窗口关闭：前端切走页面时逐个 [`close`]，多窗口模式下窗口销毁由
//! `lifecycle` 的 `Destroyed` 分支兜底 [`close_all`]。
//!
//! 「别处改过」靠两个便宜的检查发现，都不重读文件：前端那份基准 SHA1 与打开的这份一致，
//! 且文件的修改时间与长度没变过（[`Stamp`]）。拿着读写句柄也不会挡住别的程序读写，
//! 句柄默认就是共享读写删除的。

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use mml_base::hash_helper::{self, HashType};
use mml_nbt::nbt_file::NbtFile;
use mml_sys::path_helper;

use crate::err_keys;

/// 一个打开着的 NBT 文件
struct NbtSession {
    /// 文件句柄，读与写都走它
    file: File,
    /// 打开时读进来的内容；保存就在这上面套改动
    nbt: NbtFile,
    /// 最近一次读 / 写后文件字节的 SHA1
    sha1: String,
    /// 最近一次读 / 写后的状态戳
    stamp: Stamp,
    /// 能不能写回；只读文件也能看，改到时候再报只读
    writable: bool,
}

/// 文件的状态戳：修改时间与长度
#[derive(PartialEq)]
struct Stamp {
    /// 最后修改时间；取不到时为 `None`，只比长度
    modified: Option<SystemTime>,
    /// 文件长度，单位字节
    len: u64,
}

/// 打开着的文件，键是文件的绝对路径
static SESSIONS: LazyLock<Mutex<HashMap<String, NbtSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 会话的键：文件的绝对路径（调用方已经过 `resolve_under` 挡过穿越）
fn key_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// 取文件当前的状态戳
fn stamp_of(file: &File) -> Result<Stamp, String> {
    let meta = file.metadata().map_err(|err| err.to_string())?;
    Ok(Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

/// 打开一个 NBT 文件并收进会话；已经在会话里就直接用会话里那份，不碰磁盘
///
/// - `path`: 文件绝对路径
/// - `use_nbt`: 拿到内容后的回调，用来把它转成前端要的树
///
/// # 返回值
///
/// 回调的结果，以及这一份文件字节的 SHA1；读不开或解析不了时返回错误文案
pub(crate) fn open_with<T>(
    path: &Path,
    use_nbt: impl FnOnce(&NbtFile) -> T,
) -> Result<(T, String), String> {
    let key = key_of(path);
    if let Some(session) = SESSIONS.lock().unwrap().get(&key) {
        return Ok((use_nbt(&session.nbt), session.sha1.clone()));
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
    let mut cursor = Cursor::new(&data);
    let nbt = NbtFile::read(&mut cursor).map_err(|err| err.to_string())?;

    let value = use_nbt(&nbt);
    let sha1 = hash_helper::gen_hash(HashType::Sha1, &data);
    let session = NbtSession {
        stamp: stamp_of(&file)?,
        writable,
        sha1: sha1.clone(),
        file,
        nbt,
    };
    SESSIONS.lock().unwrap().insert(key, session);

    Ok((value, sha1))
}

/// 在已打开的会话上套改动并就地写回
///
/// 写之前两道核对：前端那份基准 SHA1 得与打开的这份一致，文件的状态戳也没被人动过
/// （长度或修改时间变了说明别处写过，内存里这棵树已经过期）。
///
/// - `path`: 文件绝对路径
/// - `base_sha1`: 前端手上那一份的 SHA1
/// - `apply`: 套改动的回调
///
/// # 返回值
///
/// 写回后文件字节的 SHA1；会话不在、基准对不上、文件被别处改过返回 `err.fileChanged`，
/// 只读文件返回 `err.fileReadOnly`，套改动或写盘失败返回各自的错误文案
pub(crate) fn update_with(
    path: &Path,
    base_sha1: &str,
    apply: impl FnOnce(&mut NbtFile) -> Result<(), String>,
) -> Result<String, String> {
    let key = key_of(path);
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

    let mut out = Vec::new();
    session.nbt.write(&mut out).map_err(|err| err.to_string())?;
    let sha1 = hash_helper::gen_hash(HashType::Sha1, &out);

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

    session.sha1 = sha1.clone();
    session.stamp = stamp_of(&session.file)?;

    Ok(sha1)
}

/// 关掉一个文件的会话，句柄跟着释放
///
/// - `path`: 文件绝对路径；没有这份会话就什么也不做
pub(crate) fn close(path: &Path) {
    SESSIONS.lock().unwrap().remove(&key_of(path));
}

/// 关掉全部会话：配置窗口没了，它打开过的文件都该放手
pub(crate) fn close_all() {
    SESSIONS.lock().unwrap().clear();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::sync::Mutex;

    use mml_base::hash_helper::{self, HashType};
    use mml_nbt::nbt_file::{CompressType, NbtFile};
    use mml_nbt::nbt_types::{compound, int};
    use mml_nbt::NbtType;
    use uuid::Uuid;

    use super::{close, close_all, open_with, update_with};
    use crate::err_keys;

    /// 会话表是进程级的、用例之间共用：这些用例必须串行跑，
    /// 否则一个用例的 `close_all` 会把另一个用例刚打开的会话清掉
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 造一个样本根标签：{ GameType: 1 }
    fn sample_root() -> NbtType {
        let mut root = compound();
        root.data.insert("GameType".to_string(), int(1).to_nbt());
        root.to_nbt()
    }

    /// 在临时目录里造一个 gzip 的 NBT 文件
    fn write_sample(tag: &str) -> std::path::PathBuf {
        let dir = mml_testutil::temp_dir().join(format!("mml-nbt-session-{tag}-{}", Uuid::new_v4()));
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

        assert_eq!(open_with(&path, |nbt| game_type_of(nbt)).unwrap().0, 1);

        // 绕过会话直接改盘上的字节：长度与修改时间都会变
        fs::write(&path, b"not nbt at all").unwrap();

        // 会话还在：再打开给的还是原来那份，说明没重新读
        assert_eq!(open_with(&path, |nbt| game_type_of(nbt)).unwrap().0, 1);

        // 关掉之后才是重新读 —— 这时盘上那份已经不是合法 NBT 了
        close(&path);
        assert!(open_with(&path, |nbt| game_type_of(nbt)).is_err());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// 保存就在会话上套改动、由原句柄写回：盘上的字节与返回的 SHA1 都要对得上
    #[test]
    fn update_writes_through_the_same_handle() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = write_sample("update");
        open_with(&path, |_| ()).unwrap();
        let sha1 = hash_helper::gen_hash(HashType::Sha1, &fs::read(&path).unwrap());

        let new_sha1 = update_with(&path, &sha1, |nbt| {
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
        close(&path);
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
        open_with(&path, |_| ()).unwrap();

        // 基准 SHA1 不是这一份
        let err = update_with(&path, "deadbeef", |_| Ok(())).unwrap_err();
        assert_eq!(err, err_keys::FILE_CHANGED);

        // 盘上被别处改过：状态戳变了
        fs::write(&path, b"changed elsewhere").unwrap();
        let err = update_with(&path, "deadbeef", |_| Ok(())).unwrap_err();
        assert_eq!(err, err_keys::FILE_CHANGED);

        // 没打开过就改：让他重新打开
        close_all();
        let err = update_with(&path, "deadbeef", |_| Ok(())).unwrap_err();
        assert_eq!(err, err_keys::FILE_CHANGED);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// 套改动失败时把会话扔掉：那棵树已经只套了一半，下次打开必须重新读
    #[test]
    fn failed_update_drops_the_session() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = write_sample("failed");
        open_with(&path, |_| ()).unwrap();
        let sha1 = hash_helper::gen_hash(HashType::Sha1, &fs::read(&path).unwrap());

        let err = update_with(&path, &sha1, |_| Err(String::from("boom"))).unwrap_err();
        assert_eq!(err, "boom");

        // 会话没了：紧接着的保存应当按"重新打开"拒掉，而打开会重新读盘
        assert_eq!(
            update_with(&path, &sha1, |_| Ok(())).unwrap_err(),
            err_keys::FILE_CHANGED
        );
        assert_eq!(open_with(&path, |nbt| game_type_of(nbt)).unwrap().0, 1);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
