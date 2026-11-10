//! 客户端设置锁：密码只存加盐哈希，落在隐藏文件里，不写进 gui_config。
//!
//! 运行目录一份；内部数据目录再按运行目录区分一份，只存在一侧时互相补齐，解锁时两份一起删。
//! 内部数据目录是同一用户共用的，那份文件名带运行目录摘要：装了多个启动器时各锁各的，
//! 不会因为其中一个上了锁就把另一个也锁上。
//! Windows 给它打隐藏属性，其它平台靠点开头的文件名并把权限收到 600。

use crate::err_keys;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use uuid::Uuid;

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

/// 自描述串拆出的字段
struct LockParts<'a> {
    /// 迭代轮数
    rounds: u32,
    /// 十六进制盐
    salt: &'a str,
    /// 十六进制哈希
    hash: &'a str,
}

/// 拆自描述串
///
/// - `text`: 形如 `$pbkdf2-sha256$120000$<盐>$<哈希>`
///
/// # 返回值
///
/// 拆出的轮数、盐与哈希；算法不认识或字段不齐时返回 `None`
fn parts(text: &str) -> Option<LockParts<'_>> {
    let mut pieces = text.strip_prefix('$')?.split('$');
    if pieces.next()? != ALGORITHM {
        return None;
    }
    let rounds: u32 = pieces.next()?.parse().ok()?;
    let salt = pieces.next()?;
    let hash = pieces.next()?;
    if pieces.next().is_some()
        || salt.is_empty()
        || hash.is_empty()
        || rounds == 0
        || rounds > 10_000_000
    {
        return None;
    }
    Some(LockParts {
        rounds,
        salt,
        hash,
    })
}

/// 内部数据目录那份锁文件名：带运行目录摘要，几个安装各锁各的
///
/// - `run`: 运行目录
///
/// # 返回值
///
/// 形如 `.client_lock.<16 位十六进制>.json`
fn mirror_name(run: &Path) -> String {
    let mut text = run.to_string_lossy().into_owned();
    // Windows 路径不分大小写，同一个目录换个写法不能算成两个名字
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    let digest = sha2::Sha256::digest(text.as_bytes());
    let tag: String = digest[..8].iter().map(|byte| format!("{byte:02x}")).collect();
    format!(".client_lock.{tag}.json")
}

/// 读锁文件；不存在、写坏或字段为空都当作没有锁
///
/// - `path`: 某个落点
fn read(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;
    if data.len() > 4096 {
        return None;
    }
    from_bytes(&data)
}

/// 从文件内容解析自描述串，兼容旧版两字段 JSON
///
/// - `data`: 锁文件内容
fn from_bytes(data: &[u8]) -> Option<String> {
    if let Ok(lock) = serde_json::from_slice::<StoredLock>(data) {
        if parts(&lock.password).is_none() {
            return None;
        }
        return Some(lock.password);
    }
    let lock: LegacyLock = serde_json::from_slice(data).ok()?;
    if lock.salt.is_empty() || lock.hash.is_empty() {
        return None;
    }
    Some(encode(&lock.salt, &lock.hash))
}

/// 运行目录：与 `gui_config.json` 同级
///
/// 调用方全在命令里，启动时 `gui_config::init` 早就跑过了，所以这里不可能没值。
fn run_dir() -> PathBuf {
    crate::gui_config::dir().expect("gui_config 必须在 client_lock 之前初始化")
}

/// 系统内部数据目录：Windows `%LOCALAPPDATA%\mml`，Linux / macOS `~/.local/share/mml`
fn data_dir() -> PathBuf {
    mml_base::inner_path::get_inner_path()
}

/// 锁文件的两个落点：运行目录一份，内部数据目录按运行目录区分的一份
///
/// 运行目录本身就是内部数据目录时只留运行目录那份 —— 同一个目录里摆两个名字，
/// 解锁要删两次，手动删掉一个又会被另一个补回来。
///
/// # 返回值
///
/// 落点的文件路径，运行目录在前
fn targets() -> Vec<PathBuf> {
    targets_in(&run_dir(), &data_dir())
}

/// 由运行目录与内部数据目录算出落点，`targets` 的纯函数版本，测试直接调
///
/// - `run`: 运行目录
/// - `data`: 内部数据目录
///
/// # 返回值
///
/// 见 [`targets`]
fn targets_in(run: &Path, data: &Path) -> Vec<PathBuf> {
    let file = run.join(FILE_NAME);
    if same_dir(run, data) {
        return vec![file];
    }
    vec![file, data.join(mirror_name(run))]
}

/// 两个路径是不是同一个目录
///
/// 都存在时按规范化路径比，能吃掉大小写 / 短文件名 / 符号链接的差异；
/// 规范化失败就退回文本比较，至少去掉末尾分隔符。
///
/// - `a` / `b`: 待比较的两个路径
///
/// # 返回值
///
/// 指向同一个目录返回 `true`
fn same_dir(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (fs::canonicalize(a), fs::canonicalize(b)) {
        return a == b;
    }
    let norm = |path: &Path| {
        let text = path.to_string_lossy();
        let text = text.trim_end_matches(['/', '\\']);
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text.to_string()
        }
    };
    norm(a) == norm(b)
}

/// 两侧对齐：运行目录那份为准，缺的或不一样的都按它补齐；只有内部数据目录有时反向补回
///
/// - `paths`: 两个落点，运行目录在前
fn sync(paths: &[PathBuf]) {
    let Some(source) = paths.iter().find(|path| path.is_file()) else {
        return;
    };
    let Ok(data) = fs::read(source) else {
        return;
    };
    for path in paths {
        // 内容一样就不重写，免得每次取配置都刷一遍修改时间
        if fs::read(path).is_ok_and(|current| current == data) {
            continue;
        }
        let _ = install(path, &data);
    }
}

/// 两侧任意一侧有锁文件就算锁定；文件写坏也算，免得删两行就能绕过
///
/// - `paths`: 两个落点
fn exists(paths: &[PathBuf]) -> bool {
    paths.iter().any(|path| path.is_file())
}

/// 同步两侧后判断有没有锁
///
/// - `paths`: 两个落点
fn locked_in(paths: &[PathBuf]) -> bool {
    sync(paths);
    exists(paths)
}

/// 当前是否锁定：运行目录与内部数据目录任一侧有锁文件
pub(crate) fn locked() -> bool {
    locked_in(&targets())
}

/// 校验密码；两侧都没有锁文件时直接放行
///
/// - `paths`: 两个落点
/// - `password`: 用户输入
///
/// # 返回值
///
/// 任一侧的锁文件能验证通过、或根本没有锁时返回 `true`
fn verify_in(paths: &[PathBuf], password: &str) -> bool {
    sync(paths);
    if !exists(paths) {
        return true;
    }
    paths.iter().filter_map(|path| read(path)).any(|lock| {
        parts(&lock).is_some_and(|parsed| {
            hash_password(password, parsed.salt, parsed.rounds).as_str() == parsed.hash
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
/// - `paths`: 两个落点
/// - `data`: 锁文件 JSON
fn write_all(paths: &[PathBuf], data: &[u8]) -> Result<(), String> {
    let mut written = 0;
    let mut last = None;
    for path in paths {
        match install(path, data) {
            Ok(()) => written += 1,
            Err(error) => last = Some(error),
        }
    }
    // 有一侧写成功就算锁上了，另一侧下次同步会补
    if written == 0 {
        Err(last.unwrap_or_default())
    } else {
        Ok(())
    }
}

/// 写入新密码
///
/// - `paths`: 两个落点
/// - `password`: 新密码，不能为空
fn set_in(paths: &[PathBuf], password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err(err_keys::CLIENT_LOCK_EMPTY.into());
    }
    let salt = Uuid::new_v4().simple().to_string();
    let hash = hash_password(password, &salt, ROUNDS);
    let lock = StoredLock {
        password: encode(&salt, &hash),
    };
    let data = serde_json::to_vec_pretty(&lock).map_err(|e| e.to_string())?;
    write_all(paths, &data)
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

/// 删除一个落点的锁文件；本来就没有也算成功
///
/// - `path`: 某个落点
fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// 删除两侧的锁文件
///
/// - `paths`: 两个落点
fn clear_in(paths: &[PathBuf]) -> Result<(), String> {
    let mut first = None;
    for path in paths {
        if let Err(error) = remove(path) {
            first.get_or_insert(error);
        }
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
    let paths = targets();
    sync(&paths);
    paths.iter().find_map(|path| fs::read(path).ok())
}

/// 把一份锁文件内容原子写入某个落点，并补隐藏属性
///
/// - `path`: 两个落点中的一个
/// - `data`: 要写入的 JSON 内容
fn install(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let temp = path.with_file_name(format!(".{}.tmp", Uuid::new_v4().simple()));
    fs::write(&temp, data).map_err(|e| e.to_string())?;
    hide(&temp);
    // 先写临时文件再改名，写到一半不会留下半份锁
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        e.to_string()
    })?;
    hide(path);
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
    use windows::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, GetFileAttributesW, SetFileAttributesW,
    };
    use windows::core::PCWSTR;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // 属性读写按 Win32 的既有口径：失败就只当没隐藏，不影响锁本身
    unsafe {
        let attrs = GetFileAttributesW(PCWSTR(wide.as_ptr()));
        if attrs == u32::MAX {
            return;
        }
        let _ = SetFileAttributesW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(attrs | FILE_ATTRIBUTE_HIDDEN.0),
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/client_lock_tests.rs"]
mod tests;
