//! 客户端设置锁：密码只存加盐哈希，落在隐藏文件里，不写进 gui_config。
//!
//! 运行目录与内部数据目录各存一份，只存在一侧时互相补齐；解锁时两份一起删。
//! Windows 给它打隐藏属性，其它平台靠点开头的文件名并把权限收到 600。

use std::{fs, io::ErrorKind, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::err_keys;

/// 锁文件名：点开头，Unix 下天然隐藏
pub(crate) const FILE_NAME: &str = ".client_lock.json";

/// 新写锁文件用的轮数；读的时候以串里记的为准
const ROUNDS: u32 = 120_000;
/// 自描述串里的算法标识
const ALGORITHM: &str = "pbkdf2-sha256";

/// 落盘的锁内容：一条自描述串，盐、哈希、算法与轮数都在里面
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct StoredLock {
    /// `$pbkdf2-sha256$轮数$盐$哈希`
    password: String,
}

/// 旧版两字段格式，只为兼容已经存在的锁文件
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct LegacyLock {
    /// 随机盐，十六进制
    salt: String,
    /// PBKDF2 哈希，十六进制
    hash: String,
}

/// 密码哈希：PBKDF2-HMAC-SHA256
///
/// - `password`: 明文密码
/// - `salt`: 十六进制盐
/// - `rounds`: 迭代轮数，由锁文件里的自描述串给出
///
/// # 返回值
///
/// 三十二字节哈希的十六进制文本
fn hash_password(password: &str, salt: &str, rounds: u32) -> String {
    pbkdf2::pbkdf2_hmac_array::<sha2::Sha256, 32>(password.as_bytes(), salt.as_bytes(), rounds)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 拼当前格式的自描述串
///
/// - `salt`: 十六进制盐
/// - `hash`: 十六进制哈希
fn encode(salt: &str, hash: &str) -> String {
    format!("${ALGORITHM}${ROUNDS}${salt}${hash}")
}

/// 拆自描述串
///
/// - `text`: 形如 `$pbkdf2-sha256$120000$<盐>$<哈希>`
///
/// # 返回值
///
/// 轮数、盐、哈希；算法不认识或字段不齐时返回 `None`
fn parts(text: &str) -> Option<(u32, &str, &str)> {
    let mut pieces = text.strip_prefix('$')?.split('$');
    if pieces.next()? != ALGORITHM { return None; }
    let rounds: u32 = pieces.next()?.parse().ok()?;
    let salt = pieces.next()?;
    let hash = pieces.next()?;
    if pieces.next().is_some() || salt.is_empty() || hash.is_empty()
        || rounds == 0 || rounds > 10_000_000 {
        return None;
    }
    Some((rounds, salt, hash))
}

/// 锁文件路径
///
/// - `dir`: 某个落点
fn lock_path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// 读锁文件；不存在、写坏或字段为空都当作没有锁
///
/// - `dir`: 某个落点
fn read(dir: &Path) -> Option<String> {
    let data = fs::read(lock_path(dir)).ok()?;
    if data.len() > 4096 { return None; }
    from_bytes(&data)
}

/// 从文件内容解析自描述串，兼容旧版两字段 JSON
///
/// - `data`: 锁文件内容
fn from_bytes(data: &[u8]) -> Option<String> {
    if let Ok(lock) = serde_json::from_slice::<StoredLock>(data) {
        if parts(&lock.password).is_none() { return None; }
        return Some(lock.password);
    }
    let lock: LegacyLock = serde_json::from_slice(data).ok()?;
    if lock.salt.is_empty() || lock.hash.is_empty() { return None; }
    Some(encode(&lock.salt, &lock.hash))
}

/// 运行目录：与 `gui_config.json` 同级，配置还没初始化时为 `None`
fn run_dir() -> Option<PathBuf> {
    crate::gui_config::dir()
}

/// 系统内部数据目录：Windows `%LOCALAPPDATA%\mml`，Linux `~/.local/share/mml`，macOS `~/.mml`
fn data_dir() -> PathBuf {
    mml_base::inner_path::get_inner_path()
}

/// 锁文件的两个落点：运行目录 + 内部数据目录，同一个路径只留一份
fn targets() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = run_dir() { dirs.push(dir); }
    let data = data_dir();
    if !dirs.contains(&data) { dirs.push(data); }
    dirs
}

/// 两侧对齐：运行目录那份为准，缺的或不一样的都按它补齐；只有内部数据目录有时反向补回
///
/// - `dirs`: 两个落点，运行目录在前
fn sync(dirs: &[PathBuf]) {
    let Some(source) = dirs.iter().find(|dir| lock_path(dir).is_file()) else { return; };
    let Ok(data) = fs::read(lock_path(source)) else { return; };
    for dir in dirs {
        // 内容一样就不重写，免得每次取配置都刷一遍修改时间
        if fs::read(lock_path(dir)).is_ok_and(|current| current == data) { continue; }
        let _ = install(dir, &data);
    }
}

/// 两侧任意一侧有锁文件就算锁定；文件写坏也算，免得删两行就能绕过
///
/// - `dirs`: 两个落点
fn exists(dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|dir| lock_path(dir).is_file())
}

/// 同步两侧后判断有没有锁
///
/// - `dirs`: 两个落点
fn locked_in(dirs: &[PathBuf]) -> bool {
    sync(dirs);
    exists(dirs)
}

/// 当前是否锁定：运行目录与内部数据目录任一侧有锁文件
pub(crate) fn locked() -> bool {
    locked_in(&targets())
}

/// 校验密码；两侧都没有锁文件时直接放行
///
/// - `dirs`: 两个落点
/// - `password`: 用户输入
///
/// # 返回值
///
/// 任一侧的锁文件能验证通过、或根本没有锁时返回 `true`
fn verify_in(dirs: &[PathBuf], password: &str) -> bool {
    sync(dirs);
    if !exists(dirs) { return true; }
    dirs.iter().filter_map(|dir| read(dir)).any(|lock| {
        parts(&lock).is_some_and(|(rounds, salt, hash)| {
            hash_password(password, salt, rounds).as_str() == hash
        })
    })
}

/// 校验密码
///
/// - `password`: 用户输入
///
/// # 返回值
///
/// 密码正确、或根本没有锁定时返回 `true`
pub(crate) fn verify(password: &str) -> bool {
    verify_in(&targets(), password)
}

/// 把锁文件内容写到两个落点
///
/// - `dirs`: 两个落点
/// - `data`: 锁文件 JSON
fn write_all(dirs: &[PathBuf], data: &[u8]) -> Result<(), String> {
    let mut written = 0;
    let mut last = None;
    for dir in dirs {
        match install(dir, data) {
            Ok(()) => written += 1,
            Err(error) => last = Some(error),
        }
    }
    // 有一侧写成功就算锁上了，另一侧下次同步会补
    if written == 0 { Err(last.unwrap_or_default()) } else { Ok(()) }
}

/// 写入新密码
///
/// - `dirs`: 两个落点
/// - `password`: 新密码，不能为空
fn set_in(dirs: &[PathBuf], password: &str) -> Result<(), String> {
    if password.is_empty() { return Err(err_keys::CLIENT_LOCK_EMPTY.into()); }
    let salt = Uuid::new_v4().simple().to_string();
    let hash = hash_password(password, &salt, ROUNDS);
    let lock = StoredLock { password: encode(&salt, &hash) };
    let data = serde_json::to_vec_pretty(&lock).map_err(|e| e.to_string())?;
    write_all(dirs, &data)
}

/// 写入新密码：运行目录与内部数据目录各一份
///
/// - `password`: 新密码，不能为空
///
/// # 返回值
///
/// 至少写成功一份算成功
pub(crate) fn set(password: &str) -> Result<(), String> {
    set_in(&targets(), password)
}

/// 删除一侧的锁文件；本来就没有也算成功
///
/// - `dir`: 某个落点
fn remove(dir: &Path) -> Result<(), String> {
    match fs::remove_file(lock_path(dir)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// 删除两侧的锁文件
///
/// - `dirs`: 两个落点
fn clear_in(dirs: &[PathBuf]) -> Result<(), String> {
    let mut first = None;
    for dir in dirs {
        if let Err(error) = remove(dir) { first.get_or_insert(error); }
    }
    first.map_or(Ok(()), Err)
}

/// 解锁：两侧同时删除
///
/// # 返回值
///
/// 两侧都删掉、或本来就不存在时返回 `Ok(())`
pub(crate) fn clear() -> Result<(), String> {
    clear_in(&targets())
}

/// 读现有锁文件内容，供服务器包导出
///
/// # 返回值
///
/// 两侧都没有时返回 `None`
pub(crate) fn read_bytes() -> Option<Vec<u8>> {
    let dirs = targets();
    sync(&dirs);
    dirs.iter().find_map(|dir| fs::read(lock_path(dir)).ok())
}

/// 把一份锁文件内容原子写入某个落点，并补隐藏属性
///
/// - `dir`: 两个落点中的一个
/// - `data`: 要写入的 JSON 内容
pub(crate) fn install(dir: &Path, data: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let temp = dir.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    fs::write(&temp, data).map_err(|e| e.to_string())?;
    hide(&temp);
    // 先写临时文件再改名，写到一半不会留下半份锁
    fs::rename(&temp, lock_path(dir)).map_err(|e| {
        let _ = fs::remove_file(&temp);
        e.to_string()
    })?;
    hide(&lock_path(dir));
    Ok(())
}

/// 这段内容是不是一份能用的锁文件
///
/// - `data`: 从服务器包里读出的内容
///
/// # 返回值
///
/// 字段齐全且非空返回 `true`
pub(crate) fn parse_ok(data: &[u8]) -> bool {
    data.len() <= 4096 && from_bytes(data).is_some()
}

/// 给锁文件打隐藏属性；Windows 用文件属性，Unix 收紧权限
///
/// - `path`: 锁文件或它的临时文件
pub(crate) fn hide(path: &Path) {
    #[cfg(windows)]
    {
        set_hidden(path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
    }
}

/// Windows：沿用现有属性再加隐藏位
///
/// - `path`: 锁文件路径
#[cfg(windows)]
fn set_hidden(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // 属性读写按 Win32 的既有口径：失败就只当没隐藏，不影响锁本身
    unsafe {
        let attrs = GetFileAttributesW(PCWSTR(wide.as_ptr()));
        if attrs == u32::MAX { return; }
        let _ = SetFileAttributesW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(attrs | FILE_ATTRIBUTE_HIDDEN.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn writes_hidden_lock_and_verifies() {
        let dir = test_support::temp_dir("client-lock");
        let dirs = vec![dir.clone()];
        assert!(!locked_in(&dirs));
        // 没有锁时任何密码都放行，命令层也按未锁定处理
        assert!(verify_in(&dirs, "whatever"));
        set_in(&dirs, "pw").unwrap();
        assert!(locked_in(&dirs));
        // 单串里带算法与轮数，以后调参数也能验旧密码
        let raw = fs::read_to_string(lock_path(&dir)).unwrap();
        assert!(raw.contains("$pbkdf2-sha256$120000$"), "{raw}");
        assert!(verify_in(&dirs, "pw"));
        assert!(!verify_in(&dirs, "bad"));
        clear_in(&dirs).unwrap();
        assert!(!locked_in(&dirs));
        test_support::cleanup(&[&dir]);
    }

    #[test]
    fn verifies_with_rounds_from_the_string() {
        let dir = test_support::temp_dir("client-lock-rounds");
        let dirs = vec![dir.clone()];
        let password = format!("$pbkdf2-sha256$1000$aabb${}", hash_password("pw", "aabb", 1000));
        fs::write(lock_path(&dir), serde_json::to_vec(&StoredLock { password }).unwrap()).unwrap();
        assert!(verify_in(&dirs, "pw"));
        assert!(!verify_in(&dirs, "bad"));
        assert!(parse_ok(&fs::read(lock_path(&dir)).unwrap()));
        test_support::cleanup(&[&dir]);
    }

    #[test]
    fn reads_legacy_two_field_lock() {
        let dir = test_support::temp_dir("client-lock-legacy");
        let dirs = vec![dir.clone()];
        let legacy = format!(r#"{{"Salt":"aa","Hash":"{}"}}"#, hash_password("pw", "aa", ROUNDS));
        fs::write(lock_path(&dir), legacy).unwrap();
        assert!(verify_in(&dirs, "pw"));
        assert!(!verify_in(&dirs, "bad"));
        test_support::cleanup(&[&dir]);
    }

    #[test]
    fn mirrors_between_both_locations() {
        let run = test_support::temp_dir("client-lock-run");
        let data = test_support::temp_dir("client-lock-data");
        let dirs = vec![run.clone(), data.clone()];
        set_in(&dirs, "pw").unwrap();
        assert!(lock_path(&run).is_file() && lock_path(&data).is_file());
        // 两份不一致时以运行目录为准
        fs::write(lock_path(&data), b"stale").unwrap();
        assert!(locked_in(&dirs));
        assert_eq!(fs::read(lock_path(&data)).unwrap(), fs::read(lock_path(&run)).unwrap());
        // 删掉内部数据目录那份：下次访问用运行目录补回来
        fs::remove_file(lock_path(&data)).unwrap();
        assert!(locked_in(&dirs));
        assert!(lock_path(&data).is_file());
        // 只剩一份也能反向补，两边内容一致
        fs::remove_file(lock_path(&run)).unwrap();
        assert!(verify_in(&dirs, "pw"));
        assert!(lock_path(&run).is_file());
        // 解锁两份一起删
        clear_in(&dirs).unwrap();
        assert!(!lock_path(&run).exists() && !lock_path(&data).exists());
        test_support::cleanup(&[&run, &data]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_marks_lock_hidden() {
        use std::os::windows::fs::MetadataExt;
        let dir = test_support::temp_dir("client-lock-hidden");
        set_in(&[dir.clone()], "pw").unwrap();
        let attrs = fs::metadata(lock_path(&dir)).unwrap().file_attributes();
        assert_ne!(attrs & 0x2, 0, "lock file should carry FILE_ATTRIBUTE_HIDDEN");
        test_support::cleanup(&[&dir]);
    }

    #[test]
    fn corrupt_lock_stays_locked_but_never_verifies() {
        let dir = test_support::temp_dir("client-lock-corrupt");
        let dirs = vec![dir.clone()];
        fs::write(lock_path(&dir), b"not json").unwrap();
        assert!(locked_in(&dirs));
        assert!(!verify_in(&dirs, "pw"));
        clear_in(&dirs).unwrap();
        test_support::cleanup(&[&dir]);
    }
}
