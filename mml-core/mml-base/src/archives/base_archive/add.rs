//! 写入
//!
//! 追加文件 / 数据 / 流；tar 与 7z 需要解包重组，zip 直接追加。

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use mml_names::i18_items::error_type::CoreResult;
use mml_sys::path_helper;
use uuid::Uuid;

use super::BaseArchive;
use super::util::replace_archive;

use crate::archives::{ArchiveAddFile, ArchiveType, BaseArchiveGui, fs_err, write_error};

impl BaseArchive {
    /// 向压缩包添加文件并原地保存，已有同路径条目会被覆盖
    ///
    /// Zip 和 Tar 格式下文件直接追加，无需解压或重压缩已有条目；
    /// 7z 和压缩 tar 格式则先解压到临时目录，合并文件后重压缩
    ///
    /// - `files`: 待追加的文件列表，见 [`ArchiveAddFile`]
    /// - `gui`: 可选的进度回调，仅在重压缩时使用
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误；成功后内部条目列表自动刷新
    pub fn add_files(&mut self, files: &[ArchiveAddFile], gui: BaseArchiveGui) -> CoreResult<()> {
        if files.is_empty() {
            return Ok(());
        }
        // Zip / Tar 支持就地追加，复用已打开的文件句柄；其余解压后重压缩
        match self.archive_type {
            ArchiveType::Zip | ArchiveType::Tar => {
                let mut handle = self.handle.lock().unwrap();
                handle.add_files(files)?;
                self.entries = handle.read_entries()?;
                Ok(())
            }
            _ => self.add_files_extract_recompress(files, gui),
        }
    }

    /// 批量追加磁盘文件，逐条报进度并随时可取消
    ///
    /// 与一条条调 [`add_file`](Self::add_file) 的区别在开销：zip 每追加一条都要重写一次
    /// 中央目录并重读一遍，上千个文件就是平方级的开销；这里一次打开、写完、收尾。
    ///
    /// - `files`: 待追加的文件列表，见 [`ArchiveAddFile`]
    /// - `gui`: 进度回调，`None` 时不上报也不检查取消
    /// - `offset`: 进度起点，回调里的序号加在它上面
    ///
    /// # 返回值
    ///
    /// 写入失败或被取消时返回相应错误
    pub fn add_files_reporting(
        &mut self,
        files: &[ArchiveAddFile],
        gui: BaseArchiveGui,
        offset: usize,
    ) -> CoreResult<()> {
        if files.is_empty() {
            return Ok(());
        }
        match self.archive_type {
            ArchiveType::Zip => {
                let mut handle = self.handle.lock().unwrap();
                handle.add_files_reporting(files, &gui, offset)?;
                self.entries = handle.read_entries()?;
                Ok(())
            }
            _ => self.add_files(files, gui),
        }
    }

    /// 7z / tar.gz / tar.xz 的后备路径：解压到临时目录，写入文件后重压缩
    ///
    /// - `files`: 待追加的文件列表，见 [`ArchiveAddFile`]
    /// - `gui`: 可选的进度回调，仅在重压缩时使用
    ///
    /// # 返回值
    ///
    /// 任一环节失败时返回相应错误
    fn add_files_extract_recompress(
        &mut self,
        files: &[ArchiveAddFile],
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        // 创建临时目录用于解压和新文件
        let temp_dir = std::env::temp_dir().join(format!("mml_archive_{}", Uuid::new_v4()));
        path_helper::create_dir_all(&temp_dir)?;

        // 将已有压缩包解压到临时目录，空压缩包跳过
        if !self.entries.is_empty() {
            if let Err(err) = Self::decompress(self.archive_type, &self.path, &temp_dir, None) {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err(err);
            }
        }

        // 将新文件复制到临时目录
        for file in files {
            let dest_path = temp_dir.join(&file.dest);
            if let Some(parent) = dest_path.parent() {
                path_helper::create_dir_all(parent)?;
            }
            if let Err(err) = path_helper::copy_file(&file.src, &dest_path) {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err(err);
            }
        }

        // 临时包与原包放在同一目录，确保重命名替换不跨文件系统。
        let temp_archive = self
            .path
            .with_file_name(format!(".mml_archive_{}.tmp", Uuid::new_v4()));

        let compress_result = Self::compress_inner(
            self.archive_type,
            temp_archive.as_path(),
            temp_dir.as_path(),
            None::<&Path>,
            &None,
            gui,
        );

        // 无论成功与否都清理临时目录
        let _ = fs::remove_dir_all(&temp_dir);

        if let Err(err) = compress_result {
            let _ = fs::remove_file(&temp_archive);
            return Err(err);
        }

        // 用新压缩包替换原有压缩包
        replace_archive(&temp_archive, &self.path)?;

        // 刷新内部条目列表
        self.refresh_after_modify()?;

        Ok(())
    }

    /// 将内存中的文件添加到压缩包并原地保存，已有同路径条目会被覆盖
    ///
    /// Zip 和 Tar 格式下直接追加，无需解压或重压缩已有条目；
    /// 7z 和压缩 tar 格式则先解压到临时目录，写入数据后重压缩
    ///
    /// - `name`: 条目在压缩包内的路径，如 `"subdir/readme.txt"`
    /// - `data`: 文件内容的原始字节
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误；成功后内部条目列表自动刷新
    pub fn add_data(&mut self, name: &str, data: &[u8]) -> CoreResult<()> {
        // Zip / Tar 支持就地追加；其余解压后重压缩
        match self.archive_type {
            ArchiveType::Zip | ArchiveType::Tar => {
                let mut handle = self.handle.lock().unwrap();
                handle.add_data(name, data)?;
                self.entries = handle.read_entries()?;
                Ok(())
            }
            _ => self.add_data_extract_recompress(name, data),
        }
    }

    /// 7z / tar.gz / tar.xz 的后备路径：解压到临时目录，写入数据后重压缩
    ///
    /// - `name`: 条目在压缩包内的路径，如 `"subdir/readme.txt"`
    /// - `data`: 文件内容的原始字节
    ///
    /// # 返回值
    ///
    /// 任一环节失败时返回相应错误
    fn add_data_extract_recompress(&mut self, name: &str, data: &[u8]) -> CoreResult<()> {
        // 创建临时目录用于解压和新文件
        let temp_dir = std::env::temp_dir().join(format!("mml_archive_{}", Uuid::new_v4()));
        path_helper::create_dir_all(&temp_dir)?;

        // 将已有压缩包解压到临时目录，空压缩包跳过
        if !self.entries.is_empty() {
            if let Err(err) = Self::decompress(self.archive_type, &self.path, &temp_dir, None) {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err(err);
            }
        }

        // 将内存数据写入临时目录中的目标路径
        let dest_path = temp_dir.join(name);
        if let Some(parent) = dest_path.parent() {
            path_helper::create_dir_all(parent)?;
        }
        let mut file = path_helper::open_write(&dest_path)?;
        file.write_all(data)
            .map_err(|err| fs_err(dest_path.clone(), err))?;

        // 临时包与原包放在同一目录，确保重命名替换不跨文件系统。
        let temp_archive = self
            .path
            .with_file_name(format!(".mml_archive_{}.tmp", Uuid::new_v4()));

        let compress_result = Self::compress_inner(
            self.archive_type,
            temp_archive.as_path(),
            temp_dir.as_path(),
            None::<&Path>,
            &None,
            None,
        );

        // 无论成功与否都清理临时目录
        let _ = fs::remove_dir_all(&temp_dir);

        if let Err(err) = compress_result {
            let _ = fs::remove_file(&temp_archive);
            return Err(err);
        }

        // 用新压缩包替换原有压缩包
        replace_archive(&temp_archive, &self.path)?;

        // 刷新内部条目列表
        self.refresh_after_modify()?;

        Ok(())
    }

    /// 将磁盘上的单个文件添加到压缩包并原地保存，已有同路径条目会被覆盖
    ///
    /// Zip / Tar 格式下文件直接追加，无需解压或重压缩已有条目；
    /// 7z 和压缩 tar 格式则先解压到临时目录，复制文件后重压缩
    ///
    /// - `name`: 条目在压缩包内的路径，如 `"subdir/readme.txt"`
    /// - `path`: 磁盘上的源文件路径
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误；成功后内部条目列表自动刷新
    pub fn add_file<P: AsRef<Path>>(&mut self, name: &str, path: P) -> CoreResult<()> {
        self.add_files(
            &[ArchiveAddFile {
                src: path.as_ref().to_path_buf(),
                dest: PathBuf::from(name),
            }],
            None,
        )
    }

    /// 将一个流的内容添加到压缩包并原地保存，已有同路径条目会被覆盖
    ///
    /// tar 系格式需要预知条目大小，因此统一先把流写入临时文件，
    /// 再按 [`add_file`](Self::add_file) 的路径追加，内存占用恒定
    ///
    /// - `name`: 条目在压缩包内的路径，如 `"subdir/readme.txt"`
    /// - `stream`: 内容来源，读取到 EOF 为止
    ///
    /// # 返回值
    ///
    /// 写入失败时返回相应错误；成功后内部条目列表自动刷新
    pub fn add_stream(&mut self, name: &str, stream: &mut dyn Read) -> CoreResult<()> {
        let temp_file = std::env::temp_dir().join(format!("mml_stream_{}", Uuid::new_v4()));

        let result = (|| -> CoreResult<()> {
            let mut file = path_helper::open_write(&temp_file)?;
            std::io::copy(stream, &mut file).map_err(write_error)?;
            drop(file);
            self.add_file(name, &temp_file)
        })();

        let _ = fs::remove_file(&temp_file);
        result
    }

    /// 磁盘上的压缩包被替换后重新打开读取句柄并刷新条目
    ///
    /// # 返回值
    ///
    /// 重新打开或读条目失败时返回相应错误
    fn refresh_after_modify(&mut self) -> CoreResult<()> {
        let file = path_helper::open_read_write(&self.path)?;
        let mut handle = Self::make_handle(self.archive_type, &self.path, file)?;
        self.entries = handle.read_entries()?;
        *self.handle.lock().unwrap() = handle;
        Ok(())
    }
}
