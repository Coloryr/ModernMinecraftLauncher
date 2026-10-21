//! mml-tex-draw 集成测试
//!
//! 验证公开 API `init` 的行为：
//! - 在指定根路径下创建方块数据目录，目录名来自 `mml_names::names::BLOCK_DIR`；
//! - 根路径不存在时能连同父目录一并创建；
//! - 重复调用不报错、不 panic。
//!
//! 注意：`init` 内部使用进程级 `OnceLock` 保存首次调用的路径，后续调用不会再
//! 创建新路径下的目录。同一测试二进制内的测试默认并行执行，因此所有依赖
//! "首次初始化"的断言必须集中在单条测试中顺序执行，避免顺序竞争。

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use mml_names::names;
use mml_tex_draw::init;

/// 生成测试用的唯一临时目录路径，不自动创建
///
/// - `tag`: 目录名里的用例标识
///
/// # 返回值
///
/// 返回 `mml_testutil` 临时目录下带进程 ID 与纳秒时间戳的路径
fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    mml_testutil::temp_dir().join(format!(
        "mml-tex-draw-it-{}-{}-{}",
        tag,
        std::process::id(),
        nanos
    ))
}

/// init 的完整行为验证：单条测试顺序执行，规避 OnceLock 的首次初始化竞争
#[test]
fn init_behavior_serialized() {
    // 场景 1：根目录含多级父目录且不存在时，init 应递归创建并建出 block 目录
    let nested_root = unique_temp_dir("nested").join("a").join("b");
    let result = init(&nested_root);
    assert!(result.is_ok(), "init 应返回 Ok：{:?}", result.err());
    assert!(nested_root.exists());
    assert!(
        nested_root.join(names::BLOCK_DIR).exists(),
        "init 后应创建 {} 目录",
        nested_root.join(names::BLOCK_DIR).display()
    );

    // 清理场景 1
    let _ = fs::remove_dir_all(nested_root);

    // 场景 2：重复调用不报错。BLOCK_DIR 已被首次调用固定到场景 1 的路径，
    // 因此这里只断言返回 Ok，不断言新路径下目录被创建，这是 OnceLock 语义
    let another_root = unique_temp_dir("another");
    fs::create_dir_all(&another_root).unwrap();
    assert!(init(&another_root).is_ok());
    let _ = fs::remove_dir_all(&another_root);

    // 场景 3：对普通文件路径调用 init 不应 panic，目录已固定，创建逻辑被跳过
    let file_root = unique_temp_dir("file");
    fs::create_dir_all(&file_root).unwrap();
    let file_path = file_root.join("not_a_dir");
    fs::write(&file_path, b"placeholder").unwrap();
    let _ = init(&file_path);
    let _ = fs::remove_dir_all(&file_root);
}

/// names 模块提供的常量应为相对路径名称，且能跨平台拼接
#[test]
fn names_are_relative() {
    assert_eq!(names::BLOCK_DIR, "block");
    assert_eq!(names::BLOCK_FILE, "block.json");
    assert!(!Path::new(names::BLOCK_DIR).is_absolute());
    assert!(!Path::new(names::BLOCK_FILE).is_absolute());

    let path = Path::new("data").join(names::BLOCK_FILE);
    assert!(path.ends_with("block.json"));
}
