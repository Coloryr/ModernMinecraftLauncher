//! 游戏进程句柄
//!
//! 起进程、读输出、判定退出与强制结束；读到的每一行都进运行日志。

use std::{
    io::{BufRead, BufReader},
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    thread,
};

use encoding_rs_io::DecodeReaderBytesBuilder;
use mml_names::i18_items::error_type::CoreResult;
use mml_sys::{path_helper, process_helper};
use uuid::Uuid;

use crate::{game_log::GameLog, launcher::LogEncoding};

use super::types::{GameRunObj, LaunchDebugger};

/// 游戏句柄
pub(crate) struct InstanceHandle {
    /// 游戏进程，共享所有权，用于等待退出和强制结束；提权启动时存的是
    /// 启动器进程，它在目标进程运行期间保持存活，可照样用 try_wait 检测退出
    process: Arc<Mutex<Option<Child>>>,
    /// 游戏实例 uuid
    pub uuid: Uuid,
    /// 进程是否已经退出
    is_exit: Arc<AtomicBool>,
    /// 退出码；进程还没退出时是 0
    exit_code: AtomicI32,
    /// 通过已提权的辅助进程停止游戏。
    elevation_control: Option<process_helper::ElevationControl>,
    /// 进程外面是不是包着调试器：停止时要连它的子进程一起结束
    wrapped: bool,
}

impl InstanceHandle {
    /// 创建游戏句柄并启动游戏进程
    ///
    /// - `run`: 游戏运行参数
    ///
    /// # 返回值
    ///
    /// 返回游戏句柄；启动进程失败返回对应错误
    pub fn new(run: GameRunObj) -> CoreResult<Self> {
        // 创建工作目录
        path_helper::create_dir_all(&run.path)?;

        // 启动进程。调试器是控制台程序，藏掉它的窗口；停止时也要结束整棵进程树
        let wrapped = run.debugger != LaunchDebugger::None;
        let result = if wrapped {
            process_helper::launch_hidden_in_group(
                run.java, run.args, run.env, run.path, run.admin,
            )?
        } else {
            process_helper::launch(run.java, run.args, run.env, run.path, run.admin)?
        };

        if result.is_admin {
            crate::add_game_log_item(&run.uuid, GameLog::JavaRedirect);
        }

        let process = Arc::new(Mutex::new(Some(result.child)));
        let is_exit = Arc::new(AtomicBool::new(false));
        // 调试器会把完整 Java 命令行再打印一遍，里面有 access token；这层输出要盖掉
        let mask_launch_args = wrapped;

        // 读取 stdout 线程
        if let Some(reader) = result.stdout {
            let exit = is_exit.clone();
            let uuid = run.uuid;
            let encoding = run.encoding;
            let _ = thread::Builder::new()
                .name(format!("Mml Game {} StandardOutput", uuid))
                .spawn(move || {
                    read_process_stream(reader, uuid, encoding, exit, mask_launch_args);
                });
        }

        // 读取 stderr 线程
        if let Some(reader) = result.stderr {
            let exit = is_exit.clone();
            let uuid = run.uuid;
            let encoding = run.encoding;
            let _ = thread::Builder::new()
                .name(format!("Mml Game {} StandardError", uuid))
                .spawn(move || {
                    read_process_stream(reader, uuid, encoding, exit, mask_launch_args);
                });
        }

        Ok(InstanceHandle {
            process,
            uuid: run.uuid,
            is_exit,
            exit_code: AtomicI32::new(0),
            elevation_control: result.control,
            wrapped,
        })
    }

    /// 强制结束游戏进程
    ///
    /// 结束失败时静默忽略。
    pub fn kill(&self) {
        if let Some(control) = &self.elevation_control {
            let _ = control.request_stop();
            // 保留等待句柄，由 tick 确认游戏及辅助进程真正退出。
            return;
        }
        if let Ok(mut guard) = self.process.lock()
            && let Some(ref mut child) = *guard
        {
            let result = if self.wrapped {
                // 调试器下面还挂着 Java，只杀它自己会留下一个还在跑的游戏
                process_helper::kill_tree_group(child)
            } else {
                child.kill()
            };
            if result.is_ok() {
                self.is_exit.store(true, Ordering::SeqCst);
            }
        }
    }

    /// 轮询进程状态，退出时标记退出标志
    ///
    /// 已退出或已标记时忽略。
    pub fn tick(&self) {
        // 等待进程退出线程
        let process_clone = self.process.clone();
        let exit_clone = self.is_exit.clone();
        if exit_clone.load(Ordering::Relaxed) {
            return;
        }
        let status = {
            let mut guard = process_clone.lock().unwrap();
            if let Some(ref mut child) = *guard {
                child.try_wait().ok().flatten()
            } else {
                None
            }
        };
        if let Some(exit_status) = status {
            exit_clone.store(true, Ordering::SeqCst);
            self.exit_code
                .store(exit_status.code().unwrap_or(-1), Ordering::SeqCst);
            process_clone.lock().unwrap().take();
        }
    }

    /// 进程是否已经退出
    ///
    /// # 返回值
    ///
    /// 返回进程是否已退出
    pub fn is_exit(&self) -> bool {
        self.is_exit.load(Ordering::Relaxed)
    }

    /// 进程退出码
    ///
    /// # 返回值
    ///
    /// 返回进程退出码
    pub fn code(&self) -> i32 {
        self.exit_code.load(Ordering::SeqCst)
    }
}

/// 读取进程输出流并转发到游戏日志
///
/// - `reader`: 缓冲读取器
/// - `uuid`: 游戏实例 uuid
/// - `encoding`: 日志编码，UTF-8 或 GBK
/// - `exit_flag`: 退出标志，用于停止读取
/// - `mask_launch_args`: 是否过滤 RenderDoc 的命令行回显
///
/// 流关闭或收到退出标志后结束。
pub(super) fn read_process_stream<R: std::io::Read>(
    reader: R,
    uuid: Uuid,
    encoding: LogEncoding,
    exit_flag: Arc<AtomicBool>,
    mask_launch_args: bool,
) {
    match encoding {
        LogEncoding::UTF8 => {
            let reader = BufReader::new(reader);
            for line in reader.lines() {
                if exit_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(line) = line {
                    forward_game_line(&uuid, &line, mask_launch_args);
                }
            }
        }
        LogEncoding::GBK => {
            let transcoder = DecodeReaderBytesBuilder::new()
                .encoding(Some(encoding_rs::GBK))
                .build(reader);
            let reader = BufReader::new(transcoder);
            for line in reader.lines() {
                if exit_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(line) = line {
                    forward_game_line(&uuid, &line, mask_launch_args);
                }
            }
        }
    }
}

/// 把一行进程输出转进游戏日志
///
/// renderdoccmd 启动时会把完整 Java 命令行打印一遍，里面带 access token；
/// 这一行换成固定文案，其余输出原样转发。
///
/// - `uuid`: 游戏实例 uuid
/// - `line`: 进程输出的一行
/// - `mask_launch_args`: 是否过滤 renderdoccmd 的命令行回显
pub(super) fn forward_game_line(uuid: &Uuid, line: &str, mask_launch_args: bool) {
    if mask_launch_args
        && (line.starts_with("Launching '")
            || line.contains("--accessToken")
            || line.contains("--accesstoken"))
    {
        crate::add_game_log_item(uuid, GameLog::RenderDocLaunch);
    } else {
        crate::add_game_log(uuid, line);
    }
}
