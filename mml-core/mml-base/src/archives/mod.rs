//! 压缩包处理模块
//!
//! 提供多种压缩格式的压缩和解压功能：
//!
//! | 格式 | 实现 |
//! |------|------|
//! | Zip | [`zip_reader`] — 基于 `zip` crate |
//! | Tar / TarGz / TarXz | [`tar_reader`] — 基于 `tar` + `flate2`/`xz2` crate |
//! | 7z | [`r7z_reader`] — 基于 `sevenz-rust` crate |
//!
//! # 进度回调
//!
//! 通过 [`IBaseArchiveGui`] trait 支持压缩/解压进度的 UI 回调通知。
//! 使用 [`ArchiveProcess`] 内部追踪进度状态。

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use mml_names::{
    i18_items::error_type::{CoreResult, ErrorType},
    names,
};

pub mod base_archive;
pub mod r7z_reader;
pub mod tar_reader;
pub mod zip_reader;

pub use base_archive::{ArchiveEntryInfo, BaseArchive};
use mml_sys::path_helper;

/// 压缩包进度回调句柄，`None` 表示不通知 UI
pub type BaseArchiveGui = Option<Arc<dyn IBaseArchiveGui>>;

/// 压缩包处理显示回调
pub trait IBaseArchiveGui: Send + Sync {
    /// 开始处理压缩包
    ///
    /// - `total`: 总计需要处理的数量
    fn start(&self, total: usize);
    /// 更新压缩包处理信息
    ///
    /// - `filename`: 当前文件名
    /// - `current`: 当前处理数量
    fn update(&self, filename: Option<String>, current: usize);
    /// 解压完成
    fn done(&self);
    /// 条目名含非法字符时询问用户是否同意替换
    ///
    /// - `name`: 原始条目名
    ///
    /// # 返回值
    ///
    /// `true` 表示同意替换，`false` 时调用方返回 `TaskCancel`
    fn file_rename(&self, name: &str) -> bool;
}

/// 压缩包类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveType {
    /// Zip 压缩包，含 .jar / .mrpack
    Zip,
    /// 7z 压缩包
    R7Z,
    /// 未压缩 tar 包
    Tar,
    /// gzip 压缩的 tar 包
    TarGz,
    /// xz 压缩的 tar 包
    TarXz,
}

impl ArchiveType {
    /// 根据文件路径后缀自动检测压缩包类型
    ///
    /// - `path`: 待检测的文件路径
    ///
    /// # 返回值
    ///
    /// 后缀不受支持时返回 `None`
    pub fn try_from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let file_name = path.as_ref().file_name()?.to_string_lossy().to_lowercase();

        if file_name.ends_with(names::ZIP_DOT_EXT)
            || file_name.ends_with(names::MRPACK_DOT_EXT)
            // jar 本质是 zip，客户端 jar、mod jar 都走这条分支
            || file_name.ends_with(names::JAR_DOT_EXT)
        {
            // Modrinth 整合包 .mrpack 本质是 zip
            Some(ArchiveType::Zip)
        } else if file_name.ends_with(names::R7Z_DOT_EXT) {
            Some(ArchiveType::R7Z)
        } else if file_name.ends_with(names::TAR_GZ_DOT_EXT)
            || file_name.ends_with(names::TGZ_DOT_EXT)
        {
            Some(ArchiveType::TarGz)
        } else if file_name.ends_with(names::TAR_XZ_DOT_EXT)
            || file_name.ends_with(names::TXZ_DOT_EXT)
        {
            Some(ArchiveType::TarXz)
        } else if file_name.ends_with(names::TAR_EXT) {
            Some(ArchiveType::Tar)
        } else {
            None
        }
    }
}

/// 压缩模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarMode {
    /// gzip 压缩
    Gz,
    /// xz 压缩
    Xz,
}

impl TarMode {
    /// 根据文件名后缀自动判断压缩模式
    ///
    /// - `path`: 待判断的文件路径
    ///
    /// # 返回值
    ///
    /// 后缀不受支持时返回 `None`
    pub fn try_from_path(path: &Path) -> Option<Self> {
        let file_name = path.file_name()?.to_string_lossy().to_lowercase();

        if file_name.ends_with(names::TAR_GZ_DOT_EXT) || file_name.ends_with(names::TGZ_DOT_EXT) {
            Some(TarMode::Gz)
        } else if file_name.ends_with(names::TAR_XZ_DOT_EXT)
            || file_name.ends_with(names::TXZ_DOT_EXT)
        {
            Some(TarMode::Xz)
        } else {
            None
        }
    }
}

/// 压缩/解压进度追踪器，各格式读取器共用
pub(crate) struct ArchiveProcess {
    /// 进度回调，`None` 表示不通知 UI
    gui: BaseArchiveGui,
    /// 总条目数
    size: AtomicUsize,
    /// 已处理条目数
    now: AtomicUsize,
}

impl ArchiveProcess {
    /// 创建进度追踪器
    ///
    /// - `gui`: 进度回调，`None` 表示不通知 UI
    pub fn new(gui: BaseArchiveGui) -> Self {
        Self {
            gui,
            size: AtomicUsize::new(0),
            now: AtomicUsize::new(0),
        }
    }

    /// 设置总条目数并通知 UI 开始
    ///
    /// - `count`: 总条目数
    pub fn set_count(&self, count: usize) {
        self.size.store(count, Ordering::SeqCst);
        if let Some(gui) = &self.gui {
            gui.start(count);
        }
    }

    /// 已处理计数加一，并通知 UI 当前条目
    ///
    /// - `path`: 刚处理完的条目路径
    pub fn add_now(&self, path: &PathBuf) {
        let now = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(gui) = &self.gui {
            let filename = path.display().to_string();
            gui.update(Some(filename), now);
        }
    }

    /// 通知 UI 全部处理完成
    pub fn done(&self) {
        if let Some(gui) = &self.gui {
            gui.done();
        }
    }

    /// 检查条目名是否含非法字符，非法时替换为 `_` 并询问 GUI 是否同意
    ///
    /// - `name`: 待检查的条目名
    ///
    /// # 返回值
    ///
    /// 名字合法时原样返回；GUI 不同意替换时返回 `TaskCancel`
    pub fn check_name(&self, name: &str) -> CoreResult<String> {
        if name
            .split(['/', '\\'])
            .filter(|seg| !seg.is_empty() && *seg != "." && *seg != "..")
            .all(|seg| !path_helper::file_has_invalid_chars(seg))
        {
            return Ok(name.to_string());
        }
        // 非法字符替换为 `_`，GUI 同意则使用，不同意则取消
        let safe_name = replace_invalid_name(name);
        match &self.gui {
            Some(gui) if gui.file_rename(name) => Ok(safe_name),
            Some(_) => Err(ErrorType::TaskCancel),
            None => Ok(safe_name),
        }
    }
}

/// 已打开的压缩包读取句柄，统一各格式的查文件与单文件解压
///
/// 文件句柄在打开后保持持有，各格式内部决定如何复用：zip 缓存 [`zip::ZipArchive`]，
/// tar / 7z 每次操作从持有的文件句柄克隆出独立文件描述符再重建读取器
pub(crate) trait ArchiveHandle: Send {
    /// 读取压缩包内全部条目
    ///
    /// # 返回值
    ///
    /// 返回条目元信息列表，读取失败时返回相应错误
    fn read_entries(&mut self) -> CoreResult<Vec<ArchiveEntryInfo>>;
    /// 读取单个条目内容到内存
    ///
    /// - `name`: 条目在压缩包内的名称/路径
    ///
    /// # 返回值
    ///
    /// 返回条目原始字节，条目不存在或解压失败时返回相应错误
    fn read(&mut self, name: &str) -> CoreResult<Vec<u8>>;
    /// 流式读取单个条目
    ///
    /// - `name`: 条目在压缩包内的名称/路径
    ///
    /// # 返回值
    ///
    /// 返回只读流，由调用方负责读完或丢弃
    fn read_stream(&mut self, name: &str) -> CoreResult<Box<dyn Read>>;
    /// 提取单个条目到指定路径
    ///
    /// - `name`: 条目在压缩包内的名称/路径
    /// - `output_path`: 目标磁盘路径
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误，成功返回 `Ok(())`
    fn extract_file(
        &mut self,
        name: &str,
        output_path: &Path,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()>;
    /// 就地追加文件，仅支持随机可写的格式
    ///
    /// - `files`: `(磁盘源路径, 压缩包内路径)` 对
    ///
    /// # 返回值
    ///
    /// 不支持的格式返回 `InvalidOperation`
    fn add_files(&mut self, _files: &[(PathBuf, PathBuf)]) -> CoreResult<()> {
        Err(ErrorType::InvalidOperation)
    }
    /// 就地追加内存数据，仅支持随机可写的格式
    ///
    /// - `name`: 条目在压缩包内的名称/路径
    /// - `data`: 文件内容的原始字节
    ///
    /// # 返回值
    ///
    /// 不支持的格式返回 `InvalidOperation`
    fn add_data(&mut self, _name: &str, _data: &[u8]) -> CoreResult<()> {
        Err(ErrorType::InvalidOperation)
    }
}

/// 归一化路径分隔符为 `/`
///
/// - `path`: 待归一化的路径
fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().to_string().replace('\\', "/")
}

/// 检查路径是否匹配任一排除规则
///
/// - `path`: 待检查的文件路径
/// - `patterns`: 排除规则，按子串匹配
fn should_exclude(path: &Path, patterns: &[String]) -> bool {
    let normalized_path = normalize_path(path);
    patterns.iter().any(|pattern| {
        let normalized_pattern = pattern.replace('\\', "/");
        normalized_path.contains(&normalized_pattern)
    })
}

/// 替换条目名中的非法字符为 `_`，按路径段处理并保留目录结构
///
/// - `name`: 待替换的条目名
pub(crate) fn replace_invalid_name(name: &str) -> String {
    name.split(['/', '\\'])
        .map(|seg| path_helper::replace_file_name(seg))
        .collect::<Vec<_>>()
        .join("/")
}

/// 压缩文件，委托给 [`BaseArchive::compress`] 并忽略返回的句柄
///
/// - `archive_type`: 压缩包格式
/// - `archive_file`: 压缩包输出路径
/// - `pack_dir`: 需要打包的源目录
/// - `root_path`: 相对根路径，设置后文件在包内的路径相对于此
/// - `filter`: 可选的排除子串列表，匹配的文件将被跳过
/// - `gui`: 可选的进度回调
///
/// # 返回值
///
/// 打包失败时返回相应错误，成功返回 `Ok(())`
pub fn compress<P: AsRef<Path>>(
    archive_type: ArchiveType,
    archive_file: P,
    pack_dir: P,
    root_path: Option<P>,
    filter: &Option<Vec<String>>,
    gui: BaseArchiveGui,
) -> CoreResult<()> {
    BaseArchive::compress(archive_type, archive_file, pack_dir, root_path, filter, gui)?;
    Ok(())
}

/// 解压文件，委托给 [`BaseArchive::decompress`]
///
/// - `archive_type`: 压缩包格式
/// - `archive_file`: 压缩包文件路径
/// - `output_dir`: 解压输出目录
/// - `gui`: 可选的进度回调
///
/// # 返回值
///
/// 解压失败时返回相应错误，成功返回 `Ok(())`
pub fn decompress<P: AsRef<Path>>(
    archive_type: ArchiveType,
    archive_file: P,
    output_dir: P,
    gui: BaseArchiveGui,
) -> CoreResult<()> {
    BaseArchive::decompress(archive_type, archive_file, output_dir, gui)
}
