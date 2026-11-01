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
#[path = "../tests/unit/text_diff_tests.rs"]
mod tests;
