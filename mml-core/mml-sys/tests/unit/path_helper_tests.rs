use super::*;
use std::io::Cursor;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use mml_names::i18_items::error_type::CoreResult;

use crate::path_helper::trash::{move_to_trash_linux, move_to_trash_macos};

#[test]
fn trash_backends_share_path_result_contract() {
    // 只编译并核对各平台接口，不调用系统回收站或启动 Finder。
    let _: fn(&Path) -> CoreResult<()> = move_to_trash_linux;
    let _: fn(&Path) -> CoreResult<()> = move_to_trash_macos;
    let missing = mml_testutil::temp_path(format!("trash_missing_{}", uuid::Uuid::new_v4()));
    assert!(move_to_trash(missing).is_ok());
}

/// 在临时目录创建本轮测试唯一目录
fn make_test_root(tag: &str) -> PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml_sys_path_test_{}_{}_{}",
        tag,
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// write_text / read_text 往返，以及整文件重写的截断语义
#[test]
fn test_write_read_text() {
    let root = make_test_root("text");
    let file = root.join("a.txt");

    write_text(&file, "第一行\n第二行").unwrap();
    assert_eq!(read_text(&file).unwrap(), "第一行\n第二行");

    // 新内容比旧内容短时应截断而不是残留旧尾巴
    write_text(&file, "b").unwrap();
    assert_eq!(read_text(&file).unwrap(), "b");

    // 写入时自动创建父目录
    let nested = root.join("x").join("y").join("c.txt");
    write_text(&nested, "nested").unwrap();
    assert_eq!(read_text(&nested).unwrap(), "nested");

    // 读取不存在的文件报错
    assert!(read_text(root.join("no_such.txt")).is_err());

    let _ = fs::remove_dir_all(&root);
}

/// write_bytes / read_byte / write_stream
#[test]
fn test_write_read_bytes() {
    let root = make_test_root("bytes");
    let file = root.join("bin.dat");

    let data: Vec<u8> = (0u8..=255).collect();
    write_bytes(&file, &data).unwrap();
    assert_eq!(read_byte(&file).unwrap(), data);

    // 覆盖写并截断
    write_bytes(&file, b"short").unwrap();
    assert_eq!(read_byte(&file).unwrap(), b"short".to_vec());

    // 从流写入
    let stream_file = root.join("stream.dat");
    write_stream(&stream_file, Cursor::new(data.clone())).unwrap();
    assert_eq!(read_byte(&stream_file).unwrap(), data);

    let _ = fs::remove_dir_all(&root);
}

/// 异步版本：write_text_async / read_text_async / write_bytes_async
#[tokio::test]
async fn test_async_text_bytes() {
    let root = make_test_root("async");
    let file = root.join("a.txt");

    write_text_async(file.clone(), "hello".to_string())
        .await
        .unwrap();
    assert_eq!(read_text_async(&file).await.unwrap(), "hello");
    assert_eq!(read_byte(&file).unwrap(), b"hello".to_vec());

    write_bytes_async(file.clone(), b"xyz").await.unwrap();
    assert_eq!(read_text(&file).unwrap(), "xyz");

    let _ = fs::remove_dir_all(&root);
}

/// open_append 追加写
#[test]
fn test_open_append() {
    let root = make_test_root("append");
    let file = root.join("log.txt");

    write_text(&file, "one").unwrap();
    {
        let mut stream = open_append(&file).unwrap();
        stream.write_all(b"two").unwrap();
    }
    assert_eq!(read_text(&file).unwrap(), "onetwo");

    // 对不存在文件 open_append 会创建
    let new_file = root.join("new.log");
    open_append(&new_file).unwrap();
    assert!(new_file.exists());

    let _ = fs::remove_dir_all(&root);
}

/// open_read_write 打开已存在文件且不截断；不存在时出错
#[test]
fn test_open_read_write() {
    let root = make_test_root("rw");
    let file = root.join("a.txt");
    write_text(&file, "content").unwrap();

    let mut stream = open_read_write(&file).unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, "content");

    assert!(open_read_write(root.join("no_such.txt")).is_err());

    let _ = fs::remove_dir_all(&root);
}

/// hard_link 共用同一份文件内容
#[test]
fn test_hard_link() {
    let root = make_test_root("hardlink");
    let src = root.join("src.txt");
    write_text(&src, "data").unwrap();

    let link = root.join("link.txt");
    hard_link(&src, &link).unwrap();
    assert_eq!(read_text(&link).unwrap(), "data");
    assert_eq!(file_id(&src), file_id(&link));
    // 两个路径指向同一份数据，拷贝出来的文件不会是 2
    assert_eq!(link_count(&src), Some(2));
    assert_eq!(link_count(&link), Some(2));

    // 删掉原文件后链接仍能读到内容，此时只剩一个链接
    delete(&src).unwrap();
    assert_eq!(read_text(&link).unwrap(), "data");
    assert_eq!(link_count(&link), Some(1));

    let _ = fs::remove_dir_all(&root);
}

/// copy_file / move_file / delete
#[test]
fn test_copy_move_delete() {
    let root = make_test_root("copymove");
    let src = root.join("src.txt");
    write_text(&src, "data").unwrap();

    // 复制
    let dst = root.join("dst.txt");
    copy_file(&src, &dst).unwrap();
    assert_eq!(read_text(&dst).unwrap(), "data");

    // 复制不存在的文件报错
    assert!(copy_file(root.join("no_such.txt"), dst.clone()).is_err());

    // 移动到新位置，move_file 会自动创建父目录
    let moved = root.join("sub").join("dir").join("moved.txt");
    move_file(&dst, &moved).unwrap();
    assert!(!dst.exists());
    assert_eq!(read_text(&moved).unwrap(), "data");

    // 删除文件
    delete(&src).unwrap();
    assert!(!src.exists());
    // delete 只删文件，目录保持原样
    delete(root.join("sub")).unwrap();
    assert!(root.join("sub").is_dir());

    let _ = fs::remove_dir_all(&root);
}

/// copy_dir 递归复制目录
#[test]
fn test_copy_dir() {
    let root = make_test_root("copydir");
    let src = root.join("src");
    fs::create_dir_all(src.join("nested")).unwrap();
    write_text(src.join("a.txt"), "a").unwrap();
    write_text(src.join("nested").join("b.txt"), "b").unwrap();

    let dst = root.join("dst");
    copy_dir(&src, &dst).unwrap();
    assert_eq!(read_text(dst.join("a.txt")).unwrap(), "a");
    assert_eq!(read_text(dst.join("nested").join("b.txt")).unwrap(), "b");

    let _ = fs::remove_dir_all(&root);
}

/// create_dir_all
#[test]
fn test_create_dir_all() {
    let root = make_test_root("mkdirs");
    let deep = root.join("a").join("b").join("c");
    create_dir_all(&deep).unwrap();
    assert!(deep.is_dir());
    // 重复创建不报错
    create_dir_all(&deep).unwrap();

    let _ = fs::remove_dir_all(&root);
}

/// get_all_files / get_files / get_dirs / get_folder_size / get_last_written_file
#[test]
fn test_list_and_size() {
    let root = make_test_root("list");
    write_text(root.join("a.txt"), "12345").unwrap(); // 5 字节
    fs::create_dir_all(root.join("sub")).unwrap();
    write_text(root.join("sub").join("b.txt"), "1234567890").unwrap(); // 10 字节

    // 递归列出所有文件
    let mut all = get_all_files(&root);
    all.sort();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&root.join("a.txt")));
    assert!(all.contains(&root.join("sub").join("b.txt")));

    // 仅当前目录文件
    let files = get_files(&root);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], root.join("a.txt"));

    // 仅当前目录子目录
    let dirs = get_dirs(&root);
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0], root.join("sub"));

    // 目录大小
    assert_eq!(get_folder_size(&root), 15);

    // 最后写入的文件：当前目录只有 a.txt 一个文件
    let last = get_last_written_file(&root).unwrap().unwrap();
    assert_eq!(last, root.join("a.txt"));

    // 不存在的目录报错
    assert!(get_last_written_file(root.join("no_such_dir")).is_err());

    let _ = fs::remove_dir_all(&root);
}

/// get_last_written_file 取修改时间最新的文件
#[test]
fn test_get_last_written_file_order() {
    let root = make_test_root("last");
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    write_text(&first, "1").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(80));
    write_text(&second, "2").unwrap();

    assert_eq!(get_last_written_file(&root).unwrap().unwrap(), second);

    let _ = fs::remove_dir_all(&root);
}

/// search_file 递归查找
#[test]
fn test_search_file() {
    let root = make_test_root("search");
    fs::create_dir_all(root.join("deep").join("deeper")).unwrap();
    let target = root.join("deep").join("deeper").join("target.txt");
    write_text(&target, "found").unwrap();
    write_text(root.join("other.txt"), "x").unwrap();

    assert_eq!(search_file(&root, "target.txt").unwrap(), target);
    assert_eq!(search_file(&root, "no_such.txt"), None);

    let _ = fs::remove_dir_all(&root);
}

/// file_has_invalid_chars 非法文件名检查
#[test]
fn test_file_has_invalid_chars() {
    // 空 / 全点号
    assert!(file_has_invalid_chars(""));
    assert!(file_has_invalid_chars("."));
    assert!(file_has_invalid_chars(".."));
    assert!(file_has_invalid_chars("..."));
    // 正常名字
    assert!(!file_has_invalid_chars("abc.txt"));
    assert!(!file_has_invalid_chars("中文名字"));
    // 非法字符
    for ch in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
        let name = format!("a{ch}b");
        assert!(file_has_invalid_chars(&name), "字符 {ch} 应判定非法");
    }
    // 超过 80 字节
    let long = "a".repeat(81);
    assert!(file_has_invalid_chars(&long));
    let ok_len = "a".repeat(80);
    assert!(!file_has_invalid_chars(&ok_len));
}

/// replace_file_name 替换非法字符
#[test]
fn test_replace_file_name() {
    assert_eq!(replace_file_name("a<b>c"), "a_b_c");
    assert_eq!(replace_file_name("plain.txt"), "plain.txt");
    assert_eq!(replace_file_name("a\\b/c"), "a_b_c");
    assert_eq!(replace_file_name("a\0b"), "a_b");
    // 中文保持不变
    assert_eq!(replace_file_name("中文.txt"), "中文.txt");
}

/// replace_path_name 平台差异
#[test]
fn test_replace_path_name() {
    // \0 在所有平台都替换
    assert_eq!(replace_path_name("a\0b"), "a_b");
    #[cfg(windows)]
    {
        assert_eq!(replace_path_name("a<b>:c"), "a_b__c");
        // 正斜杠在 Windows 上也替换
        assert_eq!(replace_path_name("a/b"), "a_b");
    }
    #[cfg(not(windows))]
    {
        // 非 Windows 平台仅替换 \0，其余符号保留
        assert_eq!(replace_path_name("a<b>:c/d"), "a<b>:c/d");
    }
}

/// 异步删除文件
#[tokio::test]
async fn test_delete_async() {
    let root = make_test_root("delasync");
    let file = root.join("a.txt");
    write_text(&file, "x").unwrap();
    delete_async(&file).await.unwrap();
    assert!(!file.exists());
    // 对目录 delete_async 不做任何事
    delete_async(&root).await.unwrap();
    assert!(root.is_dir());

    let _ = fs::remove_dir_all(&root);
}
