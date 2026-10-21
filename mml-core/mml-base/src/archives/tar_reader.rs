//! Tar/TarGz/TarXz 压缩/解压实现，基于 `tar` 与 `flate2` / `xz2` crate

use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use flate2::{Compression, write::GzEncoder};
use mml_names::i18_items::error_type::CoreResult;
use mml_sys::path_helper;
use tar::{Archive, Builder};
use xz2::{read::XzDecoder, write::XzEncoder};

use crate::archives::{
    self, ArchiveAddFile, ArchiveEntryInfo, ArchiveHandle, ArchiveProcess, ArchiveType,
    BaseArchiveGui, IBaseArchiveGui, TarMode, archive_err, fs_err, read_error, root_path_or_pack,
    write_error,
};

/// 保持打开文件句柄的 Tar/TarGz/TarXz 读取句柄
///
/// tar 读取器是流式的、不可复用，每次操作从持有的文件句柄克隆出独立文件描述符再重建读取器
/// 同时提供静态的压缩/解压批处理入口，`mode=None` 表示纯 tar
pub(crate) struct TarReader {
    /// 读写的文件句柄，追加与读取时克隆使用
    file: fs::File,
    /// 压缩包类型，决定读取器是否套一层解压流
    archive_type: ArchiveType,
    /// 压缩包在磁盘上的路径
    path: PathBuf,
}

impl TarReader {
    /// 打开 tar/tar.gz/tar.xz 文件
    ///
    /// - `file`: 已打开的压缩包文件句柄
    /// - `archive_type`: 压缩包类型，取 Tar / TarGz / TarXz
    /// - `path`: 压缩包磁盘路径
    ///
    /// # 返回值
    ///
    /// 返回持有该句柄的读取器，此时尚未解析条目
    pub(crate) fn new(
        file: fs::File,
        archive_type: ArchiveType,
        path: PathBuf,
    ) -> CoreResult<Self> {
        Ok(Self {
            file,
            archive_type,
            path,
        })
    }

    /// 从持有的文件句柄克隆出独立文件描述符，构建对应的 tar 读取器
    ///
    /// 克隆句柄与原句柄共享文件偏移，读取前先定位到文件开头
    ///
    /// # 返回值
    ///
    /// 返回已定位到文件开头的读取器；克隆或定位失败时返回相应错误
    fn reader(&self) -> CoreResult<Archive<Box<dyn Read>>> {
        let mut file = self
            .file
            .try_clone()
            .map_err(|err| fs_err(self.path.clone(), err))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|err| fs_err(self.path.clone(), err))?;
        match self.archive_type {
            ArchiveType::Tar => Ok(Archive::new(Box::new(file))),
            ArchiveType::TarGz => {
                let gz = GzDecoder::new(file);
                Ok(Archive::new(Box::new(gz)))
            }
            ArchiveType::TarXz => {
                let xz = XzDecoder::new(file);
                Ok(Archive::new(Box::new(xz)))
            }
            _ => unreachable!(),
        }
    }

    /// 压缩目录为 tar/tar.gz/tar.xz 文件
    ///
    /// - `mode`: 压缩模式，`None` 表示纯 tar
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
        mode: Option<TarMode>,
        archive_file: &Path,
        pack_dir: &Path,
        root_path: Option<&Path>,
        filter: &Option<Vec<String>>,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let process = ArchiveProcess::new(gui);
        Self::tar(
            &process,
            archive_file,
            pack_dir,
            root_path_or_pack(root_path, pack_dir),
            filter,
            mode,
        )
    }

    /// 解压 tar/tar.gz/tar.xz 文件到指定目录
    ///
    /// - `mode`: 压缩模式，`None` 表示纯 tar
    /// - `archive_file`: 压缩包文件路径
    /// - `output_dir`: 解压输出目录
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 解压失败时返回相应错误
    pub(crate) fn decompress(
        mode: Option<TarMode>,
        archive_file: &Path,
        output_dir: &Path,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let process = ArchiveProcess::new(gui);
        Self::un_tar(&process, archive_file, output_dir, mode)
    }

    /// 压缩实现，逐条目写入并推进进度
    ///
    /// - `process`: 进度追踪器
    /// - `archive_file`: 压缩包输出路径
    /// - `pack_dir`: 需要打包的源目录
    /// - `root_path`: 相对根路径，设置后文件在包内的路径相对于此
    /// - `filter`: 可选的排除子串列表，匹配的文件将被跳过
    /// - `mode`: 压缩模式，`None` 表示纯 tar
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误
    fn tar(
        process: &ArchiveProcess,
        archive_file: &Path,
        pack_dir: &Path,
        root_path: &Path,
        filter: &Option<Vec<String>>,
        mode: Option<TarMode>,
    ) -> CoreResult<()> {
        let file = path_helper::open_write(archive_file)?;

        let files = path_helper::get_all_files(pack_dir);

        let mut tar_builder: Builder<Box<dyn std::io::Write>> = match mode {
            Some(TarMode::Gz) => {
                let gz_encoder = GzEncoder::new(file, Compression::default());
                Builder::new(Box::new(gz_encoder))
            }
            Some(TarMode::Xz) => {
                let xz_encoder = XzEncoder::new(file, 6);
                Builder::new(Box::new(xz_encoder))
            }
            None => Builder::new(Box::new(file) as Box<dyn std::io::Write>),
        };

        for path in files {
            process.add_now(&path);

            if let Some(patterns) = filter {
                if archives::should_exclude(&path, patterns) {
                    continue;
                }
            }

            let relative_path = path.strip_prefix(root_path).unwrap();
            let archive_path = archives::normalize_path(relative_path);

            if path.is_file() {
                let mut file_reader = path_helper::open_read(&path)?;

                tar_builder
                    .append_file(&archive_path, &mut file_reader)
                    .map_err(|err| {
                        archive_err(
                            path.to_string_lossy().to_string(),
                            archive_path.clone(),
                            err,
                        )
                    })?;
            } else if path.is_dir() {
                tar_builder
                    .append_dir(&archive_path, &path)
                    .map_err(|err| {
                        archive_err(
                            path.to_string_lossy().to_string(),
                            archive_path.clone(),
                            err,
                        )
                    })?;
            }
        }

        tar_builder.finish().map_err(write_error)?;

        Ok(())
    }

    /// 解压实现，先数一遍条目数再逐条落盘
    ///
    /// - `process`: 进度追踪器
    /// - `archive_file`: 压缩包文件路径
    /// - `output_dir`: 解压输出目录
    /// - `mode`: 压缩模式，`None` 表示纯 tar
    ///
    /// # 返回值
    ///
    /// 解压失败时返回相应错误
    fn un_tar(
        process: &ArchiveProcess,
        archive_file: &Path,
        output_dir: &Path,
        mode: Option<TarMode>,
    ) -> CoreResult<()> {
        path_helper::create_dir_all(output_dir)?;

        {
            let file = path_helper::open_read(archive_file)?;
            let mut archive: Archive<Box<dyn Read>> = match mode {
                Some(TarMode::Gz) => {
                    let gz = GzDecoder::new(file);
                    Archive::new(Box::new(gz))
                }
                Some(TarMode::Xz) => {
                    let xz = XzDecoder::new(file);
                    Archive::new(Box::new(xz))
                }
                None => Archive::new(Box::new(file) as Box<dyn Read>),
            };
            let count = archive.entries().map_err(read_error)?.count();
            process.set_count(count);
        }

        let file = path_helper::open_read(archive_file)?;
        let mut archive: Archive<Box<dyn Read>> = match mode {
            Some(TarMode::Gz) => {
                let gz = GzDecoder::new(file);
                Archive::new(Box::new(gz))
            }
            Some(TarMode::Xz) => {
                let xz = XzDecoder::new(file);
                Archive::new(Box::new(xz))
            }
            None => Archive::new(Box::new(file) as Box<dyn Read>),
        };

        let items = archive.entries().map_err(read_error)?;

        for entry in items {
            let mut entry = entry.map_err(read_error)?;
            let path = entry.path().map_err(read_error)?.to_path_buf();
            // 文件名非法时询问 GUI 是否替换
            let name = process.check_name(&path.to_string_lossy())?;
            let name_path = Path::new(&name).to_path_buf();
            process.add_now(&name_path);
            if name_path != path {
                let dest = output_dir.join(name_path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| fs_err(parent.to_path_buf(), err))?;
                }
                entry.unpack(dest).map_err(|err| {
                    archive_err(
                        path.display().to_string(),
                        output_dir.display().to_string(),
                        err,
                    )
                })?;
            } else {
                entry.unpack_in(output_dir).map_err(|err| {
                    archive_err(
                        path.display().to_string(),
                        output_dir.display().to_string(),
                        err,
                    )
                })?;
            }
        }

        process.done();

        Ok(())
    }
}

impl ArchiveHandle for TarReader {
    /// 重建读取器并顺序扫一遍头部，收集条目名、是否目录与大小
    fn read_entries(&mut self) -> CoreResult<Vec<ArchiveEntryInfo>> {
        let mut archive = self.reader()?;

        let mut entries = Vec::new();
        for entry in archive.entries().map_err(read_error)? {
            let entry = entry.map_err(read_error)?;
            let header = entry.header();
            entries.push(ArchiveEntryInfo {
                name: entry
                    .path()
                    .map_err(read_error)?
                    .to_string_lossy()
                    .to_string(),
                is_dir: header.entry_type() == tar::EntryType::Directory,
                size: header.size().map_err(read_error)?,
            });
        }
        Ok(entries)
    }

    /// 线性扫描找到同名条目并读出内容，目录条目视为错误
    fn read(&mut self, name: &str) -> CoreResult<Vec<u8>> {
        let mut archive = self.reader()?;

        for entry in archive.entries().map_err(read_error)? {
            let mut entry = entry.map_err(read_error)?;
            let entry_path = entry.path().map_err(read_error)?;
            if entry_path.to_string_lossy() == name {
                let header = entry.header();
                if header.entry_type() == tar::EntryType::Directory {
                    return Err(read_error(name));
                }
                let mut buf = Vec::with_capacity(header.size().unwrap_or(0) as usize);
                entry.read_to_end(&mut buf).map_err(read_error)?;
                return Ok(buf);
            }
        }

        Err(read_error(name))
    }

    /// tar 无法真正流式读取，先整条读进内存再包成游标
    fn read_stream(&mut self, name: &str) -> CoreResult<Box<dyn Read>> {
        let data = self.read(name)?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }

    /// 扫描找到同名条目后解包到目标路径，进度按单文件上报
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
        for entry in archive.entries().map_err(read_error)? {
            let mut entry = entry.map_err(read_error)?;
            let entry_path = entry.path().map_err(read_error)?;
            if entry_path.to_string_lossy() == name {
                found = true;
                let mut outfile = path_helper::open_write(output_path)?;
                std::io::copy(&mut entry, &mut outfile).map_err(|err| {
                    archive_err(name.to_string(), output_path.display().to_string(), err)
                })?;
                break;
            }
        }

        if !found {
            return Err(read_error(name));
        }

        if let Some(gui) = gui {
            gui.update(Some(name.to_string()), 1);
        }

        Ok(())
    }

    /// 就地追加磁盘文件，靠回退 EOF 标记复用已有数据
    fn add_files(&mut self, files: &[ArchiveAddFile]) -> CoreResult<()> {
        use tar::Builder;

        let mut file = self
            .file
            .try_clone()
            .map_err(|err| fs_err(self.path.clone(), err))?;

        // 跳过 tar 的 EOF 标记，两个 512 字节零块由 Builder 用新条目与新 EOF 覆盖
        let len = file
            .metadata()
            .map_err(|err| fs_err(self.path.clone(), err))?
            .len();
        if len >= 1024 {
            file.seek(SeekFrom::Start(len - 1024))
                .map_err(|err| fs_err(self.path.clone(), err))?;
        }

        let mut builder = Builder::new(file);
        for file in files {
            let internal_path = file.dest.to_string_lossy();
            let mut reader = path_helper::open_read(&file.src)?;

            builder
                .append_file(internal_path.as_ref(), &mut reader)
                .map_err(write_error)?;
        }

        builder.finish().map_err(write_error)?;
        Ok(())
    }

    /// 就地追加内存数据，靠回退 EOF 标记复用已有数据
    fn add_data(&mut self, name: &str, data: &[u8]) -> CoreResult<()> {
        use tar::{Builder, Header};

        let mut file = self
            .file
            .try_clone()
            .map_err(|err| fs_err(self.path.clone(), err))?;

        // 跳过 tar 的 EOF 标记，两个 512 字节零块由 Builder 用新条目与新 EOF 覆盖
        let len = file
            .metadata()
            .map_err(|err| fs_err(self.path.clone(), err))?
            .len();
        if len >= 1024 {
            file.seek(SeekFrom::Start(len - 1024))
                .map_err(|err| fs_err(self.path.clone(), err))?;
        }

        let mut builder = Builder::new(file);
        let mut header = Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder
            .append_data(&mut header, name, data)
            .map_err(write_error)?;

        builder.finish().map_err(write_error)?;
        Ok(())
    }
}
