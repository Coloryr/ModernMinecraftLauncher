use std::{fs, path::Path};

use super::*;

/// init 应在指定路径下创建 block 数据目录，且目录名来自 names::BLOCK_DIR
///
/// 注意：BLOCK_FILE / BLOCK_DIR 为进程级 OnceLock，首次调用后不再变化，
/// 因此本二进制内的初始化断言集中在这一条测试中顺序执行。
#[test]
fn init_creates_block_dir() {
    let root = unique_temp_dir("unit");
    fs::create_dir_all(&root).unwrap();

    let result = init(&root);
    assert!(result.is_ok());

    let dir = root.join(names::BLOCK_DIR);
    assert!(dir.exists(), "init 后应创建 block 目录：{}", dir.display());

    // 重复调用不应报错，OnceLock 保持首次的路径
    assert!(init(&root).is_ok());

    // 清理临时目录
    let _ = fs::remove_dir_all(&root);
}

/// names 模块提供的常量应为预期的相对名称
#[test]
fn names_constants() {
    assert_eq!(names::BLOCK_DIR, "block");
    assert_eq!(names::BLOCK_FILE, "block.json");

    // 路径拼接不依赖平台分隔符写法
    let path = Path::new("data").join(names::BLOCK_FILE);
    assert!(path.ends_with("block.json"));
}

