//! mml-base 压缩包模块集成测试
//!
//! 覆盖 Zip / 7z / Tar / TarGz / TarXz 的压缩-解压往返、
//! [`BaseArchive`] 的打开/读取/提取/追加，以及排除与剥离目录等场景。
//!
//! 全部使用仓库 `target/temp` 下按进程 id + UUID 命名的唯一临时目录，
//! 测试结束自动清理，不依赖网络与固定路径。

use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use mml_base::archives::{
    ArchiveType, BaseArchive, IBaseArchiveGui, TarMode, compress, decompress,
};
use mml_sys::path_helper;

/// 在临时目录下创建本轮测试唯一根目录
fn make_test_root() -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_base_archives_test_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 构造源目录结构：
/// ```text
/// src/
///   a.txt        -> "hello"
///   sub/
///     b.txt      -> "world"
///     deep.txt   -> "deep"
/// ```
fn make_source_dir(root: &Path) -> PathBuf {
    let src = root.join("src");
    fs::create_dir_all(src.join("sub")).unwrap();
    fs::write(src.join("a.txt"), b"hello").unwrap();
    fs::write(src.join("sub").join("b.txt"), b"world").unwrap();
    fs::write(src.join("sub").join("deep.txt"), b"deep").unwrap();
    src
}

/// 各压缩包类型对应的文件后缀
fn ext_of(archive_type: ArchiveType) -> &'static str {
    match archive_type {
        ArchiveType::Zip => ".zip",
        ArchiveType::R7Z => ".7z",
        ArchiveType::Tar => ".tar",
        ArchiveType::TarGz => ".tar.gz",
        ArchiveType::TarXz => ".tar.xz",
    }
}

/// 所有支持的压缩包类型
fn all_types() -> Vec<ArchiveType> {
    vec![
        ArchiveType::Zip,
        ArchiveType::R7Z,
        ArchiveType::Tar,
        ArchiveType::TarGz,
        ArchiveType::TarXz,
    ]
}

/// 简单记录回调调用次数的 GUI
#[derive(Default)]
struct TestGui {
    /// `start` 回调调用次数
    start: AtomicUsize,
    /// `update` 回调调用次数
    update: AtomicUsize,
    /// `done` 回调调用次数
    done: AtomicUsize,
    /// `file_rename` 被询问的次数
    rename_asked: AtomicUsize,
    /// `file_rename` 的固定答复
    agree_rename: bool,
}

impl IBaseArchiveGui for TestGui {
    fn start(&self, _total: usize) {
        self.start.fetch_add(1, Ordering::SeqCst);
    }
    fn update(&self, _filename: Option<String>, _current: usize) {
        self.update.fetch_add(1, Ordering::SeqCst);
    }
    fn done(&self) {
        self.done.fetch_add(1, Ordering::SeqCst);
    }
    fn file_rename(&self, _name: &str) -> bool {
        self.rename_asked.fetch_add(1, Ordering::SeqCst);
        self.agree_rename
    }
}

/// 验证解压结果目录内容与源目录一致
fn assert_roundtrip_output(out_dir: &Path) {
    assert_eq!(fs::read(out_dir.join("a.txt")).unwrap(), b"hello");
    assert_eq!(
        fs::read(out_dir.join("sub").join("b.txt")).unwrap(),
        b"world"
    );
    assert_eq!(
        fs::read(out_dir.join("sub").join("deep.txt")).unwrap(),
        b"deep"
    );
}

/// 压缩-解压往返，走自由函数 compress / decompress
#[test]
fn test_compress_decompress_roundtrip() {
    for archive_type in all_types() {
        let root = make_test_root();
        let src = make_source_dir(&root);
        let archive_file = root.join(format!("round{}", ext_of(archive_type)));
        let out_dir = root.join("out");

        compress(
            archive_type,
            &archive_file,
            &src,
            None::<&PathBuf>,
            &None,
            None,
        )
        .unwrap();
        assert!(archive_file.exists(), "{:?} 压缩后文件应存在", archive_type);

        decompress(archive_type, &archive_file, &out_dir, None).unwrap();
        assert_roundtrip_output(&out_dir);

        let _ = fs::remove_dir_all(&root);
    }
}

/// 压缩时通过 filter 排除匹配的文件
#[test]
fn test_compress_with_filter() {
    for archive_type in all_types() {
        let root = make_test_root();
        let src = make_source_dir(&root);
        let archive_file = root.join(format!("filter{}", ext_of(archive_type)));
        let out_dir = root.join("out");

        compress(
            archive_type,
            &archive_file,
            &src,
            None::<&PathBuf>,
            &Some(vec!["deep.txt".to_string()]),
            None,
        )
        .unwrap();
        decompress(archive_type, &archive_file, &out_dir, None).unwrap();

        assert!(out_dir.join("a.txt").exists());
        assert!(!out_dir.join("sub").join("deep.txt").exists());

        let _ = fs::remove_dir_all(&root);
    }
}

/// ArchiveType / TarMode 的后缀推断
#[test]
fn test_archive_type_try_from_path() {
    assert_eq!(
        ArchiveType::try_from_path(Path::new("some/dir/pkg.zip")),
        Some(ArchiveType::Zip)
    );
    assert_eq!(
        ArchiveType::try_from_path(Path::new("a.7z")),
        Some(ArchiveType::R7Z)
    );
    assert_eq!(
        ArchiveType::try_from_path(Path::new("A.TAR.GZ")),
        Some(ArchiveType::TarGz)
    );
    assert_eq!(
        ArchiveType::try_from_path(Path::new("x.tgz")),
        Some(ArchiveType::TarGz)
    );
    assert_eq!(
        ArchiveType::try_from_path(Path::new("x.tar.xz")),
        Some(ArchiveType::TarXz)
    );
    assert_eq!(
        ArchiveType::try_from_path(Path::new("x.txz")),
        Some(ArchiveType::TarXz)
    );
    assert_eq!(
        ArchiveType::try_from_path(Path::new("x.tar")),
        Some(ArchiveType::Tar)
    );
    assert_eq!(ArchiveType::try_from_path(Path::new("x.rar")), None);

    assert_eq!(
        TarMode::try_from_path(Path::new("a.tar.gz")),
        Some(TarMode::Gz)
    );
    assert_eq!(
        TarMode::try_from_path(Path::new("a.tgz")),
        Some(TarMode::Gz)
    );
    assert_eq!(
        TarMode::try_from_path(Path::new("a.tar.xz")),
        Some(TarMode::Xz)
    );
    assert_eq!(
        TarMode::try_from_path(Path::new("a.txz")),
        Some(TarMode::Xz)
    );
    assert_eq!(TarMode::try_from_path(Path::new("a.zip")), None);
}

/// BaseArchive 打开后的条目 / 读取 / 提取
#[test]
fn test_base_archive_open_read_extract() {
    for archive_type in all_types() {
        let root = make_test_root();
        let src = make_source_dir(&root);
        let archive_file = root.join(format!("open{}", ext_of(archive_type)));

        let mut archive = BaseArchive::compress(
            archive_type,
            &archive_file,
            &src,
            None::<&PathBuf>,
            &None,
            None,
        )
        .unwrap_or_else(|e| panic!("{:?} 压缩失败: {:?}", archive_type, e));

        assert_eq!(archive.archive_type(), archive_type);
        assert_eq!(archive.path(), archive_file.as_path());

        // 条目
        let entries = archive.entries();
        assert_eq!(entries.len(), 3, "{:?} 条目数不符", archive_type);
        assert!(archive.contains("a.txt"));
        assert!(!archive.contains("no_such.txt"));

        // 单一顶层目录判断：源目录下有文件也有子目录 → 无统一顶层
        assert_eq!(archive.single_top_dir(), None);

        // 整体读取
        assert_eq!(archive.read("a.txt").unwrap(), b"hello".to_vec());
        assert_eq!(archive.read("sub/b.txt").unwrap(), b"world".to_vec());
        assert!(archive.read("no_such.txt").is_err());

        // 流式读取
        let mut stream = archive.read_stream("sub/b.txt").unwrap();
        let mut buf = String::new();
        stream.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "world");

        // 单文件提取
        let single_out = root.join("single");
        archive
            .extract_file("sub/b.txt", single_out.join("renamed.txt"), None)
            .unwrap();
        assert_eq!(fs::read(single_out.join("renamed.txt")).unwrap(), b"world");

        // 提取不存在的条目
        assert!(
            archive
                .extract_file("no_such.txt", single_out.join("x.txt"), None)
                .is_err()
        );

        // 追加内存数据后重新读取
        archive
            .add_data("added/from_memory.txt", b"memory data")
            .unwrap_or_else(|e| panic!("{:?} add_data 失败: {:?}", archive_type, e));
        assert!(archive.contains("added/from_memory.txt"));
        assert_eq!(
            archive.read("added/from_memory.txt").unwrap(),
            b"memory data".to_vec()
        );

        // 追加流数据
        archive
            .add_stream("added/from_stream.txt", &mut Cursor::new(b"stream data"))
            .unwrap_or_else(|e| panic!("{:?} add_stream 失败: {:?}", archive_type, e));
        assert_eq!(
            archive.read("added/from_stream.txt").unwrap(),
            b"stream data".to_vec()
        );

        // 追加磁盘文件：Zip 就地追加不允许同名条目，会报 Duplicate filename，
        // 因此这里只追加新名字，覆盖行为在下方单独测试中记录
        let extra = root.join("extra.txt");
        fs::write(&extra, b"extra content").unwrap();
        archive
            .add_files(
                &[(extra.clone(), PathBuf::from("added/from_disk.txt"))],
                None,
            )
            .unwrap_or_else(|e| panic!("{:?} add_files 失败: {:?}", archive_type, e));
        assert_eq!(
            archive.read("added/from_disk.txt").unwrap(),
            b"extra content".to_vec()
        );

        // 磁盘上的压缩包本身也被更新
        let reopened = BaseArchive::open(&archive_file).unwrap();
        assert_eq!(
            reopened.read("added/from_memory.txt").unwrap(),
            b"memory data".to_vec()
        );
        assert_eq!(
            reopened.read("added/from_disk.txt").unwrap(),
            b"extra content".to_vec()
        );

        let _ = fs::remove_dir_all(&root);
    }
}

/// extract_all：全量解压、排除条目、剥离顶层目录
#[test]
fn test_base_archive_extract_all_options() {
    let root = make_test_root();
    let src = make_source_dir(&root);
    let archive_file = root.join("opts.zip");

    let archive = BaseArchive::compress(
        ArchiveType::Zip,
        &archive_file,
        &src,
        None::<&PathBuf>,
        &None,
        None,
    )
    .unwrap();

    // 全量解压
    let out1 = root.join("out_all");
    archive.extract_all(&out1, None, None, None).unwrap();
    assert_roundtrip_output(&out1);

    // 排除指定条目
    let out2 = root.join("out_unselect");
    archive
        .extract_all(&out2, Some(vec!["sub/deep.txt".to_string()]), None, None)
        .unwrap();
    assert!(out2.join("a.txt").exists());
    assert!(!out2.join("sub").join("deep.txt").exists());

    // 剥离顶层目录：把所有条目包一层 "wrapped/" 再解压
    let wrapped_file = root.join("wrapped.zip");
    let wrapped_src = root.join("pkg");
    fs::create_dir_all(wrapped_src.join("wrapped").join("inner")).unwrap();
    fs::write(
        wrapped_src.join("wrapped").join("inner").join("w.txt"),
        b"w",
    )
    .unwrap();
    BaseArchive::compress(
        ArchiveType::Zip,
        &wrapped_file,
        &wrapped_src,
        None::<&PathBuf>,
        &None,
        None,
    )
    .unwrap();
    let wrapped = BaseArchive::open(&wrapped_file).unwrap();
    assert_eq!(wrapped.single_top_dir(), Some("wrapped"));

    let out3 = root.join("out_strip");
    wrapped
        .extract_all(&out3, None, Some("wrapped".to_string()), None)
        .unwrap();
    assert_eq!(fs::read(out3.join("inner").join("w.txt")).unwrap(), b"w");
    // 不剥离时保持原路径
    let out4 = root.join("out_no_strip");
    wrapped.extract_all(&out4, None, None, None).unwrap();
    assert!(out4.join("wrapped").join("inner").join("w.txt").exists());

    let _ = fs::remove_dir_all(&root);
}

/// create_empty + add_data 建包
#[test]
fn test_base_archive_create_empty() {
    for archive_type in all_types() {
        let root = make_test_root();
        let archive_file = root.join(format!("empty{}", ext_of(archive_type)));

        let mut archive = BaseArchive::create_empty(archive_type, &archive_file).unwrap();
        assert!(archive.entries().is_empty());

        archive
            .add_data("x.txt", b"xxx")
            .unwrap_or_else(|e| panic!("{:?} create_empty+add_data 失败: {:?}", archive_type, e));
        assert_eq!(archive.read("x.txt").unwrap(), b"xxx".to_vec());

        let _ = fs::remove_dir_all(&root);
    }
}

/// open 失败场景：不存在的文件 / 不支持的后缀
#[test]
fn test_base_archive_open_errors() {
    let root = make_test_root();

    // 不支持的后缀
    let bad = root.join("bad.rar");
    fs::write(&bad, b"not an archive").unwrap();
    assert!(BaseArchive::open(&bad).is_err());

    // 后缀正确但内容损坏
    let corrupt = root.join("corrupt.zip");
    fs::write(&corrupt, b"definitely not a zip").unwrap();
    assert!(BaseArchive::open(&corrupt).is_err());

    let _ = fs::remove_dir_all(&root);
}

/// 进度回调 GUI 在压缩与解压过程中被调用
#[test]
fn test_gui_callback() {
    let root = make_test_root();
    let src = make_source_dir(&root);
    let archive_file = root.join("gui.zip");
    let out_dir = root.join("out");

    let gui = Arc::new(TestGui::default());

    compress(
        ArchiveType::Zip,
        &archive_file,
        &src,
        None::<&PathBuf>,
        &None,
        Some(gui.clone() as Arc<dyn IBaseArchiveGui>),
    )
    .unwrap();
    assert!(gui.start.load(Ordering::SeqCst) >= 1);
    assert!(gui.update.load(Ordering::SeqCst) >= 3);
    // 注意现状不一致：各格式 compress 不调用 done 回调，仅 decompress 调用
    assert_eq!(gui.done.load(Ordering::SeqCst), 0);

    decompress(
        ArchiveType::Zip,
        &archive_file,
        &out_dir,
        Some(gui.clone() as Arc<dyn IBaseArchiveGui>),
    )
    .unwrap();
    assert!(gui.start.load(Ordering::SeqCst) >= 2);
    assert_eq!(gui.done.load(Ordering::SeqCst), 1);

    let _ = fs::remove_dir_all(&root);
}

/// 压缩时通过 root_path 使包内路径相对于指定根
#[test]
fn test_compress_with_root_path() {
    let root = make_test_root();
    let src = make_source_dir(&root); // root/src/...
    let archive_file = root.join("rooted.zip");
    let out_dir = root.join("out");

    // 以 root 为根打包，包内条目带 "src/" 前缀
    compress(
        ArchiveType::Zip,
        &archive_file,
        &src,
        Some(&root),
        &None,
        None,
    )
    .unwrap();
    let archive = BaseArchive::open(&archive_file).unwrap();
    assert!(archive.entries().iter().any(|e| e.name == "src/a.txt"));

    // 提取时保留 src/ 前缀
    decompress(ArchiveType::Zip, &archive_file, &out_dir, None).unwrap();
    assert_eq!(
        fs::read(out_dir.join("src").join("a.txt")).unwrap(),
        b"hello"
    );

    let _ = fs::remove_dir_all(&root);
}

/// 条目名含非法字符时通过 GUI 询问是否替换
#[test]
fn test_invalid_entry_name_with_gui() {
    let root = make_test_root();
    let archive_file = root.join("invalid_name.zip");

    // 手工构造一个条目名含非法字符的 zip
    {
        let file = fs::File::create(&archive_file).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("bad:name.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut writer, b"invalid name data").unwrap();
        writer.finish().unwrap();
    }

    let archive = BaseArchive::open(&archive_file).unwrap();
    assert!(archive.contains("bad:name.txt"));

    // GUI 同意替换：非法字符替换为下划线后正常提取
    let gui = Arc::new(TestGui {
        agree_rename: true,
        ..Default::default()
    });
    let out1 = root.join("out_agree");
    archive
        .extract_file(
            "bad:name.txt",
            out1.join("target.txt"),
            Some(&*gui as &dyn IBaseArchiveGui),
        )
        .unwrap();
    assert!(gui.rename_asked.load(Ordering::SeqCst) >= 1);
    assert_eq!(
        fs::read(out1.join("bad_name.txt")).unwrap(),
        b"invalid name data"
    );

    // GUI 拒绝替换：返回 TaskCancel，不写出文件
    let gui = Arc::new(TestGui {
        agree_rename: false,
        ..Default::default()
    });
    let out2 = root.join("out_refuse");
    let result = archive.extract_file(
        "bad:name.txt",
        out2.join("target.txt"),
        Some(&*gui as &dyn IBaseArchiveGui),
    );
    assert!(result.is_err());

    let _ = fs::remove_dir_all(&root);
}

/// 现状记录，疑似 bug：Zip 就地追加同名文件会报错，
/// 与 BaseArchive::add_files 文档所称的"已有同路径条目会被覆盖"不符
#[test]
fn test_zip_add_files_duplicate_name_documented() {
    let root = make_test_root();
    let src = make_source_dir(&root);
    let archive_file = root.join("dup.zip");

    let mut archive = BaseArchive::compress(
        ArchiveType::Zip,
        &archive_file,
        &src,
        None::<&PathBuf>,
        &None,
        None,
    )
    .unwrap();

    let extra = root.join("a2.txt");
    fs::write(&extra, b"other").unwrap();
    let result = archive.add_files(&[(extra.clone(), PathBuf::from("a.txt"))], None);
    assert!(result.is_err(), "Zip 同名追加应报错（现状）");

    // Tar 同名追加不会报错，tar 允许重复条目，此处不展开

    let _ = fs::remove_dir_all(&root);
}

/// path_helper 与压缩包路径配合：extract_file 自动创建父目录
#[test]
fn test_extract_file_creates_parent_dirs() {
    let root = make_test_root();
    let src = make_source_dir(&root);
    let archive_file = root.join("parent.zip");

    let archive = BaseArchive::compress(
        ArchiveType::Zip,
        &archive_file,
        &src,
        None::<&PathBuf>,
        &None,
        None,
    )
    .unwrap();

    // 目标父目录不存在，提取时应自动创建
    let target = root.join("deep").join("nested").join("b.txt");
    archive.extract_file("sub/b.txt", &target, None).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"world");

    // path_helper 读写辅助
    path_helper::write_text(root.join("t.txt"), "文本内容").unwrap();
    assert_eq!(
        path_helper::read_text(root.join("t.txt")).unwrap(),
        "文本内容"
    );

    let _ = fs::remove_dir_all(&root);
}
