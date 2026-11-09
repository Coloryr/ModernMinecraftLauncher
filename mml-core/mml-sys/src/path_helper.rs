//! 文件与路径处理模块
//!
//! 提供跨平台的文件系统操作封装，包括：
//!
//! - **基础 IO** — 文件读写、目录创建、复制与移动，同步与异步各一份
//! - **权限管理** — Unix chmod / Java 权限修复
//! - **回收站操作** — Windows (SHFileOperationW) / Linux (Trash) / macOS (osascript Finder)
//! - **文件搜索** — 递归搜索、目录遍历、大小统计
//! - **路径安全** — 非法字符过滤、文件名/路径名清理
//!
//! 跨平台
//!
//! 所有函数均提供同步版与 Tokio 异步版。
//! 回收站操作自动根据编译目标选择 Windows/Linux/macOS 实现。

#[cfg(any(target_os = "linux", test))]
use std::env;
#[cfg(any(not(windows), test))]
use std::process::Command;
#[cfg(not(windows))]
use std::process::Stdio;
use std::time::SystemTime;
#[cfg(any(target_os = "linux", test))]
use std::time::UNIX_EPOCH;

use std::fs::{self};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use tokio::fs as tfs;

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType, FileSystemErrorData};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

mod safe_path;
pub use safe_path::{safe_join, safe_join_with_root};

/// 把底层 io 错误包成带路径的文件系统错误
///
/// - `path`: 出错的路径，用来定位是哪个文件
/// - `err`: 底层错误，原文直接进 `error` 字段
fn fs_error(path: &Path, err: impl std::fmt::Display) -> ErrorType {
    ErrorType::FileSystemError(FileSystemErrorData {
        path: path.to_path_buf(),
        error: err.to_string(),
    })
}

/// 提升权限
///
/// - `path`: 要加上可执行位的文件路径
///
/// # 返回值
///
/// 命令执行成功返回 `Ok(())`，失败时返回相应错误
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn chmod(path: &str) -> io::Result<()> {
    let mut child = Command::new("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        writeln!(stdin, "chmod a+x {}", path)?;
        writeln!(stdin, "exit")?;
    }

    child.wait()?;
    Ok(())
}

/// 提升 Java 文件夹权限
///
/// - `path`: Java 可执行文件路径，按它推出要放权的目录
///
/// # 返回值
///
/// 命令执行成功返回 `Ok(())`，失败时返回相应错误
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn per_java_chmod(path: &str) -> io::Result<()> {
    let mut child = Command::new("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let info = Path::new(path);
    if let (Some(parent), Some(grandparent)) =
        (info.parent(), info.parent().and_then(|p| p.parent()))
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            writeln!(stdin, "chmod a+x {}/*", parent.display())?;
            writeln!(stdin, "chmod a+x {}/lib/*", grandparent.display())?;
            writeln!(stdin, "exit")?;
        }
    }

    child.wait()?;
    Ok(())
}

/// 获取回收站路径
///
/// # 返回值
///
/// 返回回收站 files 目录路径，不存在时自动创建
#[cfg(any(target_os = "linux", test))]
fn get_trash_files_path() -> PathBuf {
    let data_home = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(&home).join(".local/share")
        });

    let trash_path = data_home.join("Trash/files");
    fs::create_dir_all(&trash_path).ok();
    trash_path
}

/// 获取回收站路径
///
/// # 返回值
///
/// 返回回收站 info 目录路径，不存在时自动创建
#[cfg(any(target_os = "linux", test))]
fn get_trash_info_path() -> PathBuf {
    let data_home = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(&home).join(".local/share")
        });

    let trash_info_path = data_home.join("Trash/info");
    fs::create_dir_all(&trash_info_path).ok();
    trash_info_path
}

/// 将文件或目录移入系统回收站
///
/// - `dir`: 要移入回收站的文件或目录路径
///
/// # 返回值
///
/// 路径不存在时直接返回 `Ok(())`；挪入回收站成功返回 `Ok(())`；
/// 用户中止返回 `ErrorType::TaskCancel`，其他失败返回相应错误
pub fn move_to_trash<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    if !dir.as_ref().exists() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        move_to_trash_windows(dir)
    }

    #[cfg(target_os = "linux")]
    {
        move_to_trash_linux(dir.as_ref())
    }

    #[cfg(target_os = "macos")]
    {
        move_to_trash_macos(dir.as_ref())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        use mml_names::i18_items::error_type::MoveToTrashErrorData;

        Err(ErrorType::MoveToTrashError(MoveToTrashErrorData {
            path: dir.as_ref().to_path_buf(),
            code: 0,
        }))
    }
}

/// 将文件或目录移入 Linux 回收站，并写入恢复信息
#[cfg(any(target_os = "linux", test))]
fn move_to_trash_linux(dir: &Path) -> CoreResult<()> {
    use std::time::SystemTime;

    let trash_files_path = get_trash_files_path();
    let trash_info_path = get_trash_info_path();

    let file_name = Path::new(dir).file_name().unwrap_or_default();
    let mut dest_path = trash_files_path.join(file_name);
    let mut trash_info_file =
        trash_info_path.join(format!("{}.trashinfo", file_name.to_string_lossy()));

    let mut counter = 1;
    while dest_path.exists() {
        let name = Path::new(file_name);
        let name_without_ext = name.file_stem().unwrap_or_default();
        let ext = name.extension().unwrap_or_default();

        let new_name = if ext.is_empty() {
            format!("{}_{}", name_without_ext.to_string_lossy(), counter)
        } else {
            format!(
                "{}_{}.{}",
                name_without_ext.to_string_lossy(),
                counter,
                ext.to_string_lossy()
            )
        };

        dest_path = trash_files_path.join(&new_name);
        trash_info_file = trash_info_path.join(format!("{}.trashinfo", new_name));
        counter += 1;
    }

    // 生成回收站信息文件内容
    let deletion_date = SystemTime::now();
    let datetime_str = format!(
        "{:?}",
        deletion_date.duration_since(UNIX_EPOCH).unwrap_or_default()
    );
    let trash_info_content = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        dir.display(),
        datetime_str
    );

    fs::write(&trash_info_file, trash_info_content)
        .map_err(|err| fs_error(&trash_info_file, err))?;

    // 移动文件或目录
    if let Err(err) = fs::rename(dir, &dest_path) {
        let _ = fs::remove_file(&trash_info_file);
        return Err(fs_error(dir, err));
    }

    Ok(())
}

/// 通过 Finder 将文件或目录移入 macOS 回收站
#[cfg(any(target_os = "macos", test))]
fn move_to_trash_macos(dir: &Path) -> CoreResult<()> {
    use mml_names::i18_items::error_type::MoveToTrashErrorData;

    let escaped_dir = dir
        .to_str()
        .ok_or_else(|| {
            ErrorType::MoveToTrashError(MoveToTrashErrorData {
                path: dir.to_path_buf(),
                code: 0,
            })
        })?
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");

    let apple_script = format!(
        "tell application \"Finder\" to delete POSIX file \"{}\"",
        escaped_dir
    );

    let output = Command::new("osascript")
        .args(["-e", &apple_script])
        .output()
        .map_err(|err| fs_error(dir, err))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(ErrorType::MoveToTrashError(MoveToTrashErrorData {
            path: dir.to_path_buf(),
            code: 0,
        }))
    }
}

/// 通过 Windows Shell 将文件或目录移入回收站
#[cfg(target_os = "windows")]
fn move_to_trash_windows<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
    use mml_names::i18_items::error_type::MoveToTrashErrorData;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;
    use windows::Win32::System::Com::COINIT_MULTITHREADED;
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::CoUninitialize;
    use windows::Win32::UI::Shell::FO_DELETE;
    use windows::Win32::UI::Shell::FOF_ALLOWUNDO;
    use windows::Win32::UI::Shell::FOF_NOCONFIRMATION;
    use windows::Win32::UI::Shell::FOF_NOERRORUI;
    use windows::Win32::UI::Shell::FOF_SILENT;
    use windows::Win32::UI::Shell::SHFILEOPSTRUCTW;
    use windows::Win32::UI::Shell::SHFileOperationW;
    use windows::core::BOOL;
    use windows::core::PCWSTR;

    // SHFileOperationW 要求调用线程能处理窗口消息，在 GUI 主线程 STA 上直接调用会死锁；
    // 放到独立工作线程执行，与已验证正常的普通线程环境一致。
    let dir: std::path::PathBuf = dir.as_ref().to_path_buf();
    let err_path = dir.clone();
    let (result, aborted) = std::thread::spawn(move || -> (i32, bool) {
        // Shell 删除要求调用线程已初始化 COM，没初始化时 SHFileOperationW 返回 0x8
        // 且什么都不删；受限环境里 STA 会被拒，退回 MTA 同样能删进回收站。
        let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok()
            || unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();

        // pFrom 要求双 \0 结尾的路径列表，HSTRING 只有一个 \0 且结尾字节取自堆内存，
        // 会让 SHFileOperationW 间歇性返回 0x2；这里手动补足双 \0。
        let mut buf: Vec<u16> = dir.as_os_str().encode_wide().collect();
        buf.push(0);
        buf.push(0);
        let pcwstr = PCWSTR(buf.as_ptr());

        let mut operation = SHFILEOPSTRUCTW {
            hwnd: HWND::default(),
            wFunc: FO_DELETE,
            pFrom: pcwstr,
            pTo: PCWSTR::null(),
            fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI).0 as u16,
            fAnyOperationsAborted: BOOL::from(false),
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: PCWSTR::null(),
        };

        let result = unsafe { SHFileOperationW(&mut operation) };
        let aborted = operation.fAnyOperationsAborted.as_bool();
        // 线程是刚建的，初始化成功就该配对释放
        if com {
            unsafe { CoUninitialize() };
        }
        (result, aborted)
    })
    .join()
    .unwrap_or((-1, false));

    // 用户中止时返回码不定，先看 fAnyOperationsAborted，否则会把"取消"报成文件系统错误
    if aborted {
        return Err(ErrorType::TaskCancel);
    }

    // 返回码：0 成功；0x71 用户取消，未移动任何内容；其他值出错
    match result {
        0 => Ok(()),
        0x71 => Err(ErrorType::TaskCancel), // 用户已取消
        _ => Err(ErrorType::MoveToTrashError(MoveToTrashErrorData {
            path: err_path,
            code: result,
        })),
    }
}

/// 检查非法名字
///
/// - `name`: 需要检查的名字
///
/// # 返回值
///
/// 名字非法时返回 true：空、全点号、超 80 字节或含非法字符
pub fn file_has_invalid_chars(name: &str) -> bool {
    if name.is_empty() || name.chars().all(|c| c == '.') {
        return true;
    }

    if name.len() > 80 {
        return true;
    }

    name.contains(&['<', '>', ':', '"', '/', '\\', '|', '?', '*'][..])
}

/// 获取所有文件
///
/// - `local`: 递归搜索的根目录
///
/// # 返回值
///
/// 返回递归收集的文件路径列表
pub fn get_all_files<P: AsRef<Path>>(local: P) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(local) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                files.extend(get_all_files(&path));
            }
        }
    }

    files
}

/// 获取当前目录所有文件
///
/// - `path`: 需要获取的路径
///
/// # 返回值
///
/// 返回当前目录下的文件路径列表，不含子目录
pub fn get_files<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                files.push(entry.path());
            }
        }
    }

    files
}

/// 获取文件夹下面最后写入的文件
///
/// - `path`: 获取的目录
///
/// # 返回值
///
/// 返回最近写入的文件路径，目录为空时返回 `Ok(None)`
pub fn get_last_written_file<P: AsRef<Path>>(path: P) -> CoreResult<Option<PathBuf>> {
    let entries = fs::read_dir(&path).map_err(|err| fs_error(path.as_ref(), err))?;

    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|err| fs_error(path.as_ref(), err))?;
        let path = entry.path();

        if path.is_file() {
            let metadata = fs::metadata(&path).map_err(|err| fs_error(&path, err))?;
            if let Ok(modified) = metadata.modified() {
                files.push((path, modified));
            }
        }
    }

    files.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(files.first().map(|(path, _)| path.clone()))
}

/// 获取目录占用大小
///
/// - `path`: 需要获取的路径
///
/// # 返回值
///
/// 返回目录总大小，单位字节
pub fn get_folder_size<P: AsRef<Path>>(path: P) -> u64 {
    let mut size = 0;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                size += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                size += get_folder_size(&path);
            }
        }
    }

    size
}

/// 获取当前目录所有目录
///
/// - `path`: 需要获取的路径
///
/// # 返回值
///
/// 返回当前目录下的子目录路径列表
pub fn get_dirs<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }

    dirs
}

/// 复制文件
///
/// - `input`: 源文件路径
/// - `output`: 输出文件路径
///
/// # 返回值
///
/// 复制成功返回 `Ok(())`，失败返回相应错误
pub fn copy_file<P: AsRef<Path>>(input: P, output: P) -> CoreResult<()> {
    fs::copy(&input, output).map_err(|err| fs_error(input.as_ref(), err))?;
    Ok(())
}

/// 创建文件硬链接
///
/// - `input`: 源文件路径
/// - `output`: 新链接路径
///
/// # 返回值
///
/// 创建成功返回 `Ok(())`，失败返回相应错误
pub fn hard_link<P: AsRef<Path>>(input: P, output: P) -> CoreResult<()> {
    fs::hard_link(input.as_ref(), output.as_ref()).map_err(|err| fs_error(input.as_ref(), err))?;
    Ok(())
}

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

/// 异步复制文件
///
/// - `input`: 源文件路径
/// - `output`: 输出文件路径
///
/// # 返回值
///
/// 复制成功返回 `Ok(())`，失败返回相应错误
pub async fn copy_file_async<P: AsRef<Path>>(input: P, output: P) -> CoreResult<()> {
    tfs::copy(&input, output)
        .await
        .map_err(|err| fs_error(input.as_ref(), err))?;
    Ok(())
}

/// 搬运文件
///
/// - `input`: 源文件路径
/// - `output`: 输出文件路径
///
/// # 返回值
///
/// 移动成功返回 `Ok(())`；跨设备时自动回退为复制后删除，失败返回相应错误
pub fn move_file<P: AsRef<Path>>(input: P, output: P) -> CoreResult<()> {
    if let Some(parent) = output.as_ref().parent() {
        create_dir_all(parent)?;
    }

    match fs::rename(&input, &output) {
        Ok(_) => return Ok(()),
        Err(err) => {
            if err.kind() == io::ErrorKind::CrossesDevices {
                copy_file(&input, &output)?;
                delete(&input)?;
                return Ok(());
            }
            return Err(fs_error(input.as_ref(), err));
        }
    }
}

/// 异步搬运文件
///
/// - `input`: 源文件路径
/// - `output`: 输出文件路径
///
/// # 返回值
///
/// 移动成功返回 `Ok(())`；跨设备时自动回退为复制后删除，失败返回相应错误
pub async fn move_file_async<P: AsRef<Path>>(input: P, output: P) -> CoreResult<()> {
    if let Some(parent) = output.as_ref().parent() {
        create_dir_all(parent)?;
    }

    match tfs::rename(&input, &output).await {
        Ok(_) => return Ok(()),
        Err(err) => {
            if err.kind() == io::ErrorKind::CrossesDevices {
                copy_file(&input, &output)?;
                delete(&input)?;
                return Ok(());
            }
            return Err(fs_error(input.as_ref(), err));
        }
    }
}

/// 复制文件夹
///
/// - `input`: 源目录
/// - `output`: 输出目录
///
/// # 返回值
///
/// 复制成功返回 `Ok(())`，失败返回相应错误
pub fn copy_dir<P: AsRef<Path>>(input: P, output: P) -> CoreResult<()> {
    create_dir_all(&output)?;

    for entry in fs::read_dir(&input).map_err(|err| fs_error(input.as_ref(), err))? {
        let entry = entry.map_err(|err| {
            ErrorType::FileReadError(ErrorData {
                error: err.to_string(),
            })
        })?;
        let path = entry.path();
        let dest_path = output.as_ref().join(entry.file_name());

        if path.is_dir() {
            copy_dir(&path, &dest_path)?;
        } else {
            copy_file(&path, &dest_path)?;
        }
    }

    Ok(())
}

/// 异步复制文件夹
///
/// - `from`: 源目录
/// - `to`: 输出目录
///
/// # 返回值
///
/// 复制成功返回 `Ok(())`，失败返回相应错误
pub async fn copy_dir_async<P: AsRef<Path>>(from: P, to: P) -> CoreResult<()> {
    create_dir_all(&to)?;

    let mut dir = tfs::read_dir(&from)
        .await
        .map_err(|err| fs_error(from.as_ref(), err))?;

    loop {
        let item = dir.next_entry().await.map_err(|err| {
            ErrorType::FileReadError(ErrorData {
                error: err.to_string(),
            })
        })?;

        if item.is_none() {
            break;
        }

        let entry = item.unwrap();
        let path = entry.path();
        let dest_path = to.as_ref().join(entry.file_name());

        if path.is_dir() {
            copy_dir_async(&path, &dest_path).await?;
        } else {
            copy_file_async(&path, &dest_path).await?;
        }
    }

    Ok(())
}

/// 查找文件
///
/// - `path`: 查找的目录
/// - `name`: 查找的文件名
///
/// # 返回值
///
/// 返回首个同名文件的路径，未找到时返回 `None`
pub fn search_file<P: AsRef<Path>>(path: P, name: &str) -> Option<PathBuf> {
    let files = get_all_files(path);
    files.into_iter().find(|f| f.file_name().unwrap() == name)
}

/// 读文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回只读文件句柄，打开失败返回相应错误
pub fn open_read<P: AsRef<Path>>(file: P) -> CoreResult<fs::File> {
    match fs::File::open(&file) {
        Ok(ok) => Ok(ok),
        Err(err) => Err(fs_error(file.as_ref(), err)),
    }
}

/// 以读写方式打开已存在的文件，不创建、不截断
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回读写文件句柄，打开失败返回相应错误
pub fn open_read_write<P: AsRef<Path>>(file: P) -> CoreResult<fs::File> {
    match fs::OpenOptions::new().read(true).write(true).open(&file) {
        Ok(ok) => Ok(ok),
        Err(err) => Err(fs_error(file.as_ref(), err)),
    }
}

/// 异步打开只读文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回异步只读文件句柄，打开失败返回相应错误
pub async fn open_read_async<P: AsRef<Path>>(file: P) -> CoreResult<tfs::File> {
    match tfs::File::open(&file).await {
        Ok(ok) => Ok(ok),
        Err(err) => Err(fs_error(file.as_ref(), err)),
    }
}

/// 写文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回写入文件句柄，父目录自动创建、写前截断；打开失败返回相应错误
pub fn open_write<P: AsRef<Path>>(file: P) -> CoreResult<fs::File> {
    if let Some(parent) = file.as_ref().parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return Err(fs_error(file.as_ref(), err));
        }
    }

    Ok(fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        // 整文件重写语义：新内容比旧文件短时必须截断，
        // 否则残留旧尾巴会污染 JSON 等结构化文件
        .truncate(true)
        .open(&file)
        .map_err(|err| fs_error(file.as_ref(), err))?)
}

/// 异步写文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回写入文件句柄，父目录自动创建、写前截断；打开失败返回相应错误
pub async fn open_write_async<P: AsRef<Path>>(file: P) -> CoreResult<tfs::File> {
    if let Some(parent) = file.as_ref().parent() {
        create_dir_all(parent)?;
    }

    Ok(tfs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        // 与 open_write 一致：整文件重写语义，写前截断
        .truncate(true)
        .open(&file)
        .await
        .map_err(|err| fs_error(file.as_ref(), err))?)
}

/// 创建所有目录
///
/// - `path`: 目录
///
/// # 返回值
///
/// 创建成功或目录已存在均返回 `Ok(())`，失败返回相应错误
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> CoreResult<()> {
    match fs::create_dir_all(&path) {
        Ok(_) => Ok(()),
        Err(err) => Err(fs_error(path.as_ref(), err)),
    }
}

/// 继续写文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回追加写文件句柄，父目录与文件不存在时自动创建；打开失败返回相应错误
pub fn open_append<P: AsRef<Path>>(file: P) -> CoreResult<fs::File> {
    if let Some(parent) = file.as_ref().parent() {
        create_dir_all(parent)?;
    }
    Ok(fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|err| fs_error(file.as_ref(), err))?)
}

/// 写文本
///
/// - `file`: 文件路径
/// - `text`: 文本内容
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`，失败返回相应错误
pub fn write_text<P: AsRef<Path>>(file: P, text: &str) -> CoreResult<()> {
    let mut stream = open_write(&file)?;
    stream
        .write_all(text.as_bytes())
        .map_err(|err| fs_error(file.as_ref(), err))?;

    Ok(())
}

/// 异步写文本
///
/// - `file`: 文件路径
/// - `text`: 文本内容
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`，失败返回相应错误
pub async fn write_text_async<P: AsRef<Path>>(file: P, text: String) -> CoreResult<()> {
    let mut stream = open_write_async(&file).await?;

    stream
        .write_all(text.as_bytes())
        .await
        .map_err(|err| fs_error(file.as_ref(), err))?;
    // tokio 的 File 自己带缓冲，不 flush 时数据还停在用户态，
    // 紧接着用同步 API 读同一个文件会读不到刚写的内容
    stream
        .flush()
        .await
        .map_err(|err| fs_error(file.as_ref(), err))?;

    Ok(())
}

/// 读文本
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回文件文本内容，读取失败返回相应错误
pub fn read_text<P: AsRef<Path>>(file: P) -> CoreResult<String> {
    let mut stream = open_read(&file)?;
    let mut content = String::new();
    stream
        .read_to_string(&mut content)
        .map_err(|err| fs_error(file.as_ref(), err))?;
    Ok(content)
}

/// 异步读文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回文件文本内容，读取失败返回相应错误
pub async fn read_text_async<P: AsRef<Path>>(file: P) -> CoreResult<String> {
    let mut stream = open_read_async(&file).await?;
    let mut content = String::new();
    stream
        .read_to_string(&mut content)
        .await
        .map_err(|err| fs_error(file.as_ref(), err))?;
    Ok(content)
}

/// 读取字节数据
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回文件字节内容，读取失败返回相应错误
pub fn read_byte<P: AsRef<Path>>(file: P) -> CoreResult<Vec<u8>> {
    let mut stream = open_read(&file)?;
    let mut buffer = Vec::new();
    stream
        .read_to_end(&mut buffer)
        .map_err(|err| fs_error(file.as_ref(), err))?;
    Ok(buffer)
}

/// 异步读取字节数据
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 返回文件字节内容，读取失败返回相应错误
pub async fn read_byte_async<P: AsRef<Path>>(file: P) -> CoreResult<Vec<u8>> {
    let mut stream = open_read(&file)?;
    let mut buffer = Vec::new();
    stream
        .read_to_end(&mut buffer)
        .map_err(|err| fs_error(file.as_ref(), err))?;
    Ok(buffer)
}

/// 删除文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 目标是文件时删除并返回 `Ok(())`，目录不做任何事；删除失败返回相应错误
pub fn delete<P: AsRef<Path>>(file: P) -> CoreResult<()> {
    if file.as_ref().is_file() {
        fs::remove_file(&file).map_err(|err| fs_error(file.as_ref(), err))?;
    }
    Ok(())
}

/// 异步删除文件
///
/// - `file`: 文件路径
///
/// # 返回值
///
/// 目标是文件时删除并返回 `Ok(())`，目录不做任何事；删除失败返回相应错误
pub async fn delete_async<P: AsRef<Path>>(file: P) -> CoreResult<()> {
    if file.as_ref().is_file() {
        tfs::remove_file(&file)
            .await
            .map_err(|err| fs_error(file.as_ref(), err))?;
    }
    Ok(())
}

/// 写文件
///
/// - `file`: 文件路径
/// - `data`: 数据
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`，失败返回相应错误
pub fn write_bytes<P: AsRef<Path>>(file: P, data: &[u8]) -> CoreResult<()> {
    let mut stream = open_write(&file)?;
    stream
        .write_all(data)
        .map_err(|err| fs_error(file.as_ref(), err))
}

/// 异步写文件
///
/// - `file`: 文件路径
/// - `data`: 数据
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`，失败返回相应错误
pub async fn write_bytes_async<P: AsRef<Path>>(file: P, data: &[u8]) -> CoreResult<()> {
    let mut stream = open_write_async(&file).await?;
    stream
        .write_all(data)
        .await
        .map_err(|err| fs_error(file.as_ref(), err))?;
    // 同 write_text_async：不 flush 的话数据还留在 tokio 的缓冲里
    stream
        .flush()
        .await
        .map_err(|err| fs_error(file.as_ref(), err))
}

/// 写文件
///
/// - `file`: 文件路径
/// - `reader`: 数据流
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`，失败返回相应错误
pub fn write_stream<P: AsRef<Path>, R: Read>(file: P, mut reader: R) -> CoreResult<()> {
    let mut stream = open_write(&file)?;
    io::copy(&mut reader, &mut stream).map_err(|err| fs_error(file.as_ref(), err))?;
    Ok(())
}

/// 写文件
///
/// - `path`: 文件路径
/// - `reader`: 数据流
///
/// # 返回值
///
/// 写入成功返回 `Ok(())`，失败返回相应错误
pub async fn write_stream_async<P: AsRef<Path>, R: AsyncRead + Unpin>(
    path: P,
    mut reader: R,
) -> CoreResult<()> {
    let mut stream = open_write_async(&path).await?;
    tokio::io::copy(&mut reader, &mut stream)
        .await
        .map_err(|err| fs_error(path.as_ref(), err))?;
    stream
        .flush()
        .await
        .map_err(|err| fs_error(path.as_ref(), err))?;
    Ok(())
}

/// 替换文件名非法字符
///
/// - `name`: 原文件名
///
/// # 返回值
///
/// 返回把非法字符 `<>:"/\|?*` 与 `\0` 替换为 `_` 后的名字
pub fn replace_file_name(name: &str) -> String {
    name.replace(|c: char| "<>:\"/\\|?*\0".contains(c), "_")
}

/// 替换文件名非法字符
///
/// - `name`: 原路径名
///
/// # 返回值
///
/// 返回替换后的路径名，Windows 额外替换 `<>:"/\|?*`，其他平台仅替换 `\0`
pub fn replace_path_name(name: &str) -> String {
    #[cfg(not(windows))]
    let invalid_chars: Vec<char> = vec!['\0'];

    #[cfg(windows)]
    let invalid_chars: Vec<char> = vec!['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0'];

    name.chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect()
}

#[cfg(test)]
#[path = "../tests/unit/path_helper_tests.rs"]
mod tests;
