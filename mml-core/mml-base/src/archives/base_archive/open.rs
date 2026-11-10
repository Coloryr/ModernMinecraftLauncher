//! 打开、创建与整包压缩解压
//!
//! 按后缀分派到各格式读取器，并负责空包创建与整包压缩。

use std::{fs, path::Path, sync::Mutex};

use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};
use mml_sys::path_helper;
use uuid::Uuid;

use super::BaseArchive;

use crate::archives::{
    ArchiveHandle, ArchiveType, BaseArchiveGui, TarMode, fs_err, r7z_reader::R7zReader,
    tar_reader::TarReader, write_error, zip_reader::ZipReader,
};

impl BaseArchive {
    /// 打开压缩包文件，根据文件扩展名自动检测类型
    ///
    /// - `path`: 压缩包文件路径
    ///
    /// # 返回值
    ///
    /// 返回已打开并读完条目列表的处理器；文件无法打开、格式不支持或压缩包损坏时返回错误
    pub fn open<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let archive_type = ArchiveType::try_from_path(&path).ok_or_else(|| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: path.clone(),
                error: String::new(),
            })
        })?;

        // 以读写方式打开：读取句柄持有该文件，zip/tar 的就地追加复用同一句柄，无需重新打开
        let file = path_helper::open_read_write(&path)?;
        let mut handle = Self::make_handle(archive_type, &path, file)?;
        let entries = handle.read_entries()?;

        Ok(Self {
            path,
            archive_type,
            entries,
            handle: Mutex::new(handle),
        })
    }

    /// 以只读方式打开压缩包，不支持后续的就地写入
    ///
    /// 与 [`open`](Self::open) 等价但句柄不带写权限：并发打开同一文件时不受
    /// 写模式打开的杀软扫描影响，适合多线程各自持有一份句柄的场景
    ///
    /// - `path`: 压缩包文件路径
    ///
    /// # 返回值
    ///
    /// 返回已打开并读完条目列表的处理器；文件无法打开、格式不支持或压缩包损坏时返回错误
    pub fn open_readonly<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let archive_type = ArchiveType::try_from_path(&path).ok_or_else(|| {
            ErrorType::ArchiveOpenError(FileSystemErrorData {
                path: path.clone(),
                error: String::new(),
            })
        })?;
        Self::open_readonly_as(path, archive_type)
    }

    /// 以只读方式按指定格式打开压缩包，不按扩展名判断
    ///
    /// 给文件名不带压缩包扩展名的整包用，如自定义主页面存成 `home`。
    ///
    /// - `path`: 压缩包文件路径
    /// - `archive_type`: 压缩包格式，由调用方保证与文件内容一致
    ///
    /// # 返回值
    ///
    /// 同 [`open_readonly`](Self::open_readonly)
    pub fn open_readonly_as<P: AsRef<Path>>(
        path: P,
        archive_type: ArchiveType,
    ) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = path_helper::open_read(&path)?;
        let mut handle = Self::make_handle(archive_type, &path, file)?;
        let entries = handle.read_entries()?;

        Ok(Self {
            path,
            archive_type,
            entries,
            handle: Mutex::new(handle),
        })
    }

    /// 从目录创建新的压缩包并返回已打开的状态
    ///
    /// 创建成功后可通过 [`add_files`](Self::add_files) /
    /// [`add_data`](Self::add_data) 继续添加内容
    ///
    /// - `archive_type`: 压缩包格式
    /// - `output_file`: 压缩包输出路径
    /// - `source_dir`: 需要打包的源目录
    /// - `root_path`: 相对根路径，设置后文件在包内的路径相对于此
    /// - `filter`: 可选的排除子串列表，匹配的文件将被跳过
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 返回新压缩包已打开的状态；写入失败时返回相应错误
    pub fn compress<P: AsRef<Path>>(
        archive_type: ArchiveType,
        output_file: P,
        source_dir: P,
        root_path: Option<P>,
        filter: &Option<Vec<String>>,
        gui: BaseArchiveGui,
    ) -> CoreResult<Self> {
        Self::compress_inner(
            archive_type,
            output_file.as_ref(),
            source_dir.as_ref(),
            root_path.as_ref().map(|p| p.as_ref()),
            filter,
            gui,
        )?;
        Self::open_read_write_as(output_file.as_ref(), archive_type)
    }

    /// 创建一个不含条目的空压缩包并返回已打开的状态
    ///
    /// 创建后通过 [`add_files`](Self::add_files) /
    /// [`add_data`](Self::add_data) 填充内容
    ///
    /// - `archive_type`: 压缩包格式
    /// - `output_file`: 压缩包输出路径
    ///
    /// # 返回值
    ///
    /// 返回空压缩包已打开的状态；创建或打开失败时返回相应错误
    pub fn create_empty<P: AsRef<Path>>(
        archive_type: ArchiveType,
        output_file: P,
    ) -> CoreResult<Self> {
        let output_file = output_file.as_ref();
        match archive_type {
            ArchiveType::Zip => Self::create_empty_zip(output_file)?,
            ArchiveType::Tar => Self::create_empty_tar(output_file)?,
            // 7z / TarGz / TarXz：压缩空目录后再打开
            _ => {
                let temp_dir = std::env::temp_dir().join(format!("mml_empty_{}", Uuid::new_v4()));
                path_helper::create_dir_all(&temp_dir)?;
                let res =
                    Self::compress_inner(archive_type, output_file, &temp_dir, None, &None, None);
                let _ = fs::remove_dir_all(&temp_dir);
                res?;
            }
        }
        Self::open_read_write_as(output_file, archive_type)
    }

    /// 按给定格式以读写方式打开压缩包，不看扩展名
    ///
    /// 导出先把内容写进「xxx.part」这类临时文件，写的时候还没有正式名字，
    /// 按扩展名判断格式会把临时文件当成不支持的格式。
    ///
    /// - `path`: 压缩包文件路径
    /// - `archive_type`: 压缩包格式，由调用方保证与文件内容一致
    ///
    /// # 返回值
    ///
    /// 同 [`open`](Self::open)
    fn open_read_write_as(path: &Path, archive_type: ArchiveType) -> CoreResult<Self> {
        let path = path.to_path_buf();
        let file = path_helper::open_read_write(&path)?;
        let mut handle = Self::make_handle(archive_type, &path, file)?;
        let entries = handle.read_entries()?;

        Ok(Self {
            path,
            archive_type,
            entries,
            handle: Mutex::new(handle),
        })
    }

    /// 解压压缩包到指定目录，无需构造 [`BaseArchive`] 实例
    ///
    /// - `archive_type`: 压缩包格式
    /// - `archive_file`: 压缩包文件路径
    /// - `output_dir`: 解压输出目录
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 解压失败时返回相应错误，成功返回 `Ok(())`
    pub fn decompress<P: AsRef<Path>>(
        archive_type: ArchiveType,
        archive_file: P,
        output_dir: P,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let archive_file = archive_file.as_ref();
        let output_dir = output_dir.as_ref();
        match archive_type {
            ArchiveType::Zip => ZipReader::decompress(archive_file, output_dir, gui),
            ArchiveType::R7Z => R7zReader::decompress(archive_file, output_dir, gui),
            ArchiveType::Tar => TarReader::decompress(None, archive_file, output_dir, gui),
            ArchiveType::TarGz => {
                TarReader::decompress(Some(TarMode::Gz), archive_file, output_dir, gui)
            }
            ArchiveType::TarXz => {
                TarReader::decompress(Some(TarMode::Xz), archive_file, output_dir, gui)
            }
        }
    }

    /// 仅执行压缩并写入磁盘，不打开结果文件
    ///
    /// - `archive_type`: 压缩包格式
    /// - `output_file`: 压缩包输出路径
    /// - `source_dir`: 需要打包的源目录
    /// - `root_path`: 相对根路径，设置后文件在包内的路径相对于此
    /// - `filter`: 可选的排除子串列表，匹配的文件将被跳过
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误
    pub(super) fn compress_inner(
        archive_type: ArchiveType,
        output_file: &Path,
        source_dir: &Path,
        root_path: Option<&Path>,
        filter: &Option<Vec<String>>,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        match archive_type {
            ArchiveType::Zip => {
                ZipReader::compress(output_file, source_dir, root_path, filter, gui)
            }
            ArchiveType::R7Z => {
                R7zReader::compress(output_file, source_dir, root_path, filter, gui)
            }
            ArchiveType::Tar => {
                TarReader::compress(None, output_file, source_dir, root_path, filter, gui)
            }
            ArchiveType::TarGz => TarReader::compress(
                Some(TarMode::Gz),
                output_file,
                source_dir,
                root_path,
                filter,
                gui,
            ),
            ArchiveType::TarXz => TarReader::compress(
                Some(TarMode::Xz),
                output_file,
                source_dir,
                root_path,
                filter,
                gui,
            ),
        }
    }

    /// 根据压缩包类型创建对应的读取句柄
    ///
    /// - `archive_type`: 压缩包格式
    /// - `path`: 压缩包磁盘路径
    /// - `file`: 已打开的压缩包文件句柄
    ///
    /// # 返回值
    ///
    /// 返回对应格式的读取句柄；句柄构造失败时返回相应错误
    pub(super) fn make_handle(
        archive_type: ArchiveType,
        path: &Path,
        file: fs::File,
    ) -> CoreResult<Box<dyn ArchiveHandle>> {
        Ok(match archive_type {
            ArchiveType::Zip => Box::new(ZipReader::new(file, path.to_path_buf())?),
            ArchiveType::R7Z => Box::new(R7zReader::new(file, path.to_path_buf())?),
            ArchiveType::Tar | ArchiveType::TarGz | ArchiveType::TarXz => {
                Box::new(TarReader::new(file, archive_type, path.to_path_buf())?)
            }
        })
    }

    /// 在磁盘上创建一个只含中央目录、零条目的空 zip 文件
    ///
    /// - `path`: 输出文件路径
    ///
    /// # 返回值
    ///
    /// 创建失败时返回相应错误
    fn create_empty_zip(path: &Path) -> CoreResult<()> {
        use zip::ZipWriter;

        let file = fs::File::create(path).map_err(|err| fs_err(path.to_path_buf(), err))?;

        let zip = ZipWriter::new(file);
        zip.finish().map_err(write_error)?;

        Ok(())
    }

    /// 在磁盘上创建一个空的 tar 文件，内容为两个零块 EOF 标记
    ///
    /// - `path`: 输出文件路径
    ///
    /// # 返回值
    ///
    /// 创建失败时返回相应错误
    fn create_empty_tar(path: &Path) -> CoreResult<()> {
        use tar::Builder;

        let file = fs::File::create(path).map_err(|err| fs_err(path.to_path_buf(), err))?;

        let mut builder = Builder::new(file);
        builder.finish().map_err(write_error)?;

        Ok(())
    }
}
