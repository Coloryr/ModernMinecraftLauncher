//! 系统通知：启动器不在前台时，把用户看不到的结果推给系统
//!
//! 三个平台都由 tauri-plugin-notification 落到各自的原生形态：Windows 是操作中心的吐司，
//! macOS 是右上角的气泡，Linux 走桌面环境的通知服务。
//!
//! 判据是**启动器有没有窗口在前台**：用户正看着界面时再弹一条系统提示是多余的，
//! 界面上本来就有进度与结果。
//!
//! Windows 上还多一道门槛：未打包的 Win32 程序想弹吐司，发通知用的 AUMID 必须是"登记过"的，
//! 否则 show() 照样返回成功、系统直接把这条通知丢掉，日志里什么都看不到。微软给的办法是
//! 开始菜单里一条带 `System.AppUserModel.ID` 的快捷方式；这里改成往注册表登记同一个身份，
//! 不用往开始菜单塞文件，多份安装也不会互相覆盖。见 [register_notification_identity]。

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
    // 可见 / 聚焦只能问窗口，而窗口查询要走事件循环、等主线程；这里不需要结果，
    // 排进主线程就行。同步等的话，下载线程会和主线程上的下载任务锁互等而卡死
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if let Some(window) = foreground_window(&handle) {
            mml_log::info(LogText::NotificationSkipped { window }.text());
            return;
        }
        notify(&handle, text);
    }) {
        mml_log::error(
            LogText::NotificationFailed {
                error: error.to_string(),
            }
            .text(),
        );
    }
}

/// 直接发一条系统通知，不看前台
///
/// 用于一次性、用户会希望事后确认的结果，如启动时的数据升级完成
///
/// - `app`: 应用句柄
/// - `text`: 通知文案
pub(crate) fn notify(app: &AppHandle, text: NotifyText) {
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

/// 登记发通知用的身份（仅 Windows）
///
/// 与 [app_user_model_id] 是两回事：那个决定任务栏怎么分组，这个是发吐司时用的身份，
/// 通知插件固定用配置里的 identifier。失败只记日志，不影响启动。
///
/// - `app`: 应用句柄，取产品名当通知设置里显示的名字
pub(crate) fn register_notification_identity(app: &AppHandle) {
    // 只登记发通知那一个身份：任务栏身份（按运行目录那份）登记了会在通知设置里多出一条
    // 同名应用，而任务栏按钮的名字与图标取自窗口本身，不需要它
    #[cfg(windows)]
    if let Err(error) = windows_toast::register(IDENTIFIER, &app.package_info().name) {
        mml_log::error(LogText::NotificationRegisterFailed { error }.text());
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// 与 `tauri.conf.json` 的 identifier 一致
///
/// 通知插件发吐司用的就是它；任务栏身份在它后面接运行目录摘要，见 [app_user_model_id]。
/// 仅 Windows 使用。
#[cfg(windows)]
const IDENTIFIER: &str = "com.coloryr.mml";

/// 任务栏身份，按运行目录区分
///
/// Windows 按 AUMID 给任务栏按钮分组，同一个身份一定并成一个按钮 —— 装了两份启动器时
/// 会挤在一个按钮里。运行目录就是"这一份安装"的身份（与客户端设置锁同一口径）：
/// 拿它的摘要做后缀，同一份每次启动都一样，两份互不相同。
#[cfg(windows)]
pub(crate) fn app_user_model_id() -> &'static str {
    use sha2::Digest;
    use std::sync::OnceLock;

    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        // 路径不分大小写：同一目录换个写法不能算成两个身份
        let text = crate::gui_config::dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let digest = sha2::Sha256::digest(text.as_bytes());
        let tag: String = digest[..4]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{IDENTIFIER}.{tag}")
    })
}

#[cfg(windows)]
mod windows_toast {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegSetValueExW,
    };
    use windows::core::{HSTRING, PCWSTR};

    /// 把一个 AUMID 登记进注册表
    ///
    /// 位置是 `HKCU\Software\Classes\AppUserModelId\<AUMID>`：系统按它认这个应用，
    /// 发通知与任务栏取名字 / 图标都看它，效果与开始菜单里那条带 AUMID 的快捷方式一致，
    /// 但不用往磁盘上写文件。
    ///
    /// - `id`: 要登记的 AUMID
    /// - `name`: 通知设置里显示的应用名
    ///
    /// # 返回值
    ///
    /// 写成功返回 `Ok(())`；注册表调用失败返回原因
    pub(super) fn register(id: &str, name: &str) -> Result<(), String> {
        let path = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{id}"));
        let mut key = HKEY::default();
        let created = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &path,
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key,
                None,
            )
        };
        if created.0 != 0 {
            return Err(format!("RegCreateKeyExW: {}", created.0));
        }

        // 图标指本程序 exe：系统自己从里面取，不必再往运行目录落一份 png
        let icon = std::env::current_exe()
            .map(|exe| exe.to_string_lossy().into_owned())
            .unwrap_or_default();
        let result = write_string(key, "DisplayName", name)
            .and_then(|()| write_string(key, "IconUri", &icon));
        unsafe {
            let _ = RegCloseKey(key);
        }
        result
    }

    /// 写一条 REG_SZ
    ///
    /// - `key`: 已打开的注册表键
    /// - `name`: 值名
    /// - `value`: 字符串内容
    ///
    /// # 返回值
    ///
    /// 写成功返回 `Ok(())`，失败返回原因
    fn write_string(key: HKEY, name: &str, value: &str) -> Result<(), String> {
        // REG_SZ 要连结尾的 0 一起写，按字节交出去
        let bytes: Vec<u8> = value
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        let name = HSTRING::from(name);
        let result = unsafe { RegSetValueExW(key, &name, None, REG_SZ, Some(&bytes)) };
        if result.0 != 0 {
            return Err(format!("RegSetValueExW: {}", result.0));
        }
        Ok(())
    }
}
