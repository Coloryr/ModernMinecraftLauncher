//! 快捷方式创建：指向启动器的实例启动链接

use std::path::{Path, PathBuf};

use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};

/// 创建快捷方式
///
/// - `uuid`: 实例 UUID，作为快捷方式的启动参数
/// - `icon`: 图标文件路径，传 `None` 时用启动器默认图标
/// - `work`: 工作目录
/// - `file`: 快捷方式目标路径，Windows 下为 .lnk 文件
///
/// # 返回值
///
/// 返回快捷方式路径，创建失败时返回相应错误
#[inline(always)]
pub fn create_shortcut<P: AsRef<Path>>(
    uuid: &str,
    icon: Option<P>,
    work: P,
    file: P,
) -> CoreResult<PathBuf> {
    create_shortcut_inner(uuid, icon, work, file)
}

/// Windows 实现：通过 Shell COM 的 IShellLink 写出 .lnk 文件
#[cfg(target_os = "windows")]
fn create_shortcut_inner<P: AsRef<Path>>(
    uuid: &str,
    icon: Option<P>,
    work: P,
    file: P,
) -> CoreResult<PathBuf> {
    unsafe {
        use mml_names::names;
        use windows::Win32::Foundation::S_OK;
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use windows::core::{HSTRING, Interface, PCWSTR};

        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr != S_OK {
            return Err(ErrorType::FileSystemError(FileSystemErrorData {
                error: hr.message(),
                path: file.as_ref().to_path_buf(),
            }));
        }

        let shell_link = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER);

        if let Err(err) = shell_link {
            return Err(ErrorType::FileSystemError(FileSystemErrorData {
                path: file.as_ref().to_path_buf(),
                error: err.message(),
            }));
        }
        let shell_link = shell_link.unwrap();

        let hstring = HSTRING::from(std::env::current_exe().unwrap().as_os_str());
        let pcwstr = PCWSTR(hstring.as_ptr());

        // 目标路径，必须设置
        shell_link.SetPath(pcwstr).map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: file.as_ref().to_path_buf(),
                error: err.message(),
            })
        })?;

        // 启动参数
        let hstring = HSTRING::from(format!("{} {uuid}", names::COMMAND_GAME));
        let pcwstr = PCWSTR(hstring.as_ptr());
        shell_link.SetArguments(pcwstr).map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: file.as_ref().to_path_buf(),
                error: err.message(),
            })
        })?;

        let hstring = HSTRING::from(work.as_ref().as_os_str());
        let pcwstr = PCWSTR(hstring.as_ptr());
        shell_link.SetWorkingDirectory(pcwstr).map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: file.as_ref().to_path_buf(),
                error: err.message(),
            })
        })?;

        // 图标
        if let Some(icon) = icon {
            let hstring = HSTRING::from(icon.as_ref().as_os_str());
            let pcwstr = PCWSTR(hstring.as_ptr());
            shell_link.SetIconLocation(pcwstr, 0).map_err(|err| {
                ErrorType::FileSystemError(FileSystemErrorData {
                    path: file.as_ref().to_path_buf(),
                    error: err.message(),
                })
            })?;
        }

        let persist_file = shell_link.cast::<IPersistFile>().map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: file.as_ref().to_path_buf(),
                error: err.message(),
            })
        })?;

        let hstring = HSTRING::from(file.as_ref().as_os_str());
        let pcwstr = PCWSTR(hstring.as_ptr());
        persist_file.Save(pcwstr, true).map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: file.as_ref().to_path_buf(),
                error: err.message(),
            })
        })?;

        CoUninitialize();
        Ok(file.as_ref().to_path_buf())
    }
}

/// Linux 实现：写一个指向启动器的 .desktop 文件
///
/// 参数含义同 [`create_shortcut`]。
#[cfg(target_os = "linux")]
fn create_shortcut_inner<P: AsRef<Path>>(
    uuid: &str,
    icon: Option<P>,
    work: P,
    file: P,
) -> CoreResult<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    use mml_names::names;

    let exe = std::env::current_exe().map_err(|err| {
        ErrorType::FileSystemError(FileSystemErrorData {
            error: err.to_string(),
            path: file.as_ref().to_path_buf(),
        })
    })?;

    // 显示名取快捷方式文件名，与 Windows 下 .lnk 的表现一致
    let name = file
        .as_ref()
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| uuid.to_string());

    let mut content = String::from("[Desktop Entry]\nType=Application\nTerminal=false\n");
    content.push_str(&format!("Name={name}\n"));
    content.push_str(&format!(
        "Exec=\"{}\" {} {}\n",
        exe.display(),
        names::COMMAND_GAME,
        uuid
    ));
    content.push_str(&format!("Path={}\n", work.as_ref().display()));
    if let Some(icon) = icon {
        content.push_str(&format!("Icon={}\n", icon.as_ref().display()));
    }

    std::fs::write(file.as_ref(), content).map_err(|err| {
        ErrorType::FileSystemError(FileSystemErrorData {
            error: err.to_string(),
            path: file.as_ref().to_path_buf(),
        })
    })?;

    // 桌面环境要求启动器文件带可执行位，否则会被标成"不受信任"
    if let Ok(meta) = std::fs::metadata(file.as_ref()) {
        let mut perm = meta.permissions();
        perm.set_mode(0o755);
        let _ = std::fs::set_permissions(file.as_ref(), perm);
    }

    Ok(file.as_ref().to_path_buf())
}

/// macOS 实现：暂不支持创建快捷方式
///
/// 参数含义同 [`create_shortcut`]。
#[cfg(target_os = "macos")]
fn create_shortcut_inner<P: AsRef<Path>>(
    _uuid: &str,
    _icon: Option<P>,
    _work: P,
    _file: P,
) -> CoreResult<PathBuf> {
    Err(ErrorType::InvalidOperation)
}

/// 其它平台：不支持创建快捷方式
///
/// 参数含义同 [`create_shortcut`]。
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn create_shortcut_inner<P: AsRef<Path>>(
    _uuid: &str,
    _icon: Option<P>,
    _work: P,
    _file: P,
) -> CoreResult<PathBuf> {
    Err(ErrorType::InvalidOperation)
}
