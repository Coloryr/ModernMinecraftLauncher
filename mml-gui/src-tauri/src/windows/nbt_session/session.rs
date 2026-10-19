//! 打开着的 NBT 会话表
//!
//! 按实例归属登记、按实例清理；`Stamp` 用来判断文件是否被别处改过。

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use mml_nbt::nbt_file::NbtFile;
use uuid::Uuid;


/// 一个打开着的 NBT 文件
pub(super) struct NbtSession {
    /// 打开它的实例 uuid，即配置窗口的实例；关那个窗口时按它只清这一份
    pub(super) owner: Uuid,
    /// 文件句柄，读与写都走它
    pub(super) file: File,
    /// 打开时读进来的内容；保存就在这上面套改动
    pub(super) nbt: NbtFile,
    /// 最近一次读 / 写后文件字节的 SHA1
    pub(super) sha1: String,
    /// 最近一次读 / 写后的状态戳
    pub(super) stamp: Stamp,
    /// 能不能写回；只读文件也能看，改到时候再报只读
    pub(super) writable: bool,
}

/// 文件的状态戳：修改时间与长度
#[derive(PartialEq)]
pub(super) struct Stamp {
    /// 最后修改时间；取不到时为 `None`，只比长度
    modified: Option<SystemTime>,
    /// 文件长度，单位字节
    len: u64,
}

/// 打开着的文件，键是文件的绝对路径
pub(super) static SESSIONS: LazyLock<Mutex<HashMap<String, NbtSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 会话的键：普通 NBT 是文件绝对路径，区域文件里的区块是 `路径#表项下标`
///
/// - `path`: 文件绝对路径（调用方已经过 `resolve_under` 挡过穿越）
/// - `chunk`: 区块表项下标；普通 NBT 文件传 `None`
pub(super) fn key_of(path: &Path, chunk: Option<u32>) -> String {
    let base = path.to_string_lossy().into_owned();
    match chunk {
        Some(index) => format!("{base}#{index}"),
        None => base,
    }
}

/// 取文件当前的状态戳
pub(super) fn stamp_of(file: &File) -> Result<Stamp, String> {
    let meta = file.metadata().map_err(|err| err.to_string())?;
    Ok(Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

/// 关掉一个文件的会话，句柄跟着释放
///
/// - `path`: 文件绝对路径；没有这份会话就什么也不做
/// - `chunk`: 文件是区域文件时的区块下标，整份文件为 `None`
pub(crate) fn close(path: &Path, chunk: Option<u32>) {
    SESSIONS.lock().unwrap().remove(&key_of(path, chunk));
}

/// 关掉某个实例的全部会话：它的配置窗口没了，它打开过的文件都该放手
///
/// - `owner`: 实例 uuid，见 [`open_with`]
pub(crate) fn close_all_of(owner: &Uuid) {
    SESSIONS.lock().unwrap().retain(|_, s| s.owner != *owner);
}
