use super::*;

/// src_dir 不存在要报错，而不是静默生成空产物
#[test]
fn test_generate_rejects_missing_src_dir() {
    let cfg = GenConfig {
        src_dir: PathBuf::from("H:/Temp/__definitely_not_here__"),
        ..Default::default()
    };
    assert!(matches!(generate(&cfg), Err(GenError::SrcDirMissing(_))));
}

/// 内容没变不写，返回 false；变了才写
#[test]
fn test_write_if_changed_skips_identical() {
    let dir = mml_testutil::temp_dir().join("gui_ipc_gen_wic");
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("a/b.ts");
    assert!(write_if_changed(&path, "x").unwrap(), "首次应写入");
    assert!(!write_if_changed(&path, "x").unwrap(), "内容相同不该写");
    assert!(write_if_changed(&path, "y").unwrap(), "内容变了应写入");
    assert_eq!(fs::read_to_string(&path).unwrap(), "y");
    let _ = fs::remove_dir_all(&dir);
}

/// 生成文件固定写 CRLF：LF 旧文件改写一次，之后同样内容不重写
#[test]
fn test_write_if_changed_writes_crlf() {
    let dir = mml_testutil::temp_dir().join("gui_ipc_gen_crlf");
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("a/b.ts");

    // 磁盘上是 LF：要改写成 CRLF
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "a\nb\n").unwrap();
    assert!(
        write_if_changed(&path, "a\nb\n").unwrap(),
        "LF 旧文件应改写"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "a\r\nb\r\n");

    // 已经是 CRLF：同样内容不再写
    assert!(
        !write_if_changed(&path, "a\nb\n").unwrap(),
        "内容相同不该写"
    );

    // 输入本身已经是 CRLF 时也不能叠成 \r\r\n
    assert!(!write_if_changed(&path, "a\r\nb\r\n").unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "a\r\nb\r\n");

    let _ = fs::remove_dir_all(&dir);
}

/// 已经是 CRLF 的文本原样借用，不重复转换
#[test]
fn test_to_crlf_keeps_existing_crlf() {
    let text = "a\r\nb\r\n";
    assert!(matches!(to_crlf(text), Cow::Borrowed(_)));
    assert_eq!(to_crlf("a\nb\n").as_ref(), "a\r\nb\r\n");
}

