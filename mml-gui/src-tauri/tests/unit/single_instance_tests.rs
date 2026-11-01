use std::path::PathBuf;

use super::*;
use mml_names::names;

/// 每个用例各用一个临时目录，见 AGENTS.md §6.1
///
/// - `tag`: 用例特征名，出问题时方便在 target/temp 下认出来
///
/// # 返回值
///
/// 已建好的空目录
fn port_dir(tag: &str) -> PathBuf {
    let dir = mml_testutil::temp_path(format!("mml_ipc_port_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 端口文件是十进制文本，脏内容按「没有端口」处理
#[test]
fn read_port_parses_decimal_text() {
    let dir = port_dir("parse");
    let file = dir.join(names::IPC_PORT_FILE);

    std::fs::write(&file, "12345").unwrap();
    assert_eq!(read_port(&file), Some(12345));

    std::fs::write(&file, "not a port").unwrap();
    assert_eq!(read_port(&file), None);

    std::fs::write(&file, " 54321 ").unwrap();
    assert_eq!(read_port(&file), Some(54321));

    std::fs::remove_dir_all(&dir).unwrap();
}

/// 占住端口文件之后：写进去的是十进制文本、清掉了旧内容，**端口仍然读得到**，
/// 而且别的进程再占会失败 —— 第二个实例就是靠这三条把请求交给已运行实例的。
///
/// 用同一个进程里的两个句柄验证：Windows 的共享冲突与 Unix 的 flock 都按句柄算，
/// 与真开第二个进程走的是同一套判定。
#[test]
fn held_port_file_is_writable_by_owner_readable_by_others_and_not_lockable() {
    let dir = port_dir("lock");
    let file = dir.join(names::IPC_PORT_FILE);

    // 文件不存在时也能占住：首次启动就是这种情况
    drop(lock_port_file(&dir.join("first_run")).expect("不存在时应当能创建并占住"));

    // 先放一个旧端口，验证写入会清掉它而不是拼在后面
    std::fs::write(&file, "99999").unwrap();

    // 第一个实例：占住文件，再按启动流程写端口
    let held = lock_port_file(&file).expect("空文件应当能占住");
    keep_port_file(held);
    write_port(45678);

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "45678",
        "端口要写成十进制文本，且盖掉旧内容"
    );
    assert_eq!(
        read_port(&file),
        Some(45678),
        "占着的时候别的进程也必须读得到端口"
    );

    // 第二个实例：占不住，只能走通知那条路
    assert!(
        lock_port_file(&file).is_err(),
        "已被占住时不该再占得上，否则会起第二个实例"
    );

    // 第一个实例退出（句柄丢掉）：文件还是那个端口，新实例能占住并覆盖它
    release_port_file();
    let again = lock_port_file(&file).expect("让出之后应当能重新占住");
    assert_eq!(read_port(&file), Some(45678), "旧端口要留到新实例写自己的");
    drop(again);

    std::fs::remove_dir_all(&dir).unwrap();
}

