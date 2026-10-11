//! 输入文件抽象
//!
//! 提供统一的文件输入来源抽象，支持本地文件、网络 URL、
//! 内存数据和同步/异步流等多种输入形式，统一通过 `save_file()` 写入目标路径。

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use mml_sys::path_helper;
use tokio::io::{AsyncRead, AsyncWriteExt};

/// 输入文件来源
///
/// 封装了多种可能的文件输入形式，调用方可使用统一的 `save_file()`
/// 方法将内容写入目标路径，无需关心具体来源类型。
///
/// 两个流变体都要求 `Send`：`save_file` 是 async 函数，调用方常在
/// `#[async_trait]` 这类要求 future 为 `Send` 的地方或 `tokio::spawn` 里 await 它，
/// 而 future 是否 `Send` 取决于它跨 await 持有的值；不要求 `Send` 的话
/// 整条调用链都不是 `Send`，整合包安装器里就撞过这个。
pub enum InputFile {
    /// 本地文件系统中的文件，异步复制
    Path(PathBuf),
    /// 通过 HTTP GET 下载的网络文件
    Url(String),
    /// 内存中的字节数据
    Data(Vec<u8>),
    /// 同步读取流
    Stream(Box<dyn Read + Send>),
    /// 异步读取流
    StreamAsync(Box<dyn AsyncRead + Send + Unpin>),
}

impl InputFile {
    /// 将输入文件内容保存到指定路径
    ///
    /// - `path`: 目标文件路径
    ///
    /// # 行为
    ///
    /// - `Path` → 异步文件复制
    /// - `Url` → HTTP 流式下载
    /// - `Data` → 直接写入字节
    /// - `Stream` → 同步流写入
    /// - `StreamAsync` → 异步流写入
    pub async fn save_file<P: AsRef<Path>>(self, path: P) -> CoreResult<()> {
        match self {
            InputFile::Path(path_buf) => {
                path_helper::copy_file_async(path_buf, path.as_ref().to_path_buf()).await?;
            }
            InputFile::Url(url) => {
                let mut stream = crate::get_work_client().get(&url).await?;
                let mut file = path_helper::open_write_async(path.as_ref()).await?;

                loop {
                    match stream.chunk().await {
                        Ok(None) => break,
                        Ok(Some(data)) => {
                            // 写入文件
                            file.write_all(&data).await.map_err(|err| {
                                ErrorType::StreamError(ErrorData {
                                    error: err.to_string(),
                                })
                            })?;
                        }
                        Err(e) => {
                            return Err(ErrorType::StreamError(ErrorData {
                                error: e.to_string(),
                            }));
                        }
                    }
                }
            }
            InputFile::Data(items) => {
                path_helper::write_bytes_async(path.as_ref(), &items).await?;
            }
            InputFile::Stream(read) => {
                path_helper::write_stream(path.as_ref(), read)?;
            }
            InputFile::StreamAsync(async_read) => {
                path_helper::write_stream_async(path.as_ref(), async_read).await?;
            }
        }

        Ok(())
    }
}
