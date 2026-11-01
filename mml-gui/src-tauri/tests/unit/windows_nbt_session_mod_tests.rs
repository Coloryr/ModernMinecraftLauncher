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

