//! 系统打开操作：浏览器 / 资源管理器 / 默认程序

use std::path::Path;
use std::process::Command;

/// 在浏览器打开网址
///
/// - `url`: 目标网址
#[inline(always)]
pub fn open_url(url: &str) {
    open_url_inner(url);
}

/// Windows 实现：用 cmd start 打开网址
#[cfg(target_os = "windows")]
fn open_url_inner(url: &str) {
    Command::new("cmd")
        .args(&["/C", "start", "", url])
        .spawn()
        .unwrap();
}

/// Linux 实现：用 xdg-open 打开网址
#[cfg(target_os = "linux")]
fn open_url_inner(url: &str) {
    Command::new("xdg-open").arg(url).spawn().unwrap();
}

/// macOS 实现：用 open 打开网址
#[cfg(target_os = "macos")]
fn open_url_inner(url: &str) {
    Command::new("open").arg(url).spawn().unwrap();
}

/// 在资源管理器打开文件并选中该文件
///
/// - `path`: 目标文件路径
#[inline(always)]
pub fn open_file_with_explorer<P: AsRef<Path>>(path: P) {
    open_file_with_explorer_inner(path);
}

/// Windows 实现：用资源管理器打开父目录并选中该文件
///
/// 走 `explorer /select,<路径>`，不用 Shell 的 `SHOpenFolderAndSelectItems`：
/// 那条 COM 路要求调用线程是 STA 且带消息泵，启动器主线程不满足时会**成功返回但
/// 什么都不弹**；拉起 explorer 进程没有这些约束，也不会因为参数不合规把主线程打崩。
#[cfg(target_os = "windows")]
fn open_file_with_explorer_inner<P: AsRef<Path>>(path: P) {
    use std::os::windows::process::CommandExt;

    let path = path.as_ref();
    // 与 ColorMC 同一写法：整段 `/select,<路径>` 作为**原始参数**原样交给 explorer。
    // 两处细节都不能省：
    // - 分隔符先统一成反斜杠：配置窗口的相对路径是按 `/` 拼的，混着 `\` 和 `/` 的路径
    //   explorer 解析不出来，会退回默认目录（表现就是"打开了但位置不对"）；
    // - 用 `raw_arg` 而不是 `arg`：后者会给整段套引号（连 `/select,` 一起），选项就失效了。
    let text = path.display().to_string().replace('/', "\\");
    let arg = format!("/select,{text}");

    if Command::new("explorer").raw_arg(arg).spawn().is_err() {
        // 拉不起来就退回"只打开目录"
        open_dir(path.parent().unwrap_or(path));
    }
}

/// macOS 实现：用 open -R 在 Finder 中选中该文件
#[cfg(target_os = "macos")]
fn open_file_with_explorer_inner<P: AsRef<Path>>(path: P) {
    Command::new("open")
        .args(&["-R", path.as_ref().as_os_str()])
        .spawn()
        .unwrap();
}

/// 以系统默认方式打开文件
///
/// - `path`: 目标文件路径
#[inline(always)]
pub fn open_file<P: AsRef<Path>>(path: P) {
    open_url_inner(&path.as_ref().to_string_lossy());
}

/// 在文件管理器中打开目录本身
///
/// 与 [`open_file_with_explorer`] 的差别就在这一步：那个是"打开父目录并把这个路径选中"，
/// 用来**定位**一个文件 / 文件夹；要**进到**目录里看里面的东西用这个 ——
/// 否则用户看到的是上一层目录里选中着目标，而不是目录已经打开（实例目录踩过这个）。
///
/// - `path`: 目标目录路径
#[inline(always)]
pub fn open_dir<P: AsRef<Path>>(path: P) {
    // 三个平台上"用默认程序打开目录"都是进到目录里：Windows 的 start 拉起资源管理器，
    // Linux 走 xdg-open，macOS 走 open
    open_file(path);
}

/// 在文件管理器中打开并选中指定的文件或文件夹，仅 Linux
///
/// 模仿 opener::reveal 的行为：
/// - 优先通过 D-Bus 调用 FileManager1.ShowItems 实现精确选中。
/// - 若不可用，尝试 OpenURI 接口，通常只打开文件夹。
/// - 最后回退到 `xdg-open` 打开父目录，不会选中目标。
#[cfg(target_os = "linux")]
fn open_file_with_explorer_inner<P: AsRef<Path>>(path: P) {
    let abs_path = Path::new(path.as_ref()).canonicalize()?;
    let abs_path_str = abs_path.to_str().ok_or("路径包含非 UTF-8 字符")?;

    if let Ok(_) = try_dbus_filemanager1(abs_path_str) {
        return;
    }

    if let Ok(_) = try_dbus_openuri(abs_path_str) {
        return;
    }

    let parent = abs_path.parent().unwrap_or(Path::new("/"));
    Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|e| format!("无法打开父文件夹: {}", e))
        .unwrap();
}

/// 通过 `org.freedesktop.FileManager1` 接口显示并选中
#[cfg(target_os = "linux")]
fn try_dbus_filemanager1(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use dbus::{BusType, Connection, Message, Path as DbusPath};
    use std::path::Path;
    use std::process::Command;
    use url::Url;

    let uri = url_to_file_uri(path)?;
    let conn = Connection::get_private(BusType::Session)?;
    let proxy = conn.with_proxy(
        "org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        std::time::Duration::from_millis(5000),
    );

    // 构造参数：要选中的 URI 列表 + 启动目录，启动目录传空字符串
    let items: Vec<&str> = vec![&uri];
    let startup_dir = "";
    let (): () = proxy.method_call(
        "org.freedesktop.FileManager1",
        "ShowItems",
        (items, startup_dir),
    )?;
    Ok(())
}

/// 通过 `org.freedesktop.portal.OpenURI` 接口打开，可能不选中
#[cfg(target_os = "linux")]
fn try_dbus_openuri(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use dbus::{BusType, Connection, Message, Path as DbusPath};
    use std::path::Path;
    use std::process::Command;
    use url::Url;

    let uri = url_to_file_uri(path)?;
    let conn = Connection::get_private(BusType::Session)?;
    let proxy = conn.with_proxy(
        "org.freedesktop.portal.OpenURI",
        "/org/freedesktop/portal/desktop",
        std::time::Duration::from_millis(5000),
    );

    // 参数：父窗口句柄传 0，URI，选项传空字典
    let parent_window = "";
    let options: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let (): () = proxy.method_call(
        "org.freedesktop.portal.OpenURI",
        "OpenURI",
        (parent_window, uri, options),
    )?;
    Ok(())
}

/// 将本地路径转换为 file:// URI，用 url crate 自动处理特殊字符
#[cfg(target_os = "linux")]
fn url_to_file_uri(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    use url::Url;

    let abs = Path::new(path).canonicalize()?;
    let url = Url::from_file_path(&abs).map_err(|_| "无法将路径转换为 URL")?;
    Ok(url.to_string())
}
