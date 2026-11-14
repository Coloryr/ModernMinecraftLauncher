//! 下载线程模块
//!
//! 实现下载工作线程和文件下载的核心逻辑。
//!
//! # 下载流程
//!
//! 1. **检查已存在文件** — 文件已存在且哈希匹配 → 直接后处理
//! 2. **发起 HTTP 请求** — 成功响应才落盘；续传需校验 206 响应的 `Content-Range`
//! 3. **流式写入临时文件** — 边下载边写入，实时更新进度
//! 4. **下载后校验** — 开启文件检查时核对大小与已知哈希值
//! 5. **移至目标路径** — 校验通过后移动临时文件到最终位置
//! 6. **后处理** — 解压 native 库 / 存档文件，成功后才记为完成
//!
//! # 重试策略
//!
//! 单文件最多尝试 3 次（首次加两次重试）；流读取失败可保留已下载数据，
//! 范围或内容校验失败则重新下载。
//!
//! # 文件职责
//!
//! | 文件 | 职责 |
//! | --- | --- |
//! | `worker` | 工作线程与队列消费主循环 |
//! | `download` | 单文件下载主流程与后处理 |
//! | `transfer` | 响应校验与流式落盘 |
//! | `verify` | 大小与哈希校验 |
//! | `retry` | 失败计数与重试判定 |
//! | `runtime` | 共享异步运行时与可取消阻塞执行 |

use std::{
    fs::File,
    future::Future,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use mml_base::{
    archives::{ArchiveType, BaseArchive},
    file_item::{FileHash, LaterRun},
    hash_helper::{self, HashType},
};
use mml_names::i18_items::error_type::{
    CoreResult, DownloadFileHashErrorData, DownloadFileOverFailData, DownloadFileSizeErrorData,
    DownloadItemFailData, ErrorData,
    ErrorType::{self, StreamError},
    HttpErrorData,
};
use mml_sys::path_helper;
use reqwest::Response;
use semka::Sem;

use crate::{
    DownloadObj, download_item::DownloadItemState, download_task::DownloadTask, later_tasks,
};

mod download;
mod retry;
mod runtime;
mod transfer;
mod verify;
mod worker;

pub(crate) use self::download::download;
pub(crate) use self::retry::is_need_err;
pub(crate) use self::runtime::{block_on, block_on_cancellable};
pub(crate) use self::transfer::{resumed_total, write_file};
pub(crate) use self::verify::{check_hash, stream_err};
pub(crate) use self::worker::DownloadThread;

#[cfg(test)]
#[path = "../../tests/unit/download_thread_transfer_tests.rs"]
mod transfer_tests;

#[cfg(test)]
#[path = "../../tests/unit/download_thread_tests.rs"]
mod tests;
