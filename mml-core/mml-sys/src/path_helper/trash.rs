//! 移入回收站
//!
//! Windows 走 Shell 文件操作，Linux 走 Trash 目录，macOS 走 Finder。

#[cfg(any(target_os = "linux", test))]
use std::env;
#[cfg(any(target_os = "macos", test))]
use std::process::Command;
#[cfg(any(target_os = "linux", test))]
use std::time::UNIX_EPOCH;

use std::path::Path;

#[cfg(any(not(windows), test))]
use std::fs;
#[cfg(any(not(windows), test))]
use std::path::PathBuf;

use mml_names::i18_items::error_type::CoreResult;
#[cfg(any(target_os = "windows", target_os = "macos", test))]
use mml_names::i18_items::error_type::ErrorType;

#[cfg(any(not(windows), test))]
use super::fs_error;

/// 获取回收站路径
///
/// # 返回值
///
/// 返回回收站 files 目录路径，不存在时自动创建
#[cfg(any(target_os = "linux", test))]
pub(super) fn get_trash_files_path() -> PathBuf {
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
pub(super) fn get_trash_info_path() -> PathBuf {
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
        use mml_names::i18_items::error_type::{ErrorType, MoveToTrashErrorData};

        Err(ErrorType::MoveToTrashError(MoveToTrashErrorData {
            path: dir.as_ref().to_path_buf(),
            code: 0,
        }))
    }
}

/// 将文件或目录移入 Linux 回收站，并写入恢复信息
#[cfg(any(target_os = "linux", test))]
pub(super) fn move_to_trash_linux(dir: &Path) -> CoreResult<()> {
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
pub(super) fn move_to_trash_macos(dir: &Path) -> CoreResult<()> {
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
pub(super) fn move_to_trash_windows<P: AsRef<Path>>(dir: P) -> CoreResult<()> {
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
