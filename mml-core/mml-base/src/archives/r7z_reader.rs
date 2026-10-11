//! 7z 压缩/解压实现，基于 `sevenz-rust` crate

use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use mml_names::i18_items::error_type::{
    ArchiveErrorData, CoreResult, ErrorData, ErrorType, FileSystemErrorData,
};
use mml_sys::path_helper;
use sevenz_rust2::{ArchiveEntry, ArchiveReader, ArchiveWriter, Password};

use crate::archives::{
    self, ArchiveEntryInfo, ArchiveHandle, ArchiveProcess, BaseArchiveGui, IBaseArchiveGui,
};

/// 保持打开文件句柄的 7z 读取句柄
///
/// `ArchiveReader` 不可复用，每次操作从持有的文件句柄克隆出独立文件描述符再重建读取器
/// 同时提供静态的压缩/解压批处理入口
pub(crate) struct R7zReader {
    /// 读写的文件句柄，读取时克隆使用
    file: fs::File,
    /// 压缩包在磁盘上的路径
    path: PathBuf,
}

impl R7zReader {
    /// 打开 7z 文件
    ///
    /// - `file`: 已打开的压缩包文件句柄
    /// - `path`: 压缩包磁盘路径
    ///
    /// # 返回值
    ///
    /// 返回持有该句柄的读取器，此时尚未解析条目
    pub(crate) fn new(file: fs::File, path: PathBuf) -> CoreResult<Self> {
        Ok(Self { file, path })
    }

    /// 从持有的文件句柄克隆出独立文件描述符，构建 `ArchiveReader`
    ///
    /// 克隆句柄与原句柄共享文件偏移，读取前先定位到文件开头
    ///
    /// # 返回值
    ///
    /// 返回已定位到文件开头的读取器；克隆、定位或解析头部失败时返回相应错误
    fn reader(&self) -> CoreResult<ArchiveReader<fs::File>> {
        let mut file = self.file.try_clone().map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: self.path.clone(),
                error: err.to_string(),
            })
        })?;
        file.seek(SeekFrom::Start(0)).map_err(|err| {
            ErrorType::FileSystemError(FileSystemErrorData {
                path: self.path.clone(),
                error: err.to_string(),
            })
        })?;
        ArchiveReader::new(file, Password::empty()).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: self.path.clone(),
                error: err.to_string(),
            })
        })
    }

    /// 压缩目录为 7z 文件
    ///
    /// - `archive_file`: 压缩包输出路径
    /// - `pack_dir`: 需要打包的源目录
    /// - `root_path`: 相对根路径，设置后文件在包内的路径相对于此
    /// - `filter`: 可选的排除子串列表，匹配的文件将被跳过
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误
    pub(crate) fn compress(
        archive_file: &Path,
        pack_dir: &Path,
        root_path: Option<&Path>,
        filter: &Option<Vec<String>>,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let process = ArchiveProcess::new(gui);
        let root_path = match root_path {
            Some(path) => path,
            None => pack_dir,
        };
        Self::r7z_compress(&process, archive_file, pack_dir, root_path, filter)
    }

    /// 解压 7z 文件到指定目录
    ///
    /// - `archive_file`: 压缩包文件路径
    /// - `output_dir`: 解压输出目录
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 解压失败时返回相应错误
    pub(crate) fn decompress(
        archive_file: &Path,
        output_dir: &Path,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let process = ArchiveProcess::new(gui);
        Self::r7z_decompress(&process, archive_file, output_dir)
    }

    /// 压缩实现，逐条目写入并推进进度
    ///
    /// - `process`: 进度追踪器
    /// - `archive_file`: 压缩包输出路径
    /// - `pack_dir`: 需要打包的源目录
    /// - `root_path`: 相对根路径，设置后文件在包内的路径相对于此
    /// - `filter`: 可选的排除子串列表，匹配的文件将被跳过
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误
    fn r7z_compress(
        process: &ArchiveProcess,
        archive_file: &Path,
        pack_dir: &Path,
        root_path: &Path,
        filter: &Option<Vec<String>>,
    ) -> CoreResult<()> {
        let file = path_helper::open_write(archive_file)?;
        let mut archive = ArchiveWriter::new(file).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: archive_file.to_path_buf(),
                error: err.to_string(),
            })
        })?;

        let entries = path_helper::get_all_files(pack_dir);
        process.set_count(entries.len());

        for path in entries {
            process.add_now(&path);

            if let Some(patterns) = filter {
                if archives::should_exclude(&path, patterns) {
                    continue;
                }
            }

            let buffer = path_helper::open_read(&path)?;

            let relative_path = path.strip_prefix(root_path).unwrap();
            let tempfile = archives::normalize_path(relative_path);

            archive
                .push_archive_entry(
                    ArchiveEntry::from_path(&path, tempfile.clone()),
                    Some(buffer),
                )
                .map_err(|err| {
                    ErrorType::ArchiveError(ArchiveErrorData {
                        source: path.display().to_string(),
                        target: tempfile,
                        error: err.to_string(),
                    })
                })?;
        }

        match archive.finish() {
            Ok(_) => Ok(()),
            Err(err) => Err(ErrorType::ArchiveWriteError(ErrorData {
                error: err.to_string(),
            })),
        }
    }

    /// 解压实现，先预计算目标路径再逐条目落盘
    ///
    /// - `process`: 进度追踪器
    /// - `archive_file`: 压缩包文件路径
    /// - `output_dir`: 解压输出目录
    ///
    /// # 返回值
    ///
    /// 解压失败时返回相应错误
    fn r7z_decompress(
        process: &ArchiveProcess,
        archive_file: &Path,
        output_dir: &Path,
    ) -> CoreResult<()> {
        let file = path_helper::open_read(archive_file)?;
        path_helper::create_dir_all(output_dir)?;

        let mut seven = ArchiveReader::new(file, Password::empty()).map_err(|err| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: archive_file.to_path_buf(),
                error: err.to_string(),
            })
        })?;

        process.set_count(seven.archive().files.len());

        // 预计算所有条目的目标路径，文件名非法时替换为 `_`，GUI 不同意则取消
        let dest_map: HashMap<String, PathBuf> = seven
            .archive()
            .files
            .iter()
            .map(|f| {
                let name = process.check_name(f.name())?;
                Ok((f.name().to_string(), output_dir.join(name)))
            })
            .collect::<CoreResult<_>>()?;

        match seven.for_each_entries(|entry, reader| {
            let dest_path = dest_map
                .get(entry.name())
                .cloned()
                .unwrap_or_else(|| output_dir.join(entry.name()));
            process.add_now(&dest_path);
            sevenz_rust2::default_entry_extract_fn(entry, reader, &dest_path)
        }) {
            Ok(_) => {
                process.done();
                Ok(())
            }
            Err(err) => Err(ErrorType::ArchiveReadError(ErrorData {
                error: err.to_string(),
            })),
        }
    }
}

impl ArchiveHandle for R7zReader {
    /// 直接取头部文件表，不逐个解压
    fn read_entries(&mut self) -> CoreResult<Vec<ArchiveEntryInfo>> {
        let archive = self.reader()?;
        let entries = archive
            .archive()
            .files
            .iter()
            .map(|f| ArchiveEntryInfo {
                name: f.name().to_string(),
                is_dir: f.is_directory(),
                size: f.size(),
            })
            .collect();
        Ok(entries)
    }

    /// 遍历条目找到同名文件并读出内容，目录条目视为错误
    fn read(&mut self, name: &str) -> CoreResult<Vec<u8>> {
        let mut archive = self.reader()?;

        let mut result: Option<Vec<u8>> = None;
        let mut found = false;
        archive
            .for_each_entries(|entry, reader| {
                if entry.name() == name {
                    found = true;
                    if entry.is_directory() {
                        result = None;
                        return Err(sevenz_rust2::Error::Other("is directory".into()));
                    }
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(reader, &mut buf)?;
                    result = Some(buf);
                    Ok(false) // 停止遍历
                } else {
                    Ok(true)
                }
            })
            .map_err(|err| {
                ErrorType::ArchiveReadError(ErrorData {
                    error: err.to_string(),
                })
            })?;

        if !found {
            return Err(ErrorType::ArchiveReadError(ErrorData {
                error: name.to_string(),
            }));
        }

        result.ok_or_else(|| {
            ErrorType::ArchiveReadError(ErrorData {
                error: name.to_string(),
            })
        })
    }

    /// 7z 无法真正流式读取，先整条读进内存再包成游标
    fn read_stream(&mut self, name: &str) -> CoreResult<Box<dyn Read>> {
        let data = self.read(name)?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }

    /// 遍历条目找到同名文件后解包到目标路径，进度按单文件上报
    fn extract_file(
        &mut self,
        name: &str,
        output_path: &Path,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        if let Some(gui) = gui {
            gui.start(1);
        }

        let mut archive = self.reader()?;

        let mut found = false;
        let output_pb = output_path.to_path_buf();
        archive
            .for_each_entries(|entry, reader| {
                if entry.name() == name {
                    found = true;
                    if let Some(parent) = output_pb.parent() {
                        fs::create_dir_all(parent).ok();
                    }
                    sevenz_rust2::default_entry_extract_fn(entry, reader, &output_pb)
                } else {
                    Ok(true)
                }
            })
            .map_err(|err| {
                ErrorType::ArchiveReadError(ErrorData {
                    error: err.to_string(),
                })
            })?;

        if !found {
            return Err(ErrorType::ArchiveReadError(ErrorData {
                error: name.to_string(),
            }));
        }

        if let Some(gui) = gui {
            gui.update(Some(name.to_string()), 1);
        }

        Ok(())
    }
}
