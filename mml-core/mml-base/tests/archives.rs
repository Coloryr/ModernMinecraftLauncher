use std::{collections::HashMap, fs, path::{Path, PathBuf}};

use mml_base::archives::{ArchiveAddFile, ArchiveType, BaseArchive, TarMode};

/// 对比两个文件夹是否一致
///
/// - `dir1`: 第一个文件夹
/// - `dir2`: 第二个文件夹
/// - `skip_patterns`: 对比时跳过的路径模式
///
/// # 返回值
///
/// 文件数量、相对路径与内容都一致时为 `true`
pub fn compare_folders(
    dir1: &PathBuf,
    dir2: &PathBuf,
    skip_patterns: &[String],
) -> bool {
    let mut differences = Vec::new();
    
    // 收集第一个文件夹的所有文件
    let files1 = collect_files_with_filter(dir1, dir1, skip_patterns);
    let files2 = collect_files_with_filter(dir2, dir2, skip_patterns);
    
    // 检查文件数量
    if files1.len() != files2.len() {
        differences.push(format!(
            "文件数量不一致: {} vs {}",
            files1.len(),
            files2.len()
        ));
    }
    
    // 检查缺失的文件
    for (rel_path, _) in &files1 {
        if !files2.contains_key(rel_path) {
            differences.push(format!("文件缺失: {} 在第二个文件夹中不存在", rel_path));
        }
    }
    
    for (rel_path, _) in &files2 {
        if !files1.contains_key(rel_path) {
            differences.push(format!("多余文件: {} 在第一个文件夹中不存在", rel_path));
        }
    }
    
    // 对比共同文件的内容
    for (rel_path, content1) in &files1 {
        if let Some(content2) = files2.get(rel_path) {
            if content1 != content2 {
                differences.push(format!("文件内容不一致: {}", rel_path));
            }
        }
    }
    
    differences.is_empty()
}

/// 收集文件夹中所有文件的内容，跳过符合模式的文件
///
/// - `current_dir`: 起始目录
/// - `root_dir`: 相对路径的基准目录
/// - `skip_patterns`: 需要跳过的路径模式
///
/// # 返回值
///
/// 键为相对路径、值为文件内容的映射
fn collect_files_with_filter(
    current_dir: &Path,
    root_dir: &Path,
    skip_patterns: &[String],
) -> HashMap<String, Vec<u8>> {
    let mut files = HashMap::new();
    collect_files_recursive(current_dir, root_dir, skip_patterns, &mut files);
    files
}

/// 递归收集文件内容到 `files`
///
/// - `current_dir`: 当前遍历目录
/// - `root_dir`: 相对路径的基准目录
/// - `skip_patterns`: 需要跳过的路径模式
/// - `files`: 收集结果，键为相对路径
fn collect_files_recursive(
    current_dir: &Path,
    root_dir: &Path,
    skip_patterns: &[String],
    files: &mut HashMap<String, Vec<u8>>,
) {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            
            if path.is_file() {
                // 计算相对路径
                let rel_path = path.strip_prefix(root_dir).unwrap_or(&path);
                let rel_path_str = normalize_path(rel_path);
                
                // 检查是否需要跳过
                if should_skip(&rel_path_str, skip_patterns) {
                    println!("  跳过对比: {}", rel_path_str);
                    continue;
                }
                
                // 读取文件内容
                if let Ok(content) = fs::read(&path) {
                    files.insert(rel_path_str, content);
                } else {
                    eprintln!("无法读取文件: {}", path.display());
                }
            } else if path.is_dir() {
                // 递归处理子目录
                collect_files_recursive(&path, root_dir, skip_patterns, files);
            }
        }
    }
}

/// 统一路径分隔符为 `/`
fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .to_string()
        .replace('\\', "/")
}

/// 检查文件是否命中跳过模式
///
/// - `file_path`: 归一化后的相对路径
/// - `patterns`: 跳过模式，支持 `*` 通配
///
/// # 返回值
///
/// 命中任一模式时为 `true`
fn should_skip(file_path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        let pattern_normalized = pattern.replace('\\', "/");
        
        // 支持简单的通配符
        if pattern_normalized.contains('*') {
            matches_wildcard(file_path, &pattern_normalized)
        } else {
            // 精确匹配或包含匹配
            file_path == pattern_normalized || 
            file_path.contains(&pattern_normalized) ||
            // 检查文件名是否匹配
            Path::new(file_path).file_name()
                .map(|name| name.to_string_lossy().to_string() == pattern_normalized)
                .unwrap_or(false)
        }
    })
}

/// 按 `/` 分段做通配符匹配，`*` 可出现在段内
///
/// - `text`: 待匹配的相对路径
/// - `pattern`: 模式串，`*` 为通配符
///
/// # 返回值
///
/// 段数与逐段匹配都通过时为 `true`
fn matches_wildcard(text: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    
    let text_parts: Vec<&str> = text.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    
    // 简单的模式匹配
    if pattern_parts.len() != text_parts.len() && !pattern.contains('*') {
        return false;
    }
    
    pattern_parts.iter().enumerate().all(|(i, p)| {
        if i >= text_parts.len() {
            return false;
        }
        if *p == "*" {
            true
        } else if p.contains('*') {
            let prefix = p.trim_end_matches('*');
            text_parts[i].starts_with(prefix)
        } else {
            text_parts[i] == *p
        }
    })
}

/// 压缩源目录为 zip 并断言成功
#[test]
fn zip() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let zip = file.join("test.zip");
    let filter = Some(vec![
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]);
    let res = BaseArchive::compress(ArchiveType::Zip, &zip, &source, Some(&source), &filter, None);
    assert!(res.is_ok());
}

/// 解压 zip 到目录并断言成功，依赖 `zip` 先产出压缩包
#[test]
#[ignore]
fn unzip() {
    let file = Path::new("test_run");
    let zip = file.join("test.zip");
    let dir = file.join("test_zip/");
    let res = BaseArchive::decompress(ArchiveType::Zip, &zip, &dir, None);
    assert!(res.is_ok());
}

/// 断言 zip 解压结果与源目录一致，依赖 `unzip` 先执行
#[test]
#[ignore]
fn zip_equal() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let dir1 = file.join("test_zip/");
    assert!(compare_folders(&source, &dir1, &[
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]));
}

/// 压缩源目录为 7z 并断言成功
#[test]
#[ignore]
fn r7z() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let zip = file.join("test.7z");
    let filter = Some(vec![
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]);
    let res = BaseArchive::compress(ArchiveType::R7Z, &zip, &source, Some(&source), &filter, None);
    assert!(res.is_ok());
}

/// 解压 7z 到目录并断言成功
#[test]
#[ignore]
fn unr7z() {
    let file = Path::new("test_run");
    let zip = file.join("test.7z");
    let dir = file.join("test_7z/");
    let res = BaseArchive::decompress(ArchiveType::R7Z, &zip, &dir, None);
    assert!(res.is_ok());
}

/// 断言 7z 解压结果与源目录一致
#[test]
#[ignore]
fn r7z_equal() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let dir1 = file.join("test_7z/");
    assert!(compare_folders(&source, &dir1, &[
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]));
}

/// 压缩源目录为 tar.gz 并断言成功
#[test]
#[ignore]
fn targz() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let zip = file.join("test.tar.gz");
    let filter = Some(vec![
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]);
    let res = BaseArchive::compress(ArchiveType::TarGz, &zip, &source, Some(&source), &filter, None);
    assert!(res.is_ok());
}

/// 解压 tar.gz 到目录并断言成功
#[test]
#[ignore]
fn untargz() {
    let file = Path::new("test_run");
    let zip = file.join("test.tar.gz");
    let dir = file.join("test_targz/");
    let res = BaseArchive::decompress(ArchiveType::TarGz, &zip, &dir, None);
    assert!(res.is_ok());
}

/// 断言 tar.gz 解压结果与源目录一致
#[test]
#[ignore]
fn targz_equal() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let dir1 = file.join("test_targz/");
    assert!(compare_folders(&source, &dir1, &[
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]));
}

/// 压缩源目录为 tar.xz 并断言成功
#[test]
#[ignore]
fn tarxz() {
   let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let zip = file.join("test.tar.xz");
    let filter = Some(vec![
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]);
    let res = BaseArchive::compress(ArchiveType::TarXz, &zip, &source, Some(&source), &filter, None);
    assert!(res.is_ok());
}

/// 解压 tar.xz 到目录并断言成功
#[test]
#[ignore]
fn untarxz() {
    let file = Path::new("test_run");
    let zip = file.join("test.tar.xz");
    let dir = file.join("test_tarxz/");
    let res = BaseArchive::decompress(ArchiveType::TarXz, &zip, &dir, None);
    assert!(res.is_ok());
}

/// 断言 tar.xz 解压结果与源目录一致
#[test]
#[ignore]
fn tarxz_equal() {
    let file = Path::new("test_run");
    let source = Path::new("tests").join("test_compress/");
    let dir1 = file.join("test_tarxz/");
    assert!(compare_folders(&source, &dir1, &[
        String::from("skip.text"),
        String::from("dir1/skip.text"),
    ]));
}

/// 校验 TarMode 按后缀识别 gz 与 xz，不支持的返回 `None`
#[test]
fn test_tar_mode_from_path() {
    // 测试 .tar.gz
    let path_gz = &Path::new("archive.tar.gz").to_path_buf();
    assert_eq!(TarMode::try_from_path(path_gz), Some(TarMode::Gz));
    
    // 测试 .tgz
    let path_tgz = &Path::new("archive.tgz").to_path_buf();
    assert_eq!(TarMode::try_from_path(path_tgz), Some(TarMode::Gz));
    
    // 测试 .tar.xz
    let path_xz = &Path::new("archive.tar.xz").to_path_buf();
    assert_eq!(TarMode::try_from_path(path_xz), Some(TarMode::Xz));
    
    // 测试 .txz
    let path_txz = &Path::new("archive.txz").to_path_buf();
    assert_eq!(TarMode::try_from_path(path_txz), Some(TarMode::Xz));
    
    // 测试不支持的类型
    let path_zip = &Path::new("archive.zip").to_path_buf();
    assert!(TarMode::try_from_path(path_zip).is_none());
}

/// 清理用例产生的 `test_run` 目录
#[test]
#[ignore]
fn remove_file() {
    let file = Path::new("test_run");
    fs::remove_dir_all(file).ok();
}

/// 校验 zip / 7z / tar.gz 的压缩、打开、读条目与单文件解压
#[test]
fn open_read_extract() {
    // 使用独立目录，避免与其它测试并行冲突
    let file = Path::new("test_run_read");
    fs::remove_dir_all(file).ok();
    // 构建源目录
    let source = file.join("src_read");
    fs::create_dir_all(source.join("sub")).unwrap();
    fs::write(source.join("readme.txt"), b"hello archive").unwrap();
    fs::write(source.join("sub/data.bin"), vec![1u8, 2, 3, 4]).unwrap();

    for (name, archive_type) in [
        ("t.zip", ArchiveType::Zip),
        ("t.7z", ArchiveType::R7Z),
        ("t.tar.gz", ArchiveType::TarGz),
    ] {
        let arc = file.join(name);
        // 压缩
        BaseArchive::compress(archive_type, &arc, &source, Some(&source), &None, None).unwrap();
        // 打开并查文件：条目名可能用 / 或 \ 分隔，按实际条目名查找
        let base = BaseArchive::open(&arc).unwrap();
        let readme = base
            .entries()
            .iter()
            .find(|e| e.name.ends_with("readme.txt"))
            .unwrap()
            .name
            .clone();
        let data = base
            .entries()
            .iter()
            .find(|e| e.name.ends_with("data.bin"))
            .unwrap()
            .name
            .clone();
        assert_eq!(base.read(&readme).unwrap(), b"hello archive");
        assert_eq!(base.read(&data).unwrap(), vec![1u8, 2, 3, 4]);
        // 单文件解压
        let out_dir = file.join(format!("out_{}", name.replace('.', "_")));
        base.extract_file(&data, out_dir.join("data.bin"), None)
            .unwrap();
        assert_eq!(
            fs::read(out_dir.join("data.bin")).unwrap(),
            vec![1u8, 2, 3, 4]
        );
        // 清理
        fs::remove_file(&arc).ok();
        fs::remove_dir_all(&out_dir).ok();
    }
    fs::remove_dir_all(file).ok();
}

/// 校验排除列表生效，且 zip 走并行解压路径
#[test]
fn extract_all_unselect_parallel() {
    // 独立目录，避免与其它测试并行冲突
    let file = Path::new("test_run_par");
    fs::remove_dir_all(file).ok();
    let source = file.join("src");
    fs::create_dir_all(source.join("a")).unwrap();
    fs::write(source.join("keep.txt"), b"keep").unwrap();
    fs::write(source.join("a/one.txt"), b"1").unwrap();
    fs::write(source.join("a/two.txt"), b"2").unwrap();

    let arc = file.join("par.zip");
    BaseArchive::compress(ArchiveType::Zip, &arc, &source, Some(&source), &None, None).unwrap();
    let base = BaseArchive::open(&arc).unwrap();

    // 排除 keep.txt，并行解压剩余文件，zip 走多线程路径
    let out = file.join("out");
    base.extract_all(&out, Some(vec!["keep.txt".to_string()]), None, None)
        .unwrap();

    assert!(!out.join("keep.txt").exists());
    assert_eq!(fs::read(out.join("a/one.txt")).unwrap(), b"1");
    assert_eq!(fs::read(out.join("a/two.txt")).unwrap(), b"2");

    fs::remove_dir_all(file).ok();
}

/// 校验单顶层目录的剥离，以及与排除项同时生效
#[test]
fn extract_all_strip_single_top_dir() {
    // 独立目录，避免与其它测试并行冲突
    let file = Path::new("test_run_strip");
    fs::remove_dir_all(file).ok();
    let source = file.join("src");
    fs::create_dir_all(source.join("wrapped/sub")).unwrap();
    fs::write(source.join("wrapped/level.dat"), b"world").unwrap();
    fs::write(source.join("wrapped/sub/region.bin"), b"1234").unwrap();

    let arc = file.join("strip.zip");
    BaseArchive::compress(ArchiveType::Zip, &arc, &source, Some(&source), &None, None).unwrap();
    let base = BaseArchive::open(&arc).unwrap();
    assert_eq!(base.single_top_dir(), Some("wrapped"));

    // 不传 strip_dir：保持原路径，wrapped 目录保留
    let out = file.join("out");
    base.extract_all(&out, None, None, None).unwrap();
    assert_eq!(fs::read(out.join("wrapped/level.dat")).unwrap(), b"world");
    assert_eq!(
        fs::read(out.join("wrapped/sub/region.bin")).unwrap(),
        b"1234"
    );

    // 传入 strip_dir：去掉 wrapped 层，直接解压到目标目录
    let out2 = file.join("out2");
    base.extract_all(&out2, None, Some("wrapped".to_string()), None)
        .unwrap();
    assert_eq!(fs::read(out2.join("level.dat")).unwrap(), b"world");
    assert_eq!(fs::read(out2.join("sub/region.bin")).unwrap(), b"1234");
    assert!(!out2.join("wrapped").exists());

    // 排除项与剥目录同时生效，排除仍按压缩包内完整条目名匹配
    let out3 = file.join("out3");
    base.extract_all(
        &out3,
        Some(vec!["wrapped/level.dat".to_string()]),
        Some("wrapped".to_string()),
        None,
    )
    .unwrap();
    assert!(!out3.join("level.dat").exists());
    assert_eq!(fs::read(out3.join("sub/region.bin")).unwrap(), b"1234");

    fs::remove_dir_all(file).ok();
}

/// 校验存在多个顶层目录时不剥离，原路径保留
#[test]
fn extract_all_keeps_multiple_top_dirs() {
    // 独立目录，避免与其它测试并行冲突
    let file = Path::new("test_run_multi");
    fs::remove_dir_all(file).ok();
    let source = file.join("src");
    fs::create_dir_all(source.join("a")).unwrap();
    fs::create_dir_all(source.join("b")).unwrap();
    fs::write(source.join("a/one.txt"), b"1").unwrap();
    fs::write(source.join("b/two.txt"), b"2").unwrap();

    let arc = file.join("multi.zip");
    BaseArchive::compress(ArchiveType::Zip, &arc, &source, Some(&source), &None, None).unwrap();
    let base = BaseArchive::open(&arc).unwrap();
    assert_eq!(base.single_top_dir(), None);

    // 多个顶层目录保持原路径
    let out = file.join("out");
    base.extract_all(&out, None, None, None).unwrap();
    assert_eq!(fs::read(out.join("a/one.txt")).unwrap(), b"1");
    assert_eq!(fs::read(out.join("b/two.txt")).unwrap(), b"2");

    fs::remove_dir_all(file).ok();
}

/// 校验纯 tar 的压缩与解压往返
#[test]
fn tar_plain_roundtrip() {
    // 独立目录，避免与其它测试并行冲突
    let file = Path::new("test_run_tar");
    fs::remove_dir_all(file).ok();
    let source = file.join("src");
    fs::create_dir_all(source.join("d")).unwrap();
    fs::write(source.join("a.txt"), b"plain tar").unwrap();
    fs::write(source.join("d/b.txt"), b"nested").unwrap();

    let arc = file.join("plain.tar");
    BaseArchive::compress(ArchiveType::Tar, &arc, &source, Some(&source), &None, None).unwrap();
    let out = file.join("out");
    BaseArchive::decompress(ArchiveType::Tar, &arc, &out, None).unwrap();

    assert_eq!(fs::read(out.join("a.txt")).unwrap(), b"plain tar");
    assert_eq!(fs::read(out.join("d/b.txt")).unwrap(), b"nested");

    fs::remove_dir_all(file).ok();
}

/// 校验 zip 与 tar 的就地追加磁盘文件与内存数据
#[test]
fn add_files_data_inplace() {
    // 独立目录，避免与其它测试并行冲突
    let file = Path::new("test_run_add");
    fs::remove_dir_all(file).ok();
    let source = file.join("src");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("a.txt"), b"a").unwrap();
    let extra = file.join("extra.txt");
    fs::write(&extra, b"extra").unwrap();

    // Zip 就地追加，复用已打开句柄
    let arc = file.join("a.zip");
    BaseArchive::compress(ArchiveType::Zip, &arc, &source, Some(&source), &None, None).unwrap();
    let mut base = BaseArchive::open(&arc).unwrap();
    base.add_files(
        &[ArchiveAddFile {
            src: extra.clone(),
            dest: PathBuf::from("extra.txt"),
        }],
        None,
    )
    .unwrap();
    assert_eq!(base.read("extra.txt").unwrap(), b"extra");
    assert_eq!(base.read("a.txt").unwrap(), b"a");
    base.add_data("data.bin", &[1u8, 2, 3]).unwrap();
    assert_eq!(base.read("data.bin").unwrap(), vec![1u8, 2, 3]);

    // Tar 就地追加
    let arc2 = file.join("a.tar");
    BaseArchive::compress(ArchiveType::Tar, &arc2, &source, Some(&source), &None, None).unwrap();
    let mut base2 = BaseArchive::open(&arc2).unwrap();
    base2
        .add_files(
            &[ArchiveAddFile {
                src: extra.clone(),
                dest: PathBuf::from("extra.txt"),
            }],
            None,
        )
        .unwrap();
    assert_eq!(base2.read("extra.txt").unwrap(), b"extra");
    base2.add_data("data.bin", &[9u8]).unwrap();
    assert_eq!(base2.read("data.bin").unwrap(), vec![9u8]);

    fs::remove_dir_all(file).ok();
}

/// 按顺序跑完整套压缩包用例，覆盖压缩、解压与一致性对比
#[test]
fn archive_test() {
    zip();
    r7z();
    targz();
    tarxz();

    unzip();
    unr7z();
    untargz();
    untarxz();

    zip_equal();
    r7z_equal();
    targz_equal();
    tarxz_equal();

    remove_file();
}