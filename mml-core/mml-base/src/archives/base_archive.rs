//! 压缩包处理核心逻辑
//!
//! 实现压缩/解压的统一入口，根据 [`ArchiveType`] 分派到对应格式的读取器。

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType, FileSystemErrorData};
use mml_sys::path_helper;
use uuid::Uuid;

use crate::archives::{
    ArchiveAddFile, ArchiveHandle, ArchiveType, BaseArchiveGui, IBaseArchiveGui, TarMode, fs_err,
    r7z_reader::R7zReader, replace_invalid_name, tar_reader::TarReader, write_error,
    zip_reader::ZipReader,
};

/// 压缩包条目信息
#[derive(Debug, Clone)]
pub struct ArchiveEntryInfo {
    /// 条目名称，即压缩包内路径
    pub name: String,
    /// 是否为目录条目
    pub is_dir: bool,
    /// 未压缩大小，单位字节；目录条目恒为 `0`
    pub size: u64,
}

/// 统一的压缩包处理器，自动检测格式并提供读写功能
///
/// # 示例
///
/// ```ignore
/// use mml_base::archives::BaseArchive;
///
/// // 打开压缩包，根据后缀自动检测类型
/// let archive = BaseArchive::open("path/to/file.zip").unwrap();
///
/// // 遍历条目
/// for entry in archive.entries() {
///     println!("{} ({} bytes)", entry.name, entry.size);
/// }
///
/// // 提取单个文件
/// archive.extract_file("readme.txt", "output/readme.txt", None).unwrap();
///
/// // 提取全部文件
/// archive.extract_all("output_dir/", None, None, None).unwrap();
/// ```
pub struct BaseArchive {
    /// 压缩包在磁盘上的路径
    path: PathBuf,
    /// 打开时按文件名后缀判定出的压缩包格式
    archive_type: ArchiveType,
    /// 打开时读取并缓存的条目列表，增删后同步刷新
    entries: Vec<ArchiveEntryInfo>,
    /// 已打开的读取句柄，各格式实现见对应读取器
    handle: Mutex<Box<dyn ArchiveHandle>>,
}

/// 一条待提取任务，供批量解压的串行与并行路径共用
///
/// 目标路径已在收集阶段由 [`BaseArchive::resolve_output_path`] 解析完毕，
/// 提取时直接落盘，不再参与路径计算。
#[derive(Clone)]
struct ExtractTask {
    /// 条目在压缩包内的名称/路径，如 `assets/a.txt`
    name: String,
    /// 条目解包后写入的目标磁盘路径，已含重命名与非法字符处理结果
    output_path: PathBuf,
}

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
        Self::open(output_file)
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
        Self::open(output_file)
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
    fn compress_inner(
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
    fn make_handle(
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

    /// 返回检测到的压缩包类型
    ///
    /// # 返回值
    ///
    /// 打开时按文件名后缀判定出的格式
    pub fn archive_type(&self) -> ArchiveType {
        self.archive_type
    }

    /// 返回压缩包在磁盘上的路径
    ///
    /// # 返回值
    ///
    /// 打开时传入的文件路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回压缩包内所有条目
    ///
    /// 用于在不解压的情况下遍历压缩包内容
    ///
    /// # 返回值
    ///
    /// 返回打开时缓存的条目列表，增删后已同步刷新
    pub fn entries(&self) -> &[ArchiveEntryInfo] {
        &self.entries
    }

    /// 检查压缩包中是否存在指定名称的条目
    ///
    /// - `name`: 条目名，与包内路径完全匹配
    ///
    /// # 返回值
    ///
    /// 存在同名条目时为 `true`
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }

    /// 所有条目共享同一个顶层目录时返回该目录名
    ///
    /// 例如条目都以 `"MyWorld/"` 开头时返回 `Some("MyWorld")`
    ///
    /// # 返回值
    ///
    /// 根目录下直接有文件、或有多个顶层名字时返回 `None`
    pub fn single_top_dir(&self) -> Option<&str> {
        let mut top: Option<&str> = None;
        for entry in self.entries.iter() {
            let trimmed = entry.name.trim_end_matches(['/', '\\']);
            let mut parts = trimmed.split(['/', '\\']);
            let Some(first) = parts.next().filter(|s| !s.is_empty()) else {
                continue;
            };
            // 根目录下的单个文件也占一个首段，但它不是顶层目录
            if parts.next().is_none() && !entry.is_dir {
                return None;
            }
            match top {
                None => top = Some(first),
                Some(name) if name == first => {}
                Some(_) => return None,
            }
        }
        top
    }

    /// 读取压缩包内单个条目的内容到内存
    ///
    /// - `name`: 条目在压缩包内的名称/路径，如 `"subdir/readme.txt"`
    ///
    /// # 返回值
    ///
    /// 返回条目内容的原始字节；条目不存在、是目录或读取失败时返回错误
    pub fn read(&self, name: &str) -> CoreResult<Vec<u8>> {
        self.handle.lock().unwrap().read(name)
    }

    /// 以流式 [`Read`] 接口打开压缩包内的单个文件条目
    ///
    /// 与返回完整 `Vec<u8>` 的 [`read`](Self::read) 不同，此方法返回
    /// `Box<dyn Read>`，可直接传递给任何接受 [`Read`] trait 的函数
    ///
    /// - `name`: 条目在压缩包内的名称/路径，如 `"subdir/readme.txt"`
    ///
    /// # 返回值
    ///
    /// 返回只读流，由调用方负责读完或丢弃；条目不存在、是目录或读取失败时返回错误
    pub fn read_stream(&self, name: &str) -> CoreResult<Box<dyn Read>> {
        self.handle.lock().unwrap().read_stream(name)
    }

    /// 将压缩包中单个文件提取到指定输出路径
    ///
    /// - `name`: 条目在压缩包内的名称/路径，如 `"subdir/readme.txt"`
    /// - `output_path`: 目标磁盘路径，父目录会自动创建
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 条目不存在或提取失败时返回相应错误
    pub fn extract_file<P: AsRef<Path>>(
        &self,
        name: &str,
        output_path: P,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        let output_path =
            self.resolve_output_path(name, output_path.as_ref().to_path_buf(), gui)?;
        self.extract_resolved(name, &output_path, gui)
    }

    /// 解析最终输出路径，文件名非法时替换非法字符为 `_` 并询问 GUI 是否同意
    ///
    /// 仅替换目标路径的最后一个名称段
    ///
    /// - `name`: 条目在压缩包内的名称/路径
    /// - `output`: 原始目标磁盘路径
    /// - `gui`: 可选的重命名确认回调
    ///
    /// # 返回值
    ///
    /// 返回替换后的目标路径；GUI 不同意替换时返回 `TaskCancel`
    fn resolve_output_path(
        &self,
        name: &str,
        output: PathBuf,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<PathBuf> {
        if name
            .split(['/', '\\'])
            .filter(|seg| !seg.is_empty() && *seg != "." && *seg != "..")
            .any(|seg| path_helper::file_has_invalid_chars(seg))
        {
            let safe_name = replace_invalid_name(name);
            let new_name = match gui {
                Some(gui) => {
                    if gui.file_rename(name) {
                        safe_name
                    } else {
                        return Err(ErrorType::TaskCancel);
                    }
                }
                None => safe_name,
            };
            let new_file = Path::new(&new_name)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| name.to_string());
            Ok(output.with_file_name(new_file))
        } else {
            Ok(output)
        }
    }

    /// 提取单个条目到已解析的目标路径，并按需创建父目录
    ///
    /// - `name`: 条目在压缩包内的名称/路径
    /// - `output_path`: 已解析的目标磁盘路径
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 提取失败时返回相应错误
    fn extract_resolved(
        &self,
        name: &str,
        output_path: &Path,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        if let Some(parent) = output_path.parent() {
            path_helper::create_dir_all(parent)?;
        }
        self.handle
            .lock()
            .unwrap()
            .extract_file(name, output_path, gui)
    }

    /// 顺序提取一组条目
    ///
    /// - `tasks`: 待提取任务列表，见 [`ExtractTask`]
    /// - `gui`: 可选的进度回调，每提取完一个文件触发一次
    ///
    /// # 返回值
    ///
    /// 任一任务失败时返回相应错误
    fn extract_tasks_sequential(
        &self,
        tasks: &[ExtractTask],
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        let mut current = 0usize;
        for task in tasks {
            self.extract_resolved(&task.name, &task.output_path, None)?;
            current += 1;
            if let Some(gui) = gui {
                gui.update(Some(task.name.clone()), current);
            }
        }
        Ok(())
    }

    /// 并行提取一组条目，仅用于 Zip，每个线程持有独立读取句柄随机访问
    ///
    /// - `tasks`: 待提取任务列表，见 [`ExtractTask`]
    /// - `gui`: 可选的进度回调，每提取完一个文件触发一次
    ///
    /// # 返回值
    ///
    /// 任一任务失败时返回该错误；可用线程数不足 2 时退化为顺序提取
    fn extract_tasks_parallel(
        &self,
        tasks: &[ExtractTask],
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(tasks.len());
        if thread_count <= 1 {
            return self.extract_tasks_sequential(tasks, gui);
        }

        let current = Arc::new(AtomicUsize::new(0));
        let error: Arc<Mutex<Option<ErrorType>>> = Arc::new(Mutex::new(None));

        std::thread::scope(|scope| {
            for chunk in tasks.chunks(tasks.len().div_ceil(thread_count)) {
                let chunk = chunk.to_vec();
                let path = self.path.clone();
                let gui = gui;
                let current = current.clone();
                let error = error.clone();
                scope.spawn(move || {
                    // 任一线程出错则尽早退出
                    if error.lock().unwrap().is_some() {
                        return;
                    }
                    let file = match path_helper::open_read(&path) {
                        Ok(file) => file,
                        Err(err) => {
                            *error.lock().unwrap() = Some(err);
                            return;
                        }
                    };
                    let mut handle = match Self::make_handle(ArchiveType::Zip, &path, file) {
                        Ok(handle) => handle,
                        Err(err) => {
                            *error.lock().unwrap() = Some(err);
                            return;
                        }
                    };
                    for task in chunk {
                        if error.lock().unwrap().is_some() {
                            return;
                        }
                        if let Some(parent) = task.output_path.parent() {
                            if let Err(err) = path_helper::create_dir_all(parent) {
                                *error.lock().unwrap() = Some(err);
                                return;
                            }
                        }
                        if let Err(err) = handle.extract_file(&task.name, &task.output_path, None) {
                            *error.lock().unwrap() = Some(err);
                            return;
                        }
                        let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                        if let Some(gui) = gui {
                            gui.update(Some(task.name.clone()), now);
                        }
                    }
                });
            }
        });

        if let Some(err) = error.lock().unwrap().take() {
            return Err(err);
        }
        Ok(())
    }

    /// 将压缩包中所有文件提取到指定输出目录
    ///
    /// - `output_dir`: 目标目录
    /// - `unselect`: 可选的排除条目名列表，名字完全匹配的条目将被跳过
    /// - `strip_dir`: 可选的顶层包裹目录名；压缩包整体被 `dir/` 包裹时去掉该目录直接提取，
    ///   `dir/file.txt` 落到 `output_dir/file.txt`，避免多套一层文件夹；
    ///   条目不在该目录下时保持原路径
    /// - `gui`: 可选的进度回调
    ///
    /// # 返回值
    ///
    /// 提取失败时返回相应错误
    pub fn extract_all<P: AsRef<Path>>(
        &self,
        output_dir: P,
        unselect: Option<Vec<String>>,
        strip_dir: Option<String>,
        gui: BaseArchiveGui,
    ) -> CoreResult<()> {
        let output_dir = output_dir.as_ref();

        // 需要剥离目录或存在排除项时逐条提取，批量解压路径无法调整条目位置
        if strip_dir.is_some() || unselect.is_some() {
            return self.extract_mapped(
                |entry| {
                    if let Some(patterns) = &unselect {
                        if patterns.iter().any(|u| u == &entry.name) {
                            return Ok(None);
                        }
                    }
                    path_helper::safe_join(output_dir, &entry.name)?;
                    let rel = match &strip_dir {
                        Some(dir) => strip_dir_prefix(&entry.name, dir),
                        None => entry.name.clone(),
                    };
                    path_helper::safe_join(output_dir, &rel).map(Some)
                },
                Some(output_dir),
                gui.as_deref(),
            );
        }

        // 无排除项且无剥离目录时走 runner 的批量解压路径
        Self::decompress(self.archive_type, self.path.as_path(), output_dir, gui)
    }

    /// 按条件提取选中条目到由闭包计算的输出路径
    ///
    /// 闭包接收每个文件 [`ArchiveEntryInfo`]，返回 `Some(输出路径)` 则提取，
    /// 返回 `None` 则跳过；目录条目始终自动跳过
    ///
    /// Zip 格式下并行解压，每个线程持有独立读取句柄随机访问，其余格式流式解压保持顺序。
    /// 文件名非法时的 GUI 询问发生在任务收集阶段，保持串行
    ///
    /// - `map`: 将每个条目映射为可选输出路径的闭包
    /// - `gui`: 可选的进度回调，每提取完一个文件触发一次
    ///
    /// # 返回值
    ///
    /// 提取失败时返回相应错误
    pub fn extract_where<F: FnMut(&ArchiveEntryInfo) -> Option<PathBuf>>(
        &self,
        mut map: F,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        self.extract_mapped(|entry| Ok(map(entry)), None, gui)
    }

    /// 收集提取路径；指定根目录时，重命名后的最终路径也必须留在目录内。
    fn extract_mapped<F: FnMut(&ArchiveEntryInfo) -> CoreResult<Option<PathBuf>>>(
        &self,
        mut map: F,
        root: Option<&Path>,
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        // 顺序阶段：计算目标路径，文件名非法时的 GUI 询问保持串行
        let mut tasks: Vec<ExtractTask> = Vec::new();
        for entry in &self.entries {
            if entry.is_dir {
                continue;
            }
            if let Some(base) = map(entry)? {
                let output = self.resolve_output_path(&entry.name, base, gui)?;
                if let Some(root) = root {
                    let relative =
                        output
                            .strip_prefix(root)
                            .map_err(|_| ErrorType::UnsafePath {
                                root: root.to_path_buf(),
                                path: output.clone(),
                            })?;
                    path_helper::safe_join(root, &relative.to_string_lossy())?;
                }
                tasks.push(ExtractTask {
                    name: entry.name.clone(),
                    output_path: output,
                });
            }
        }

        if let Some(gui) = gui {
            gui.start(tasks.len());
        }

        let result = if self.archive_type == ArchiveType::Zip {
            self.extract_tasks_parallel(&tasks, gui)
        } else {
            self.extract_tasks_sequential(&tasks, gui)
        };

        if let Some(gui) = gui {
            gui.done();
        }
        result
    }

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

        // 先压缩到临时文件，再原子替换
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

        // 先压缩到临时文件，再原子替换
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

/// 去掉条目名指定的顶层包裹目录前缀
///
/// 例如 `dir/file.txt` 去掉 `dir` 后得到 `file.txt`，同时兼容 `\` 分隔的条目名
///
/// - `name`: 条目名
/// - `dir`: 要去掉的顶层包裹目录名
///
/// # 返回值
///
/// 条目不在该目录下、或本身就是 `dir` 时保持原名，避免误剥
fn strip_dir_prefix(name: &str, dir: &str) -> String {
    name.strip_prefix(&format!("{dir}/"))
        .or_else(|| name.strip_prefix(&format!("{dir}\\")))
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

/// 同目录重命名覆盖，失败时保留原包并清理临时包。
fn replace_archive(replacement: &Path, original: &Path) -> CoreResult<()> {
    fs::rename(replacement, original).map_err(|err| {
        let _ = fs::remove_file(replacement);
        fs_err(original.to_path_buf(), err)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_failure_keeps_original() {
        let root = mml_testutil::temp_path(format!("archive_replace_{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("original.tar.gz");
        fs::write(&original, b"original").unwrap();
        assert!(replace_archive(&root.join("missing.tmp"), &original).is_err());
        assert_eq!(fs::read(&original).unwrap(), b"original");
        let replacement = root.join("replacement.tmp");
        fs::write(&replacement, b"replacement").unwrap();
        replace_archive(&replacement, &original).unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"replacement");
        assert!(!replacement.exists());
    }
}
