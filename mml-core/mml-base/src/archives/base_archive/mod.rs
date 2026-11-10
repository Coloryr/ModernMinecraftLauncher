//! 压缩包处理核心逻辑
//!
//! 实现压缩/解压的统一入口，根据 [`ArchiveType`] 分派到对应格式的读取器。

use std::path::PathBuf;
use std::sync::Mutex;

use crate::archives::{ArchiveHandle, ArchiveType};

mod add;
mod extract;
mod info;
mod open;
mod util;

/// 走 mmap 并行解压的压缩包大小下限，单位字节
///
/// 更小的包线程调度与逐线程解析中央目录的开销盖过解压本身，顺序解压更快
const ZIP_PARALLEL_MIN_SIZE: u64 = 64 * 1024 * 1024;

/// 并行解压最多开几个工作线程
///
/// 每个线程要在内存里解析一份中央目录，线程数并不是越多越好
const ZIP_PARALLEL_MAX_THREADS: usize = 8;

/// 压缩包条目信息
#[derive(Debug, Clone)]
pub struct ArchiveEntryInfo {
    /// 条目名称，即压缩包内路径
    pub name: String,
    /// 是否为目录条目
    pub is_dir: bool,
    /// 未压缩大小，单位字节；目录条目恒为 `0`
    pub size: u64,
}

/// 统一的压缩包处理器，自动检测格式并提供读写功能
///
/// # 示例
///
/// ```ignore
/// use mml_base::archives::BaseArchive;
///
/// // 打开压缩包，根据后缀自动检测类型
/// let archive = BaseArchive::open("path/to/file.zip").unwrap();
///
/// // 遍历条目
/// for entry in archive.entries() {
///     println!("{} ({} bytes)", entry.name, entry.size);
/// }
///
/// // 提取单个文件
/// archive.extract_file("readme.txt", "output/readme.txt", None).unwrap();
///
/// // 提取全部文件
/// archive.extract_all("output_dir/", None, None, None).unwrap();
/// ```
pub struct BaseArchive {
    /// 压缩包在磁盘上的路径
    pub(super) path: PathBuf,
    /// 打开时按文件名后缀判定出的压缩包格式
    pub(super) archive_type: ArchiveType,
    /// 打开时读取并缓存的条目列表，增删后同步刷新
    pub(super) entries: Vec<ArchiveEntryInfo>,
    /// 已打开的读取句柄，各格式实现见对应读取器
    pub(super) handle: Mutex<Box<dyn ArchiveHandle>>,
}

/// 一条待提取任务，供批量解压的串行与并行路径共用
///
/// 目标路径已在收集阶段由 [`BaseArchive::resolve_output_path`] 解析完毕，
/// 提取时直接落盘，不再参与路径计算。
#[derive(Clone)]
pub(super) struct ExtractTask {
    /// 条目在压缩包内的名称/路径，如 `assets/a.txt`
    pub(super) name: String,
    /// 条目解包后写入的目标磁盘路径，已含重命名与非法字符处理结果
    pub(super) output_path: PathBuf,
}

#[cfg(test)]
#[path = "../../../tests/unit/archives_base_archive_tests.rs"]
mod tests;
