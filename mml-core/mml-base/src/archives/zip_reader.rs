//! Zip 压缩/解压实现，基于 `zip` crate

use std::{
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::path_helper;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::archives::{
    self, ArchiveAddFile, ArchiveEntryInfo, ArchiveHandle, ArchiveProcess, BaseArchiveGui,
    IBaseArchiveGui, archive_err, fs_err, open_error, read_error, root_path_or_pack, write_error,
};

/// 保持打开的 Zip 读取句柄，缓存 [`ZipArchive`]，中央目录只解析一次
///
/// 同时提供静态的压缩/解压批处理入口
pub(crate) struct ZipReader {
    /// 读写的文件句柄，追加与重建时克隆使用
    file: fs::File,
    /// 缓存的中央目录句柄，由 `file` 克隆构建
    zip: ZipArchive<fs::File>,
    /// 压缩包在磁盘上的路径
    path: PathBuf,
}

impl ZipReader {
    /// 压缩目录为 zip 文件
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
        Self::zip(
            &process,
            archive_file,
            pack_dir,
            root_path_or_pack(root_path, pack_dir),
            filter,
        )
    }

    /// 解压 zip 文件到指定目录
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
        Self::unzip(&process, archive_file, output_dir)
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
    fn zip(
        process: &ArchiveProcess,
        archive_file: &Path,
        pack_dir: &Path,
        root_path: &Path,
        filter: &Option<Vec<String>>,
    ) -> CoreResult<()> {
        let file = path_helper::open_write(archive_file)?;
        let mut zip = ZipWriter::new(file);
        let files = path_helper::get_all_files(pack_dir);

        process.set_count(files.len());

        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for path in files {
            process.add_now(&path);

            if let Some(patterns) = filter {
                if archives::should_exclude(&path, patterns) {
                    continue;
                }
            }

            if !path.is_dir() {
                let mut buffer = path_helper::open_read(&path)?;

                let relative_path = path.strip_prefix(root_path).unwrap();
                let tempfile = archives::normalize_path(relative_path);

                zip.start_file(&tempfile, options).map_err(|err| {
                    archive_err(path.to_string_lossy().to_string(), tempfile.clone(), err)
                })?;

                std::io::copy(&mut buffer, &mut zip).map_err(|err| {
                    archive_err(path.to_string_lossy().to_string(), tempfile.clone(), err)
                })?;
            } else {
                let relative_path = path.strip_prefix(root_path).unwrap();
                let tempfile = archives::normalize_path(relative_path);

                zip.add_directory(&tempfile, options).map_err(|err| {
                    archive_err(path.to_string_lossy().to_string(), tempfile.clone(), err)
                })?;
            }
        }

        Ok(())
    }

    /// 解压实现，逐条目落盘并推进进度
    ///
    /// - `process`: 进度追踪器
    /// - `archive_file`: 压缩包文件路径
    /// - `output_dir`: 解压输出目录
    ///
    /// # 返回值
    ///
    /// 解压失败时返回相应错误
    fn unzip(process: &ArchiveProcess, archive_file: &Path, output_dir: &Path) -> CoreResult<()> {
        let file = path_helper::open_read(archive_file)?;
        let mut archive = ZipArchive::new(file).map_err(|err| open_error(archive_file, err))?;
        process.set_count(archive.len());

        path_helper::create_dir_all(output_dir)?;
        let output_dir_canonical = output_dir
            .canonicalize()
            .map_err(|err| fs_err(output_dir.to_path_buf(), err))?;

        // 收集需要恢复权限的路径，Unix 下先设为可写，最后再改回只读
        #[cfg(unix)]
        let mut unix_modes = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(read_error)?;

            // 安全检查：获取安全的输出路径
            let outpath = match file.enclosed_name() {
                Some(path) => path,
                None => continue, // 跳过不安全的路径
            };

            // 文件名非法时询问 GUI 是否替换
            let outpath = path_helper::safe_join(
                &output_dir_canonical,
                &process.check_name(&outpath.to_string_lossy())?,
            )?;

            process.add_now(&outpath);

            if file.is_dir() {
                path_helper::create_dir_all(&outpath)?;
                // Unix 下目录要保持可写直到子文件提取完，最后统一恢复权限
                #[cfg(unix)]
                if let Some(mode) = file.unix_mode() {
                    // 临时设为 0o700 保证可写

                    use crate::archives::set_perms;
                    set_perms(&outpath, 0o700)
                        .map_err(|err| fs_err(output_dir.to_path_buf(), err))?;

                    unix_modes.push((outpath, mode));
                }
                continue;
            }

            if file.is_symlink() {
                // 读取链接目标
                let mut target = Vec::new();
                file.read_to_end(&mut target).map_err(read_error)?;
                let target_str = String::from_utf8(target).map_err(read_error)?;
                let resolved = resolve_link_target(&output_dir_canonical, &outpath, &target_str)?;
                if let Some(parent) = outpath.parent() {
                    path_helper::create_dir_all(parent)?;
                }
                make_symlink(&outpath, &target_str, &resolved)?;
                continue;
            }

            // 普通文件
            let now = process.now.fetch_add(1, Ordering::SeqCst) + 1;

            if let Some(gui) = &process.gui {
                let filename = outpath
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                gui.update(Some(filename), now);
            }

            if let Some(parent) = outpath.parent() {
                let res = fs::create_dir_all(parent);
                if let Err(err) = res {
                    return Err(fs_err(parent.to_path_buf(), err));
                }
            }
            let mut outfile = path_helper::open_write(&outpath)?;
            io::copy(&mut file, &mut outfile).map_err(|err| {
                archive_err(
                    file.name().unwrap_or_default().to_string(),
                    outpath.display().to_string(),
                    err,
                )
            })?;

            // 保留 Unix 权限
            #[cfg(unix)]
            if let Some(mode) = file.unix_mode() {
                use crate::archives::set_perms;

                set_perms(&outpath, mode).map_err(|err| fs_err(outpath.to_path_buf(), err))?;
            }

            // 保留修改时间，需要 chrono feature
            if let Some(last_modified) = file.last_modified() {
                if let Some(system_time) = datetime_to_systemtime(&last_modified) {
                    outfile
                        .set_modified(system_time)
                        .map_err(|err| fs_err(outpath.to_path_buf(), err))?;
                }
            }
        }

        // 恢复所有目录的最终权限，Unix 下最后才设为只读
        #[cfg(unix)]
        for (path, mode) in unix_modes {
            use crate::archives::set_perms;

            set_perms(&path, mode).map_err(|err| fs_err(path.clone(), err))?;
        }

        process.done();

        Ok(())
    }

    /// 打开 zip 文件并缓存中央目录
    ///
    /// - `file`: 已打开的 zip 文件句柄
    /// - `path`: 压缩包磁盘路径
    ///
    /// # 返回值
    ///
    /// 中央目录解析失败时返回相应错误
    pub(crate) fn new(file: fs::File, path: PathBuf) -> CoreResult<Self> {
        let zip = ZipArchive::new(file.try_clone().map_err(|err| fs_err(path.clone(), err))?)
            .map_err(|err| open_error(&path, err))?;
        Ok(Self { file, zip, path })
    }

    /// 追加后用持有的读写句柄重新解析中央目录
    ///
    /// # 返回值
    ///
    /// 重新解析失败时返回相应错误
    fn reload(&mut self) -> CoreResult<()> {
        let mut file = self
            .file
            .try_clone()
            .map_err(|err| fs_err(self.path.clone(), err))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|err| fs_err(self.path.clone(), err))?;
        self.zip = ZipArchive::new(file).map_err(|err| open_error(&self.path, err))?;
        Ok(())
    }
}

impl ArchiveHandle for ZipReader {
    /// 逐条读取中央目录，得到条目名、是否目录与未压缩大小
    fn read_entries(&mut self) -> CoreResult<Vec<ArchiveEntryInfo>> {
        let mut entries = Vec::with_capacity(self.zip.len());
        for i in 0..self.zip.len() {
            let entry = self.zip.by_index(i).map_err(read_error)?;
            entries.push(ArchiveEntryInfo {
                name: entry.name().unwrap_or_default().to_string(),
                is_dir: entry.is_dir(),
                size: entry.size(),
            });
        }
        Ok(entries)
    }

    /// 按名称读出条目内容，目录条目视为错误
    fn read(&mut self, name: &str) -> CoreResult<Vec<u8>> {
        let mut entry = self.zip.by_name(name).map_err(read_error)?;

        if entry.is_dir() {
            return Err(read_error(name));
        }

        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(read_error)?;
        Ok(buf)
    }

    /// zip 无法真正流式读取，先整条读进内存再包成游标
    fn read_stream(&mut self, name: &str) -> CoreResult<Box<dyn Read>> {
        let data = self.read(name)?;
        Ok(Box::new(std::io::Cursor::new(data)))
    }

    /// 提取单个条目到目标路径，进度按单文件上报
    fn extract_file(
        &mut self,
        name: &str,
        output_path: &Path,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        if let Some(gui) = gui {
            gui.start(1);
        }

        let mut entry = self.zip.by_name(name).map_err(read_error)?;

        let mut outfile = path_helper::open_write(output_path)?;
        std::io::copy(&mut entry, &mut outfile)
            .map_err(|err| archive_err(name.to_string(), output_path.display().to_string(), err))?;

        if let Some(gui) = gui {
            gui.update(Some(name.to_string()), 1);
        }

        Ok(())
    }

    /// 用 `new_append` 就地追加磁盘文件，再重解析中央目录
    fn add_files(&mut self, files: &[ArchiveAddFile]) -> CoreResult<()> {
        use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

        let file = self
            .file
            .try_clone()
            .map_err(|err| fs_err(self.path.clone(), err))?;
        let mut zip = ZipWriter::new_append(file).map_err(|err| open_error(&self.path, err))?;

        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for file in files {
            let internal_path = file.dest.to_string_lossy();
            let mut reader = path_helper::open_read(&file.src)?;

            zip.start_file(internal_path.as_ref(), options)
                .map_err(write_error)?;

            std::io::copy(&mut reader, &mut zip).map_err(write_error)?;
        }

        zip.finish().map_err(write_error)?;

        self.reload()
    }

    /// 用 `new_append` 就地追加内存数据，再重解析中央目录
    fn add_data(&mut self, name: &str, data: &[u8]) -> CoreResult<()> {
        use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

        let file = self
            .file
            .try_clone()
            .map_err(|err| fs_err(self.path.clone(), err))?;
        let mut zip = ZipWriter::new_append(file).map_err(|err| open_error(&self.path, err))?;

        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);

        zip.start_file(name, options).map_err(write_error)?;

        zip.write_all(data).map_err(write_error)?;

        zip.finish().map_err(write_error)?;

        self.reload()
    }
}

/// 将 zip 时间字段转换为 chrono 时间，字段无效时返回 `None`
fn generate_chrono_datetime(time: &DateTime) -> Option<chrono::NaiveDateTime> {
    if let Some(chrono_date) =
        chrono::NaiveDate::from_ymd_opt(time.year().into(), time.month().into(), time.day().into())
        && let Some(chrono_datetime) = chrono_date.and_hms_opt(
            time.hour().into(),
            time.minute().into(),
            time.second().into(),
        )
    {
        return Some(chrono_datetime);
    }
    None
}

/// 将 zip 时间字段转换为系统时间，字段无效时返回 `None`
fn datetime_to_systemtime(time: &DateTime) -> Option<std::time::SystemTime> {
    if let Some(chrono_datetime) = generate_chrono_datetime(time) {
        let time = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            chrono_datetime,
            chrono::Utc,
        );
        return Some(time.into());
    }
    None
}

/// 创建符号链接，各平台各自实现
///
/// - `target_path`: 符号链接的创建路径
/// - `link_target`: 链接目标
///
/// # 返回值
///
/// 创建失败时返回相应错误
#[cfg(unix)]
fn make_symlink(target_path: &Path, link_target: &str, _resolved: &Path) -> CoreResult<()> {
    std::os::unix::fs::symlink(link_target, target_path)
        .map_err(|err| fs_err(target_path.to_path_buf(), err))
}

/// 创建符号链接，Windows 下按目标类型选择目录或文件链接
#[cfg(windows)]
fn make_symlink(target_path: &Path, link_target: &str, resolved: &Path) -> CoreResult<()> {
    let target = Path::new(link_target);
    if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, target_path)
    } else {
        std::os::windows::fs::symlink_file(target, target_path)
    }
    .map_err(|err| fs_err(target_path.to_path_buf(), err))
}

/// 创建符号链接，不支持的平台退化为写普通文件
#[cfg(not(any(unix, windows)))]
fn make_symlink(target_path: &Path, link_target: &str, _resolved: &Path) -> CoreResult<()> {
    // 不支持符号链接的平台：写为普通文件，内容为链接目标
    let mut f =
        fs::File::create(target_path).map_err(|err| fs_err(target_path.to_path_buf(), err))?;
    f.write_all(link_target.as_bytes())
        .map_err(|err| fs_err(target_path.to_path_buf(), err))?;
    Ok(())
}

/// 按链接所在目录解析目标，允许包内的父目录引用，拒绝逃逸和链接链。
fn resolve_link_target(root: &Path, link: &Path, target: &str) -> CoreResult<PathBuf> {
    let reject = || ErrorType::UnsafePath {
        root: root.to_path_buf(),
        path: PathBuf::from(target),
    };
    let target = target.replace('\\', "/");
    if target.starts_with('/') || target.is_empty() {
        return Err(reject());
    }
    let mut relative = link
        .strip_prefix(root)
        .map_err(|_| reject())?
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    for part in target
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
    {
        if part == ".." {
            if !relative.pop() {
                return Err(reject());
            }
        } else {
            if part.as_bytes().get(1) == Some(&b':') {
                return Err(reject());
            }
            relative.push(part);
        }
    }
    path_helper::safe_join(root, &relative.to_string_lossy())
}

/// 设置文件或目录的 Unix 权限，仅 Unix 下编译
///
/// - `path`: 目标文件或目录路径
/// - `mode`: 权限位，如 `0o644`
///
/// # 返回值
///
/// 设置失败时返回相应错误
#[cfg(unix)]
fn set_perms(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}
