//! 系统通知：启动器不在前台时，把用户看不到的结果推给系统
//!
//! 三个平台都由 tauri-plugin-notification 落到各自的原生形态：Windows 是操作中心的吐司，
//! macOS 是右上角的气泡，Linux 走桌面环境的通知服务。
//!
//! 判据是**启动器有没有窗口在前台**：用户正看着界面时再弹一条系统提示是多余的，
//! 界面上本来就有进度与结果。
//!
//! Windows 上还多一道门槛：未打包的 Win32 程序想弹吐司，必须有一个带
//! `System.AppUserModel.ID` 属性的开始菜单快捷方式，否则 show() 照样返回成功、
//! 系统直接把这条通知丢掉，日志里什么都看不到。安装包会建这个快捷方式，便携版不会，
//! 所以启动时自己补一个，见 [register_start_menu_shortcut]。

use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

use crate::log_text::{LogText, NotifyText};

/// 启动器是否有窗口在前台，有则返回它的标签
///
/// 多窗口模式下任何一个窗口在前台都算：用户正看着某个子窗口时不必再弹系统提示。
/// 只认**可见**的窗口：收进托盘的窗口可能还留着上次的聚焦标记，按它判断会永远不发通知。
///
/// - `app`: 应用句柄
///
/// # 返回值
///
/// 前台窗口的标签；没有窗口在前台时为 `None`
fn foreground_window(app: &AppHandle) -> Option<String> {
    app.webview_windows()
        .into_iter()
        .find(|(_, window)| {
            window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false)
        })
        .map(|(label, _)| label)
}

/// 启动器不在前台时发一条系统通知
///
/// 发送失败只记日志：通知是锦上添花，不该影响下载或安装本身的结果。
///
/// - `app`: 应用句柄
/// - `text`: 通知文案，标题与正文都跟界面语言走
pub(crate) fn notify_if_unfocused(app: &AppHandle, text: NotifyText) {
    if let Some(window) = foreground_window(app) {
        mml_log::info(LogText::NotificationSkipped { window }.text());
        return;
    }

    let (title, body) = text.parts();
    match app
        .notification()
        .builder()
        .title(title.clone())
        .body(body)
        .show()
    {
        Ok(()) => mml_log::info(LogText::NotificationSent { title }.text()),
        Err(error) => mml_log::error(
            LogText::NotificationFailed {
                error: error.to_string(),
            }
            .text(),
        ),
    }
}

/// 请求一次系统通知权限，启动时调用
///
/// Windows 与 Linux 上直接就是已授权；macOS 首次会弹系统授权框，
/// 之后由系统记住，所以只在启动时问一次。
///
/// - `app`: 应用句柄
pub(crate) fn request_permission(app: &AppHandle) {
    if let Err(error) = app.notification().request_permission() {
        mml_log::error(
            LogText::NotificationPermissionFailed {
                error: error.to_string(),
            }
            .text(),
        );
    }
}

/// 补一个带 AUMID 的开始菜单快捷方式（仅 Windows）
///
/// 只在便携版上真会写：装过的用户开始菜单里已经有同名的快捷方式，见 [windows_link::ensure]。
/// 失败只记日志，不影响启动。
pub(crate) fn register_start_menu_shortcut() {
    #[cfg(windows)]
    if let Err(error) = windows_link::ensure() {
        mml_log::error(LogText::NotificationShortcutFailed { error }.text());
    }
}

#[cfg(windows)]
mod windows_link {
    use std::path::{Path, PathBuf};

    use windows::Win32::System::Com::StructuredStorage::{
        PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemAlloc, CoTaskMemFree, IPersistFile,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
    };
    use windows::core::{GUID, HSTRING, Interface, PWSTR};

    use crate::log_text::LogText;

    /// 任务栏身份，与 `tauri.conf.json` 的 identifier 一致，见 lib.rs 的 set_app_user_model_id
    const APP_USER_MODEL_ID: &str = "com.coloryr.mml";

    /// `System.AppUserModel.ID` 的属性键（shell 的 `PKEY_AppUserModel_ID`）
    const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };

    /// 开始菜单里的快捷方式名，与安装包（NSIS）建的那份同名：
    /// 装过的用户这里已经有文件，直接跳过，免得开始菜单里出现两条
    const SHORTCUT_NAME: &str = "mml-gui.lnk";

    /// 确保开始菜单里有带 AUMID 的快捷方式
    ///
    /// # 返回值
    ///
    /// 已存在或写成功返回 `Ok(())`；写失败返回原因
    pub(super) fn ensure() -> Result<(), String> {
        let path = programs_dir()?.join(SHORTCUT_NAME);
        if path.is_file() {
            return Ok(());
        }
        write_link(&path)?;
        mml_log::info(
            LogText::NotificationShortcut {
                path: path.to_string_lossy().to_string(),
            }
            .text(),
        );
        Ok(())
    }

    /// 开始菜单的「程序」目录：走 Known Folder，重定向过的也能找对
    fn programs_dir() -> Result<PathBuf, String> {
        let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None) }
            .map_err(|err| err.message().to_string())?;
        let text = unsafe { raw.to_string() }.map_err(|err| err.to_string());
        unsafe { CoTaskMemFree(Some(raw.as_ptr() as *const _)) };
        Ok(PathBuf::from(text?))
    }

    /// 写一个指向本程序、带 AUMID 属性的快捷方式
    ///
    /// - `path`: 快捷方式落盘路径
    ///
    /// # 返回值
    ///
    /// 写成功返回 `Ok(())`；COM 调用失败返回原因
    fn write_link(path: &Path) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let exe_text = exe.to_string_lossy().to_string();
        let dir_text = exe
            .parent()
            .map(|dir| dir.to_string_lossy().to_string())
            .unwrap_or_default();
        // AUMID 按 VT_LPWSTR 传。字符串照 COM 的规矩用 CoTaskMemAlloc 分配，
        // 交给 PROPVARIANT 的析构（PropVariantClear）释放，不能拿 Rust 的 Vec 指针糊弄
        let wide: Vec<u16> = APP_USER_MODEL_ID
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let aumid = unsafe { CoTaskMemAlloc(wide.len() * 2) } as *mut u16;
        if aumid.is_null() {
            return Err(String::from("CoTaskMemAlloc failed"));
        }
        unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), aumid, wide.len()) };

        unsafe {
            // 跑在 worker 线程上，这里自己初始化一次 COM；已初始化时返回 S_FALSE，忽略
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| err.message().to_string())?;
            link.SetPath(&HSTRING::from(exe_text.as_str()))
                .map_err(|err| err.message().to_string())?;
            link.SetWorkingDirectory(&HSTRING::from(dir_text.as_str()))
                .map_err(|err| err.message().to_string())?;

            // AUMID 是这条快捷方式唯一真正要紧的属性：没有它，Windows 不认这个应用能发通知
            let store: IPropertyStore = link.cast().map_err(|err| err.message().to_string())?;
            // 联合体字段包着 ManuallyDrop，直接赋值会被拒绝，整块按类型构造
            let value = PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_LPWSTR,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: PROPVARIANT_0_0_0 { pwszVal: PWSTR(aumid) },
                    }),
                },
            };
            store
                .SetValue(&PKEY_APP_USER_MODEL_ID, &value)
                .map_err(|err| err.message().to_string())?;
            store.Commit().map_err(|err| err.message().to_string())?;
            // 析构会调 PropVariantClear，把上面那块 CoTaskMem 释放掉
            drop(value);

            let file: IPersistFile = link.cast().map_err(|err| err.message().to_string())?;
            file.Save(&HSTRING::from(path.to_string_lossy().as_ref()), true)
                .map_err(|err| err.message().to_string())?;
        }

        Ok(())
    }
}
