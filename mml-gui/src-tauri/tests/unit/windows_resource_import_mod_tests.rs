//! 导入相关单元测试

use super::common::*;
use super::files::*;

use std::path::{Path, PathBuf};

use crate::test_support::{cleanup, ensure_boot, temp_dir};

/// 在 `dir` 下造一个内容为 `name` 的文件
fn write(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, name.as_bytes()).unwrap();
    path
}

/// 在 `dir` 下造一个**指定文件名与内容**的文件，覆盖 / 跳过 / 重命名那几个用例要区分这两者
fn write_as(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text.as_bytes()).unwrap();
    path
}

/// 预检：只报冲突，一个字节都不落盘，也不建目标目录
#[test]
fn check_reports_conflicts_without_writing() {
    ensure_boot();
    let root = temp_dir("import-check");
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let a = write(&src, "a.jar");
    let b = write(&src, "b.jar");
    // 目标目录都还不存在：预检不该顺手把它建出来
    let dst = root.join("mods");

    let result = copy_into(&dst, &[a, b], MODE_CHECK).unwrap();
    assert!(result.conflicts.is_empty());
    assert!(result.imported.is_empty());
    assert!(!dst.exists());

    cleanup(&[&root]);
}

/// 预检撞上同名：只报那一个，且不动盘上的东西
#[test]
fn check_reports_conflict_names() {
    ensure_boot();
    let root = temp_dir("import-conflict");
    let src = root.join("src");
    let dst = root.join("mods");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    let a = write(&src, "a.jar");
    let b = write(&src, "b.jar");
    let old = write(&dst, "a.jar");

    let result = copy_into(&dst, &[a, b], MODE_CHECK).unwrap();
    assert_eq!(result.conflicts, vec![String::from("a.jar")]);
    // 盘上那条还是老内容，预检不是"先删后报"
    assert_eq!(std::fs::read_to_string(&old).unwrap(), "a.jar");

    cleanup(&[&root]);
}

/// 没有冲突时直接导入，目标目录不存在就建出来
#[test]
fn import_creates_dir_and_copies() {
    ensure_boot();
    let root = temp_dir("import-copy");
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let a = write(&src, "a.jar");
    let dst = root.join("mods");
    assert!(!dst.exists());

    let result = copy_into(&dst, &[a], "overwrite").unwrap();
    assert_eq!(result.imported, vec![String::from("a.jar")]);
    assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "a.jar");

    cleanup(&[&root]);
}

/// 覆盖：老内容被新的顶掉，名字不变
#[test]
fn import_overwrites_same_name() {
    ensure_boot();
    let root = temp_dir("import-overwrite");
    let src = root.join("src");
    let dst = root.join("mods");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    let a = write_as(&src, "a.jar", "new");
    std::fs::write(dst.join("a.jar"), b"old").unwrap();

    let result = copy_into(&dst, &[a], "overwrite").unwrap();
    assert_eq!(result.imported, vec![String::from("a.jar")]);
    assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "new");

    cleanup(&[&root]);
}

/// 跳过：盘上那条原封不动，结果里记一笔
#[test]
fn import_skips_same_name() {
    ensure_boot();
    let root = temp_dir("import-skip");
    let src = root.join("src");
    let dst = root.join("mods");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    let a = write_as(&src, "a.jar", "new");
    std::fs::write(dst.join("a.jar"), b"old").unwrap();

    let result = copy_into(&dst, &[a], MODE_SKIP).unwrap();
    assert_eq!(result.skipped, vec![String::from("a.jar")]);
    assert!(result.imported.is_empty());
    assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "old");

    cleanup(&[&root]);
}

/// 重命名：老的留着，新的按 ` (n)` 递增找空位，`a (1)` 占了就给 `a (2)`
#[test]
fn import_renames_with_next_free_index() {
    ensure_boot();
    let root = temp_dir("import-rename");
    let src = root.join("src");
    let dst = root.join("mods");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    let a = write_as(&src, "a.jar", "new");
    std::fs::write(dst.join("a.jar"), b"old").unwrap();
    std::fs::write(dst.join("a (1).jar"), b"older").unwrap();

    let result = copy_into(&dst, &[a], MODE_RENAME).unwrap();
    assert_eq!(result.imported, vec![String::from("a (2).jar")]);
    assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "old");
    assert_eq!(
        std::fs::read_to_string(dst.join("a (2).jar")).unwrap(),
        "new"
    );

    cleanup(&[&root]);
}

/// 一次拖入**多个文件**要全部落盘，不是只导第一个，冲突也逐个判
#[test]
fn import_multiple_files_at_once() {
    ensure_boot();
    let root = temp_dir("import-multi");
    let src = root.join("src");
    let dst = root.join("mods");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    let a = write_as(&src, "a.jar", "A");
    let b = write_as(&src, "b.jar", "B");
    let c = write_as(&src, "c.jar", "C");
    // b.jar 的目标位置已有同名：选"跳过"只该跳它一个
    std::fs::write(dst.join("b.jar"), b"old").unwrap();

    let result = copy_into(&dst, &[a, b, c], MODE_SKIP).unwrap();
    assert_eq!(
        result.imported,
        vec![String::from("a.jar"), String::from("c.jar")]
    );
    assert_eq!(result.skipped, vec![String::from("b.jar")]);
    assert_eq!(std::fs::read_to_string(dst.join("a.jar")).unwrap(), "A");
    assert_eq!(std::fs::read_to_string(dst.join("c.jar")).unwrap(), "C");
    assert_eq!(std::fs::read_to_string(dst.join("b.jar")).unwrap(), "old");

    cleanup(&[&root]);
}

/// 目录一律跳过：不导入、不报错，这是"拖目录不显示导入标志"那条的兜底
#[test]
fn directories_are_skipped() {
    ensure_boot();
    let root = temp_dir("import-dir");
    let dst = root.join("mods");
    std::fs::create_dir_all(&dst).unwrap();
    let folder = root.join("some-pack");
    std::fs::create_dir_all(&folder).unwrap();
    write(&folder, "inside.jar");

    let result = copy_into(&dst, &[folder], "overwrite").unwrap();
    assert!(result.imported.is_empty());
    assert!(result.skipped.is_empty());
    assert_eq!(std::fs::read_dir(&dst).unwrap().count(), 0);

    cleanup(&[&root]);
}

/// 没扩展名 / 多个点的文件名不被 ` (n)` 规则弄坏
#[test]
fn rename_keeps_name_without_extension() {
    ensure_boot();
    let root = temp_dir("import-rename-noext");
    let src = root.join("src");
    let dst = root.join("mods");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    let a = write(&src, "servers.dat");
    std::fs::write(dst.join("servers.dat"), b"old").unwrap();

    let result = copy_into(&dst, &[a], MODE_RENAME).unwrap();
    assert_eq!(result.imported, vec![String::from("servers (1).dat")]);

    cleanup(&[&root]);
}
