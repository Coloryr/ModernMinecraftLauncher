//! 主题包的容器层与路径层校验
//!
//! 只做导入期的一次性扫描：把中央目录读出来，逐条目检查加密、条目类型、大小、压缩比、
//! 名字合法性，并算出「整包是否套了一层顶层目录」。读取内容只给清单用，
//! 其余条目只在真正被请求时由常驻句柄按条目解压。

use std::{
    collections::HashMap,
    io::Read,
    path::Path,
};

use zip::ZipArchive;

use crate::err_keys;
use crate::windows::web_assets::{
    MAX_ARCHIVE_BYTES, MAX_COMPRESSION_RATIO, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES,
};

/// 中央目录里的一条记录，名字已规范化
pub(super) struct Entry {
    /// 规范化后的条目名：分隔符统一为正斜杠，去掉 `.` 与空段，目录不带结尾斜杠
    pub(super) name: String,
    /// 中央目录下标，读取内容时用
    pub(super) index: usize,
    /// 是否目录条目
    pub(super) is_dir: bool,
    /// 解压后大小，单位字节
    pub(super) size: u64,
    /// 压缩后大小，单位字节，只用于压缩比检查
    compressed: u64,
}

/// 按名字找到的条目
pub(super) struct Located {
    /// 中央目录下标
    pub(super) index: usize,
    /// 解压后大小，单位字节
    pub(super) size: u64,
    /// 包里的实际条目名，大小写不一致时提示用
    pub(super) name: String,
    /// 是不是只能按大小写不敏感匹配到
    pub(super) case_mismatch: bool,
}

/// 扫描完成的包：中央目录条目 + 布局根
pub(super) struct Package {
    /// 已打开的 zip，读取清单内容时用
    archive: ZipArchive<std::fs::File>,
    /// 全部条目，顺序与中央目录一致
    entries: Vec<Entry>,
    /// 唯一顶层目录名；包根就散放文件时为 `None`
    root: Option<String>,
}

impl Package {
    /// 打开 zip 并完成容器层、路径层校验
    ///
    /// - `path`: 待导入的压缩包路径
    ///
    /// # 返回值
    ///
    /// 扫描结果；不是合法 zip、含加密 / 不支持条目、路径越界、重名或超限时返回错误键
    pub(super) fn open(path: &Path) -> Result<Self, String> {
        let meta = std::fs::metadata(path).map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
        if !meta.is_file() || meta.len() == 0 {
            return Err(error(err_keys::THEME_ZIP_INVALID));
        }
        if meta.len() > MAX_ARCHIVE_BYTES {
            return Err(error(err_keys::PAGE_ARCHIVE_LIMIT));
        }
        let file = std::fs::File::open(path).map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
        let archive = ZipArchive::new(file).map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
        if archive.len() > MAX_ENTRIES {
            return Err(error(err_keys::PAGE_ARCHIVE_LIMIT));
        }

        let count = archive.len();
        let mut entries = Vec::with_capacity(count);
        let mut names: HashMap<String, ()> = HashMap::with_capacity(count);
        let mut lower_names: HashMap<String, ()> = HashMap::with_capacity(count);
        let mut total = 0u64;

        for index in 0..count {
            // 只取元数据：加密条目在 by_index 阶段就要密码，这里必须先看 flags
            let entry = archive
                .by_index_data(index)
                .map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
            if entry.encrypted() {
                return Err(error(err_keys::THEME_ZIP_ENCRYPTED));
            }
            // 只认普通文件与目录：符号链接、设备、FIFO 一律拒绝
            if entry.is_symlink() {
                return Err(error(err_keys::THEME_ZIP_UNSUPPORTED));
            }
            let mode = entry.unix_mode();
            if let Some(mode) = mode {
                match mode & 0o170000 {
                    0 | 0o100000 | 0o040000 => {}
                    _ => return Err(error(err_keys::THEME_ZIP_UNSUPPORTED)),
                }
            }
            // 条目名必须是合法 UTF-8：非 UTF-8 名字会让协议层与大小写比较都失真
            let raw = std::str::from_utf8(entry.name_raw())
                .map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
            let name = normalize(raw)?;
            // 有的打包工具只写 Unix 目录位、名字不带结尾斜杠，两种都认
            let is_dir = entry.is_dir() || mode.is_some_and(|mode| mode & 0o170000 == 0o040000);
            let size = entry.size();
            let compressed = entry.compressed_size();

            if !is_dir {
                if size > MAX_ENTRY_BYTES {
                    return Err(error(err_keys::PAGE_ARCHIVE_LIMIT));
                }
                total = total.saturating_add(size);
                if total > MAX_TOTAL_BYTES {
                    return Err(error(err_keys::PAGE_ARCHIVE_LIMIT));
                }
                // 中央目录声明的压缩后大小不可能超过文件本身
                if compressed > meta.len() {
                    return Err(error(err_keys::THEME_ZIP_INVALID));
                }
            }
            // 规范化重名与大小写重名都不允许：Windows 上后者会互相覆盖
            if names.insert(name.clone(), ()).is_some()
                || lower_names.insert(name.to_ascii_lowercase(), ()).is_some()
            {
                return Err(error(err_keys::THEME_ZIP_DUPLICATE));
            }
            entries.push(Entry {
                name,
                index,
                is_dir,
                size,
                compressed,
            });
        }

        // 压缩比单列一轮：先让大小限额报错，否则炸弹包会先报比例
        for entry in &entries {
            if entry.is_dir || entry.size == 0 {
                continue;
            }
            if entry.compressed == 0
                || entry.size > entry.compressed.saturating_mul(MAX_COMPRESSION_RATIO)
            {
                return Err(error(err_keys::THEME_ZIP_RATIO));
            }
        }

        let root = detect_root(&entries);
        Ok(Self {
            archive,
            entries,
            root,
        })
    }

    /// 按包内相对名找文件条目，先精确匹配，再按大小写不敏感匹配一次
    ///
    /// - `rel`: 相对包根的条目名，分隔符为正斜杠
    ///
    /// # 返回值
    ///
    /// 找到的文件条目；不存在或是目录条目时返回 `None`
    pub(super) fn locate(&self, rel: &str) -> Option<Located> {
        let full = self.full_name(rel);
        if let Some(entry) = self.entries.iter().find(|entry| !entry.is_dir && entry.name == full) {
            return Some(located(entry, false));
        }
        self.entries
            .iter()
            .find(|entry| !entry.is_dir && entry.name.eq_ignore_ascii_case(&full))
            .map(|entry| located(entry, true))
    }

    /// 读一个条目的内容，最多读 `limit` 字节
    ///
    /// - `index`: 中央目录下标
    /// - `limit`: 读取上限，单位字节
    ///
    /// # 返回值
    ///
    /// 条目内容；超过上限返回 `None`，解压或 CRC 校验失败返回错误键
    pub(super) fn read(&mut self, index: usize, limit: u64) -> Result<Option<Vec<u8>>, String> {
        let entry = self
            .archive
            .by_index(index)
            .map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
        let mut data = Vec::new();
        entry
            .take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|_| error(err_keys::THEME_ZIP_INVALID))?;
        if data.len() as u64 > limit {
            return Ok(None);
        }
        Ok(Some(data))
    }

    /// 把相对名补上唯一顶层目录前缀
    ///
    /// - `rel`: 相对包根的条目名
    fn full_name(&self, rel: &str) -> String {
        match &self.root {
            Some(root) => format!("{root}/{rel}"),
            None => String::from(rel),
        }
    }
}

/// 组装一个找到的条目
fn located(entry: &Entry, case_mismatch: bool) -> Located {
    Located {
        index: entry.index,
        size: entry.size,
        name: entry.name.clone(),
        case_mismatch,
    }
}

/// 规范化并校验条目名
///
/// 反斜杠统一成正斜杠，`.` 段与空段去掉；绝对路径、盘符、`..`、控制字符与
/// 规范化后为空的路径一律拒绝。
///
/// - `raw`: 中央目录里的原始条目名
///
/// # 返回值
///
/// 规范化后的名字；命中任一拒绝规则时返回 `err.themeZipEntryPath`
fn normalize(raw: &str) -> Result<String, String> {
    let unified = raw.replace('\\', "/");
    if unified.starts_with('/') {
        return Err(error(err_keys::THEME_ZIP_ENTRY_PATH));
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in unified.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(error(err_keys::THEME_ZIP_ENTRY_PATH));
        }
        // 盘符 / 备用数据流用 `:`，控制字符会让后续路径处理与日志都失真
        if segment.chars().any(|ch| ch == ':' || ch.is_control()) {
            return Err(error(err_keys::THEME_ZIP_ENTRY_PATH));
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(error(err_keys::THEME_ZIP_ENTRY_PATH));
    }
    Ok(segments.join("/"))
}

/// 判断整包是否套了一层唯一顶层目录
///
/// 顶层出现文件时不认为有唯一顶层目录，入口文件必须就在那个根下；目录条目
/// 本身不算「顶层文件」，否则带目录记录的整包永远解不开那层。
///
/// - `entries`: 全部条目
///
/// # 返回值
///
/// 唯一顶层目录名；包根散放文件或顶层有两个以上目录时返回 `None`
fn detect_root(entries: &[Entry]) -> Option<String> {
    let mut top: Option<&str> = None;
    for entry in entries {
        let first = entry.name.split('/').next()?;
        if !entry.name.contains('/') && !entry.is_dir {
            return None;
        }
        match top {
            None => top = Some(first),
            Some(seen) if seen == first => {}
            Some(_) => return None,
        }
    }
    top.map(String::from)
}

/// 错误键转成命令返回的字符串
///
/// - `key`: crate::err_keys 里的常量
fn error(key: &str) -> String {
    String::from(key)
}
