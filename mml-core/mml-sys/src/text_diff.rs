//! 文本文件的行级差异
//!
//! 「配置编辑器」保存文本走的就是这一套：前端只把**改了哪几行**发回来（差异由 Monaco 的
//! LcsDiff 算，见 mml-vue 的 `lib/textDiff.ts`），这里把差异套到原文上，一次 IPC 不必搬整份文件。
//!
//! 两边必须按同一口径切行：都按 `\n` 切、按 `\n` 拼，行号一律相对**打开时读到的那一份**。

use std::path::Path;

use mml_names::i18_items::error_type::{CoreResult, ErrorType};

use crate::path_helper;

/// 一次行替换：把原文 `start` 起的 `delete_count` 行换成 `lines`
pub struct LineHunk {
    /// 原文里的起始行号，从 0 起
    pub start: usize,
    /// 要替换掉的行数；0 表示纯插入
    pub delete_count: usize,
    /// 替换成的新行；空表示纯删除
    pub lines: Vec<String>,
}

impl LineHunk {
    /// 造一次行替换
    ///
    /// - `start`: 原文里的起始行号，从 0 起
    /// - `delete_count`: 要替换掉的行数；0 表示纯插入
    /// - `lines`: 替换成的新行；空表示纯删除
    ///
    /// # 返回值
    ///
    /// 见 [`Self`]
    pub fn new(start: usize, delete_count: usize, lines: Vec<String>) -> Self {
        Self {
            start,
            delete_count,
            lines,
        }
    }
}

/// 把行级差异套到原文上，得到新的行列表
///
/// 行号都是相对**原文**的：倒着改，前面那些改动才不会被后面的影响；同时要求各段有序
/// 不重叠（同点插入允许），越界或交叉都说明差异与这一份对不上。
///
/// - `text`: 原文全文
/// - `hunks`: 行替换列表
///
/// # 返回值
///
/// 新的行列表；差异非法时返回 [`ErrorType::InvalidOperation`]
pub fn apply_hunks(text: &str, hunks: &[LineHunk]) -> CoreResult<Vec<String>> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let original_len = lines.len();

    // 下一个（更靠后的）改动在原文里的起点，当前这段不能越过它
    let mut next_start = original_len;
    for hunk in hunks.iter().rev() {
        let end = hunk
            .start
            .checked_add(hunk.delete_count)
            .ok_or(ErrorType::InvalidOperation)?;
        if end > original_len || end > next_start {
            return Err(ErrorType::InvalidOperation);
        }
        next_start = hunk.start;
        lines.splice(hunk.start..end, hunk.lines.iter().cloned());
    }

    Ok(lines)
}

/// 按行级差异改一份文本：套用差异，再核对改完的总行数
///
/// - `text`: 原文全文
/// - `hunks`: 行替换列表，行号相对 `text`
/// - `expected_lines`: 改完后的总行数，用来交叉校验差异算得对不对
///
/// # 返回值
///
/// 改完的全文；差异非法或行数与预期不符时返回 [`ErrorType::InvalidOperation`]
pub fn apply_diff(text: &str, hunks: &[LineHunk], expected_lines: usize) -> CoreResult<String> {
    let lines = apply_hunks(text, hunks)?;
    if lines.len() != expected_lines {
        return Err(ErrorType::InvalidOperation);
    }

    Ok(lines.join("\n"))
}

/// 把行级差异套到 `text` 上并写回 `path`
///
/// `text` 必须是**刚从这个文件读出来的那一份**：配置窗口按 SHA1 核对过磁盘上还是不是它算差异
/// 时那一份，这里只负责套差异与落盘，不再自己读一遍。差异套不上就带着错误返回，**不落盘**。
///
/// - `path`: 文件路径
/// - `text`: 文件当前内容
/// - `hunks`: 行替换列表，行号相对 `text`
/// - `expected_lines`: 改完后的总行数，用来交叉校验差异算得对不对
///
/// # 返回值
///
/// 写下去的全文；差异对不上或行数不符返回 [`ErrorType::InvalidOperation`]，
/// 写盘失败返回文件系统错误
pub fn write_diff(
    path: &Path,
    text: &str,
    hunks: &[LineHunk],
    expected_lines: usize,
) -> CoreResult<String> {
    let new_text = apply_diff(text, hunks, expected_lines)?;
    path_helper::write_text(path, &new_text)?;

    Ok(new_text)
}

#[cfg(test)]
mod tests {
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
}
