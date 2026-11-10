//! 硬链接与文件标识
//!
//! 按平台取链接数与文件 id，供资源链接判断是否同一份文件。

use std::fs;
use std::path::Path;

/// 文件身份，同一份数据的不同硬链接共享同一个值
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId {
    /// 卷标识：Windows 是卷序列号，Unix 是设备号
    pub volume: u64,
    /// 文件索引：Windows 是 file index，Unix 是 inode
    pub index: u64,
}

/// 文件被几个路径指向，即硬链接数
///
/// 只有链接数大于 1 才是真正意义上的硬链接；拷贝出来的文件恒为 1。
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 链接数；平台不支持或读取失败时返回 `None`
#[cfg(windows)]
pub fn link_count<P: AsRef<Path>>(path: P) -> Option<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
    };

    let file = fs::File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut info).ok()?;
    }
    Some(info.nNumberOfLinks)
}

/// 文件被几个路径指向，即硬链接数
///
/// 只有链接数大于 1 才是真正意义上的硬链接；拷贝出来的文件恒为 1。
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 链接数；平台不支持或读取失败时返回 `None`
#[cfg(unix)]
pub fn link_count<P: AsRef<Path>>(path: P) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;

    fs::metadata(path).ok().map(|meta| meta.nlink() as u32)
}

/// 文件被几个路径指向，即硬链接数
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 当前平台不支持时返回 `None`
#[cfg(not(any(windows, unix)))]
pub fn link_count<P: AsRef<Path>>(_path: P) -> Option<u32> {
    None
}

/// 读取文件身份
///
/// 硬链接的不同路径会得到相同结果，可用来判断两个文件是否共享同一份数据。
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 平台不支持或读取失败时返回 `None`
#[cfg(windows)]
pub fn file_id<P: AsRef<Path>>(path: P) -> Option<FileId> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
    };

    let file = fs::File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut info).ok()?;
    }

    Some(FileId {
        volume: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

/// 读取文件身份
///
/// 硬链接的不同路径会得到相同结果，可用来判断两个文件是否共享同一份数据。
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 平台不支持或读取失败时返回 `None`
#[cfg(unix)]
pub fn file_id<P: AsRef<Path>>(path: P) -> Option<FileId> {
    use std::os::unix::fs::MetadataExt;

    let meta = fs::metadata(path).ok()?;
    Some(FileId {
        volume: meta.dev(),
        index: meta.ino(),
    })
}

/// 读取文件身份
///
/// 当前平台不支持时返回 `None`，调用方应退回按路径统计。
///
/// - `path`: 文件路径
///
/// # 返回值
///
/// 恒为 `None`
#[cfg(not(any(windows, unix)))]
pub fn file_id<P: AsRef<Path>>(_path: P) -> Option<FileId> {
    None
}
