//! 单开与唤回：用运行目录里的端口文件认「已经有一个实例在跑」，并把请求交给它
//!
//! 有实例在跑时关掉主窗口后进程留在后台（计时继续，见 [`crate::tray`]），这时再启动一次
//! exe 就是靠这里把界面唤回来：读端口 → 发「启动显示」→ 本进程重建主窗口；
//! 带启动参数时发「启动参数」，本进程收下再交给前端执行。
//!
//! 端口文件就是运行目录下的 `temp`（与 ColorMC 同名），内容是**十进制文本**的端口号：
//! 出问题能直接打开看。文件在启动时就被本进程占住，占不住的那个进程走通知那条路；
//! 占住的同时**必须留出读权限**，否则第二个实例连端口都读不到。
//! 端口的读写与通知都由 `mml-ipc` 提供，这里只负责接上 Tauri 这一侧。

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

use tauri::AppHandle;

use mml_names::i18;
use mml_sys::path_helper;

use crate::log_text::LogText;

/// 端口文件句柄：本进程活着就一直持有，端口也写在这上面
///
/// 必须留着这个句柄：锁在它上面，另开一个句柄写同一个文件在 Windows 上会被锁挡掉。
static PORT_FILE: Mutex<Option<File>> = Mutex::new(None);

/// 打开并占住端口文件
///
/// 占住之后本进程活着期间，别的进程占不到 —— 这就是「已经有一个实例在跑」的判据。
/// **必须留出读权限**：第二个实例要读端口才知道该通知谁。
///
/// - Windows：用共享模式只给读，别的进程写不进来；不能用 `LockFileEx` 排它锁，
///   那会把读一起挡掉，第二个实例就读不到端口了。
/// - 其它平台：没有共享模式，用 `flock` 兜住。它是劝告锁，不挡读。
///
/// - `path`: 端口文件路径，不存在会创建
///
/// # 返回值
///
/// 占住的句柄；已被别的进程占着时返回错误
pub fn lock_port_file(path: &Path) -> std::io::Result<File> {
    let file = open_port_file(path)?;
    #[cfg(not(windows))]
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        }
        Err(std::fs::TryLockError::Error(err)) => return Err(err),
    }
    Ok(file)
}

/// 打开端口文件：读写、不存在就建、**别的进程仍可读**
///
/// - `path`: 端口文件路径
///
/// # 返回值
///
/// 打开的句柄；被别的进程占着时返回错误
#[cfg(windows)]
fn open_port_file(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(FILE_SHARE_READ.0)
        .open(path)
}

/// 打开端口文件：读写、不存在就建
///
/// - `path`: 端口文件路径
///
/// # 返回值
///
/// 打开的句柄
#[cfg(not(windows))]
fn open_port_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// 记下已锁住的端口文件句柄
///
/// - `file`: [`lock_port_file`] 返回的句柄
pub fn keep_port_file(file: File) {
    *PORT_FILE.lock().unwrap() = Some(file);
}

/// 让出端口文件：重启前调用，新进程才锁得上
pub fn release_port_file() {
    // 句柄一丢锁就释放了
    *PORT_FILE.lock().unwrap() = None;
}

/// 读端口文件里的端口号
///
/// - `path`: 端口文件路径
///
/// # 返回值
///
/// 端口号；文件不存在、读不出来或内容不是端口时返回 `None`
pub fn read_port(path: &Path) -> Option<u16> {
    let text = path_helper::read_text(path).ok()?;
    text.trim().parse().ok()
}

/// 把端口写进已锁住的文件
///
/// 先清空再写：上一个实例留下的端口必须被盖掉，否则下次启动会去连一个死端口。
///
/// - `port`: 本进程 IPC 服务的端口
fn write_port(port: u16) {
    let guard = PORT_FILE.lock().unwrap();
    let Some(file) = guard.as_ref() else {
        return;
    };
    let mut file = file;
    if file.set_len(0).is_err() || file.seek(SeekFrom::Start(0)).is_err() {
        return;
    }
    let _ = file.write_all(port.to_string().as_bytes());
    let _ = file.flush();
}

/// 起 IPC 服务、记下端口，并接上「显示主窗口 / 转发启动参数」两个事件
///
/// 必须在 `mml_core::init` 之后调用：`mml_ipc::init` 会写内核日志。
///
/// - `app`: 应用句柄，唤回主窗口与转发参数都用它
pub fn start(app: &AppHandle) {
    match mml_ipc::init(None) {
        Ok(Some(port)) => write_port(port),
        Ok(None) => {}
        Err(err) => {
            mml_log::error(
                LogText::IpcStartFailed {
                    error: i18::get_error(err),
                }
                .text(),
            );
        }
    }

    let handle = app.clone();
    mml_ipc::register_ipc_event(move || {
        crate::windows::summon_main_window(&handle);
    });

    let handle = app.clone();
    mml_ipc::register_ipc_arg_event(move |args| {
        // mml-ipc 传来的是去掉 exe 的参数表，而 start_arg 按「第 0 项是 exe」解析
        crate::start_arg::collect(std::iter::once(String::new()).chain(args.iter().cloned()));
        // 界面已经开着时前端不会再取一次，靠这条事件让它重取
        crate::windows::main::emit_start_arg(&handle);
        crate::windows::summon_main_window(&handle);
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use mml_names::names;

    /// 每个用例各用一个临时目录，见 AGENTS.md §6.1
    ///
    /// - `tag`: 用例特征名，出问题时方便在 target/temp 下认出来
    ///
    /// # 返回值
    ///
    /// 已建好的空目录
    fn port_dir(tag: &str) -> PathBuf {
        let dir = mml_testutil::temp_path(format!("mml_ipc_port_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 端口文件是十进制文本，脏内容按「没有端口」处理
    #[test]
    fn read_port_parses_decimal_text() {
        let dir = port_dir("parse");
        let file = dir.join(names::IPC_PORT_FILE);

        std::fs::write(&file, "12345").unwrap();
        assert_eq!(read_port(&file), Some(12345));

        std::fs::write(&file, "not a port").unwrap();
        assert_eq!(read_port(&file), None);

        std::fs::write(&file, " 54321 ").unwrap();
        assert_eq!(read_port(&file), Some(54321));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 占住端口文件之后：写进去的是十进制文本、清掉了旧内容，**端口仍然读得到**，
    /// 而且别的进程再占会失败 —— 第二个实例就是靠这三条把请求交给已运行实例的。
    ///
    /// 用同一个进程里的两个句柄验证：Windows 的共享冲突与 Unix 的 flock 都按句柄算，
    /// 与真开第二个进程走的是同一套判定。
    #[test]
    fn held_port_file_is_writable_by_owner_readable_by_others_and_not_lockable() {
        let dir = port_dir("lock");
        let file = dir.join(names::IPC_PORT_FILE);

        // 文件不存在时也能占住：首次启动就是这种情况
        drop(lock_port_file(&dir.join("first_run")).expect("不存在时应当能创建并占住"));

        // 先放一个旧端口，验证写入会清掉它而不是拼在后面
        std::fs::write(&file, "99999").unwrap();

        // 第一个实例：占住文件，再按启动流程写端口
        let held = lock_port_file(&file).expect("空文件应当能占住");
        keep_port_file(held);
        write_port(45678);

        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "45678",
            "端口要写成十进制文本，且盖掉旧内容"
        );
        assert_eq!(
            read_port(&file),
            Some(45678),
            "占着的时候别的进程也必须读得到端口"
        );

        // 第二个实例：占不住，只能走通知那条路
        assert!(
            lock_port_file(&file).is_err(),
            "已被占住时不该再占得上，否则会起第二个实例"
        );

        // 第一个实例退出（句柄丢掉）：文件还是那个端口，新实例能占住并覆盖它
        release_port_file();
        let again = lock_port_file(&file).expect("让出之后应当能重新占住");
        assert_eq!(read_port(&file), Some(45678), "旧端口要留到新实例写自己的");
        drop(again);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
