//! `InputFile` 的本地文件写入测试，不联网。
//!
//! 验证 `InputFile::Data`、`InputFile::Stream`、`InputFile::Path` 三种来源
//! 经 `save_file()` 落盘后的内容一致性。全部使用仓库 `target/temp` 下的
//! 临时目录，测完清理，不硬编码任何绝对路径。

use std::io::Cursor;

use mml_net::input_file::InputFile;

/// 为当前进程按用例特征名创建唯一的临时目录
fn make_temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = mml_testutil::temp_dir().join(format!(
        "mml-net-input-file-test-{}-{}",
        std::process::id(),
        tag
    ));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// `InputFile::Data`：内存字节直接写入目标文件
#[tokio::test]
async fn save_data_input() {
    let dir = make_temp_dir("data");
    let target = dir.join("out.bin");
    let payload = vec![1u8, 2, 3, 4, 250, 251, 252];

    InputFile::Data(payload.clone())
        .save_file(&target)
        .await
        .expect("Data 写入应成功");

    let written = std::fs::read(&target).expect("目标文件应存在");
    assert_eq!(written, payload);

    let _ = std::fs::remove_dir_all(&dir);
}

/// `InputFile::Stream`：同步读取流写入目标文件
#[tokio::test]
async fn save_stream_input() {
    let dir = make_temp_dir("stream");
    let target = dir.join("out.txt");
    let payload = b"hello mml stream";

    InputFile::Stream(Box::new(Cursor::new(payload.to_vec())))
        .save_file(&target)
        .await
        .expect("Stream 写入应成功");

    let written = std::fs::read(&target).expect("目标文件应存在");
    assert_eq!(written, payload);

    let _ = std::fs::remove_dir_all(&dir);
}

/// `InputFile::Path`：本地文件异步复制到目标路径
#[tokio::test]
async fn save_path_input() {
    let dir = make_temp_dir("path");
    let src = dir.join("src.txt");
    let target = dir.join("nested").join("dst.txt");
    std::fs::create_dir_all(target.parent().unwrap()).expect("创建目标目录失败");
    std::fs::write(&src, b"copy me").expect("创建源文件失败");

    InputFile::Path(src.clone())
        .save_file(&target)
        .await
        .expect("Path 复制应成功");

    let written = std::fs::read(&target).expect("目标文件应存在");
    assert_eq!(written, b"copy me");

    let _ = std::fs::remove_dir_all(&dir);
}
