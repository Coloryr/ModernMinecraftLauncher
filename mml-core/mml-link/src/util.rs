//! 内部工具
//!
//! 链接记录表、hash 归一化与文件名提取。
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

use mml_base::hash_helper::{self, HashType};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::info::LinkInfoObj;

use super::*;

/// 实例映射记录表
///
/// # 返回值
///
/// 全局映射表；未初始化时懒建立空表
pub(crate) fn link_data() -> &'static RwLock<HashMap<Uuid, Vec<LinkInfoObj>>> {
    LINK_DATA.get_or_init(Default::default)
}

/// 规范化 SHA1
///
/// - `hash`: 原始哈希
///
/// # 返回值
///
/// 合法时返回小写 SHA1，否则 `None`
pub(crate) fn normalize_hash(hash: &str) -> Option<String> {
    let hash = hash.trim();
    if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(hash.to_ascii_lowercase())
}

/// 从原始文件名取出安全的单段文件名
///
/// Windows 不允许的字符替换成下划线；空名或路径片段返回 None。
///
/// - name: 原始文件名，可能带目录
///
/// # 返回值
///
/// 可用的文件名；无法得到时返回 None
pub(crate) fn file_name(name: &str) -> Option<String> {
    let file = Path::new(name).file_name()?.to_str()?.trim();
    if file.is_empty() || file == "." || file == ".." {
        return None;
    }

    let file: String = file
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '|' | '?' | '*') {
                '_'
            } else {
                ch
            }
        })
        .collect();
    if file.is_empty() || file == "." || file == ".." {
        None
    } else {
        Some(file)
    }
}

/// 同名冲突时的候选文件名：名字.sha1前8位.扩展名
///
/// - base: 原目标路径
/// - hash: 小写 SHA1
///
/// # 返回值
///
/// 带哈希后缀的路径
pub(crate) fn collide_file(base: &Path, hash: &str) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file");
    let short = &hash[..hash.len().min(8)];
    let name = match base.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{stem}.{short}.{ext}"),
        _ => format!("{stem}.{short}"),
    };
    base.with_file_name(name)
}

/// 为冲突文件找一个不覆盖已有内容的路径
///
/// - base: 原目标路径
/// - hash: 新文件 SHA1
///
/// # 返回值
///
/// 可写入的路径；已存在同哈希文件时返回它
pub(crate) fn unique_file(base: &Path, hash: &str) -> PathBuf {
    let mut file = collide_file(base, hash);
    if file.is_file() && hash_matches(&file, hash) {
        return file;
    }

    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file");
    let short = &hash[..hash.len().min(8)];
    let ext = base.extension().and_then(|ext| ext.to_str());
    let mut index = 1;
    loop {
        let name = match ext {
            Some(ext) if !ext.is_empty() => format!("{stem}.{short}-{index}.{ext}"),
            _ => format!("{stem}.{short}-{index}"),
        };
        file = base.with_file_name(name);
        if !file.exists() || hash_matches(&file, hash) {
            return file;
        }
        index += 1;
    }
}

/// 校验文件 SHA1
///
/// - `file`: 待校验文件
/// - `hash`: 小写 SHA1
///
/// # 返回值
///
/// 能读出文件且哈希一致时为 `true`
pub(crate) fn hash_matches(file: &Path, hash: &str) -> bool {
    let Ok(mut stream) = path_helper::open_read(file) else {
        return false;
    };
    let Ok(now) = hash_helper::gen_hash_from_reader(HashType::Sha1, &mut stream) else {
        return false;
    };
    now.eq_ignore_ascii_case(hash)
}
