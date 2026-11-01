use super::*;

/// 返回值只算 stdout；stderr 只进日志
#[tokio::test]
async fn returns_stdout_only() {
    let output = run(
        "node",
        &[
            "-e".to_string(),
            "console.log('out'); console.error('err')".to_string(),
        ],
        RunOptions {
            cwd: None,
            log: no_log(),
            cancel: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(output, "out");
}

