//! 解压
//!
//! 单条提取、批量解压（大 zip 走并行）、按条件映射输出路径。

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use mml_names::i18_items::error_type::{CoreResult, ErrorType};
use mml_sys::path_helper;

use super::util::strip_dir_prefix;
use super::{
    ArchiveEntryInfo, BaseArchive, ExtractTask, ZIP_PARALLEL_MAX_THREADS, ZIP_PARALLEL_MIN_SIZE,
};

use crate::archives::{
    ArchiveType, BaseArchiveGui, IBaseArchiveGui, archive_err, read_error, replace_invalid_name,
};

impl BaseArchive {
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

    /// 并行提取 zip 的一批条目
    ///
    /// 整个包只 mmap 一次，各线程在内存里各自解析一份中央目录 —— 不是每个线程去开一次文件，
    /// 所以没有"重开包、重读中央目录"的磁盘开销，解压本身（DEFLATE）才是并行的大头
    ///
    /// - `tasks`: 待提取任务列表，见 [`ExtractTask`]
    /// - `gui`: 可选的进度回调，每提取完一个文件触发一次
    ///
    /// # 返回值
    ///
    /// 任一任务失败时返回相应错误；映射失败时退回顺序解压
    fn extract_tasks_zip_parallel(
        &self,
        tasks: &[ExtractTask],
        gui: Option<&dyn IBaseArchiveGui>,
    ) -> CoreResult<()> {
        let file = path_helper::open_read(&self.path)?;
        // SAFETY: 只读映射，解压期间不写这个压缩包
        let map = match unsafe { memmap2::Mmap::map(&file) } {
            Ok(map) => map,
            // 映射不了（网络盘、权限）就按原来的顺序解压走
            Err(_) => return self.extract_tasks_sequential(tasks, gui),
        };
        let data: &[u8] = &map;

        let threads = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .min(ZIP_PARALLEL_MAX_THREADS)
            .clamp(1, tasks.len());
        if threads <= 1 {
            return self.extract_tasks_sequential(tasks, gui);
        }

        let current = AtomicUsize::new(0);
        let error: Mutex<Option<ErrorType>> = Mutex::new(None);

        std::thread::scope(|scope| {
            for chunk in tasks.chunks(tasks.len().div_ceil(threads)) {
                let current = &current;
                let error = &error;
                scope.spawn(move || {
                    let mut zip = match zip::ZipArchive::new(Cursor::new(data)) {
                        Ok(zip) => zip,
                        Err(err) => {
                            *error.lock().unwrap() = Some(read_error(err));
                            return;
                        }
                    };
                    for task in chunk {
                        if error.lock().unwrap().is_some() {
                            return;
                        }
                        // 先报名字再解压：大文件或慢盘时界面不能干等
                        if let Some(gui) = gui {
                            gui.update(Some(task.name.clone()), current.load(Ordering::SeqCst));
                        }
                        if let Some(parent) = task.output_path.parent()
                            && let Err(err) = path_helper::create_dir_all(parent)
                        {
                            *error.lock().unwrap() = Some(err);
                            return;
                        }
                        let copied =
                            zip.by_name(&task.name)
                                .map_err(read_error)
                                .and_then(|mut entry| {
                                    let mut outfile = path_helper::open_write(&task.output_path)?;
                                    std::io::copy(&mut entry, &mut outfile).map_err(|err| {
                                        archive_err(
                                            task.name.clone(),
                                            task.output_path.display().to_string(),
                                            err,
                                        )
                                    })?;
                                    Ok(())
                                });
                        if let Err(err) = copied {
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

        match error.into_inner().unwrap() {
            Some(err) => Err(err),
            None => Ok(()),
        }
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
            // 先报名字再解压：等文件写完才报的话，首个条目偏大时界面会空等很久
            if let Some(gui) = gui {
                gui.update(Some(task.name.clone()), current);
            }
            self.extract_resolved(&task.name, &task.output_path, None)?;
            current += 1;
            if let Some(gui) = gui {
                gui.update(Some(task.name.clone()), current);
            }
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
        // 解压前把根的规范路径算一次：几万条目逐个 canonicalize 是纯开销
        let canonical_root = output_dir.canonicalize().ok();

        // 需要剥离目录或存在排除项时逐条提取，批量解压路径无法调整条目位置
        if strip_dir.is_some() || unselect.is_some() {
            return self.extract_mapped(
                |entry| {
                    if let Some(patterns) = &unselect {
                        if patterns.iter().any(|u| u == &entry.name) {
                            return Ok(None);
                        }
                    }
                    let rel = match &strip_dir {
                        Some(dir) => strip_dir_prefix(&entry.name, dir),
                        None => entry.name.clone(),
                    };
                    path_helper::safe_join_with_root(output_dir, canonical_root.as_deref(), &rel)
                        .map(Some)
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
    /// - `map`: 将条目映射为可信输出路径；调用方负责限制输出目录
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

    /// 收集并提取条目；指定根目录时同时校验重命名后的最终路径。
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
                    // 目标路径已经由上面的 safe_join 限制在 root 之内，这里只再确认一次
                    // 前缀：逐条重跑 safe_join 要为几万个条目再做一遍链接检查，纯开销
                    output
                        .strip_prefix(root)
                        .map_err(|_| ErrorType::UnsafePath {
                            root: root.to_path_buf(),
                            path: output.clone(),
                        })?;
                }
                tasks.push(ExtractTask {
                    name: entry.name.clone(),
                    output_path: output,
                });
            }
        }

        if let Some(gui) = gui {
            gui.start(tasks.len());
            // 并行解压要先给每个线程开一遍压缩包，大包在机械盘上这一段不短：
            // 先把第一条名字亮出来，界面不至于只看到一条不动的进度条
            if let Some(first) = tasks.first() {
                gui.update(Some(first.name.clone()), 0);
            }
        }

        // zip 大包走 mmap 并行：整包只映射一次，线程间共享同一份字节，解压并行；
        // 其余格式与太小的包走顺序解压，复用已经打开的句柄
        let parallel = self.archive_type == ArchiveType::Zip
            && tasks.len() > 1
            && self
                .path
                .metadata()
                .map(|meta| meta.len() >= ZIP_PARALLEL_MIN_SIZE)
                .unwrap_or(false);
        let result = if parallel {
            self.extract_tasks_zip_parallel(&tasks, gui)
        } else {
            self.extract_tasks_sequential(&tasks, gui)
        };

        if let Some(gui) = gui {
            gui.done();
        }
        result
    }
}
