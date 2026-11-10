//! 文件读写与复制
//!
//! 同步与 Tokio 异步各一份，失败统一包成带路径的文件系统错误。

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use tokio::fs as tfs;

use mml_names::i18_items::error_type::{CoreResult, ErrorData, ErrorType};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use super::fs_error;

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
