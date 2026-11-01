use super::{LineHunk, apply_diff, apply_hunks};

/// 造一次行替换
fn hunk(start: usize, delete_count: usize, lines: &[&str]) -> LineHunk {
    LineHunk::new(
        start,
        delete_count,
        lines.iter().map(|line| line.to_string()).collect(),
    )
}

/// 行替换按原文行号套回去；多段有序不重叠也要对
#[test]
fn apply_hunks_splices_lines() {
    // 改中间一行
    let one = [hunk(1, 1, &["x", "y"])];
    assert_eq!(
        apply_hunks("a\nb\nc", &one).unwrap().join("\n"),
        "a\nx\ny\nc"
    );

    // 两段：改第一行、删最后一行
    let two = [hunk(0, 1, &["A"]), hunk(2, 1, &[])];
    assert_eq!(apply_hunks("a\nb\nc", &two).unwrap().join("\n"), "A\nb");

    // 纯插入
    let insert = [hunk(1, 0, &["mid"])];
    assert_eq!(
        apply_hunks("a\nb", &insert).unwrap().join("\n"),
        "a\nmid\nb"
    );

    // 末尾追加（行列表按 `\n` 切，末行是空串）
    let append = [hunk(2, 0, &["c"])];
    assert_eq!(
        apply_hunks("a\nb\n", &append).unwrap().join("\n"),
        "a\nb\nc\n"
    );
}

/// 越界与交叉的差异要拒掉：宁可报错，也不能按错的差异改文件
#[test]
fn apply_hunks_rejects_bad_ranges() {
    let out_of_range = [hunk(3, 1, &[])];
    assert!(apply_hunks("a\nb", &out_of_range).is_err());

    let crossing = [hunk(0, 2, &[]), hunk(1, 1, &[])];
    assert!(apply_hunks("a\nb\nc", &crossing).is_err());
}

/// 套完的全文与预期行数都要对，缺一个就当差异与这一份对不上
#[test]
fn apply_diff_checks_line_count() {
    let hunks = [hunk(1, 1, &["x"])];
    assert_eq!(apply_diff("a\nb\nc", &hunks, 3).unwrap(), "a\nx\nc");

    // 实际改完是 3 行，说成 4 行就不认
    assert!(apply_diff("a\nb\nc", &hunks, 4).is_err());
    // 越界同样不认
    assert!(apply_diff("a\nb\nc", &[hunk(9, 1, &[])], 3).is_err());
}

