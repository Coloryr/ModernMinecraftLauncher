//! 文本文件行级差异：拿**真文件**跑的大文本用例
//!
//! 用例自己生成几万行文本（含空行、缩进、中文、emoji 与 CRLF），落成**测试目录下的真文件**
//! 再走一遍真实流程：读出来 → 套差异写回去 → 从盘上读回来**逐字符**比对。
//!
//! 生成的样本就放在 `mml-core/mml-sys/tests/` 下，跑完能直接打开看：
//!
//! | 文件 | 内容 |
//! | --- | --- |
//! | `text_diff_big.txt` | 生成的原文，5 万行 |
//! | `text_diff_big.expected.txt` | 按那串编辑算出来的期望结果 |
//! | `text_diff_big.result.txt` | 实际被改的那一份（跑完应当与 `.expected.txt` 逐字节相同） |
//!
//! 三个都是每次跑用例时现生成的，已在 `.gitignore` 里排除，不入库。
//!
//! 为什么逐字符：几 MB 的文本上 `assert_eq!` 会把整份内容打进失败信息，等于什么也没说；
//! 走到第一处不同就报字符下标、行号与两边的上下文。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mml_sys::path_helper;
use mml_sys::text_diff::{LineHunk, write_diff};

/// 造一次行替换
fn hunk(start: usize, delete_count: usize, lines: &[&str]) -> LineHunk {
    LineHunk::new(
        start,
        delete_count,
        lines.iter().map(|line| line.to_string()).collect(),
    )
}

/// 造一份大文本的行：含空行、缩进、中文与 emoji
fn big_lines(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| match i % 7 {
            0 => String::new(),
            1 => format!("第 {i} 行：中文与 emoji 🐟 混排"),
            2 => format!("\t缩进 {i} = {}", i * 31),
            _ => format!("line {i} value={}", i * 7 % 1013),
        })
        .collect()
}

/// 测试目录下的样本路径：生成的文本落在这里，跑完直接能看到
fn sample_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(name)
}

/// 在 `target/temp` 下给这条用例开一个自己的目录（只有需要一份可写副本时用）
fn case_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        mml_testutil::temp_dir().join(format!("mml-text-diff-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 逐字符比对，第一处不一样就报出来
fn assert_same_text(got: &str, want: &str) {
    if got == want {
        return;
    }

    let mut index = 0;
    for (a, b) in got.chars().zip(want.chars()) {
        if a != b {
            break;
        }
        index += 1;
    }

    /// 取第 `index` 个字符所在的那一行
    fn line_at(text: &str, index: usize) -> String {
        let byte = text
            .char_indices()
            .nth(index)
            .map(|(byte, _)| byte)
            .unwrap_or(text.len());
        let start = text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let end = text[byte..].find('\n').map(|i| byte + i).unwrap_or(text.len());
        format!(
            "第 {} 行 {:?}",
            text[..start].matches('\n').count() + 1,
            &text[start..end]
        )
    }

    panic!(
        "第 {} 个字符起不一样（两边分别 {} / {} 个字符，{} / {} 行）\n  实际：{}\n  预期：{}",
        index,
        got.chars().count(),
        want.chars().count(),
        got.matches('\n').count() + 1,
        want.matches('\n').count() + 1,
        line_at(got, index),
        line_at(want, index),
    );
}

/// 把文本写进文件，再按同一份内容读出来 —— 与配置窗口保存时读盘那一份完全一致
fn write_and_read(path: &Path, text: &str) -> String {
    path_helper::write_text(path, text).unwrap();
    path_helper::read_text(path).unwrap()
}

/// 大文本 + 多处改动：头插、中替、中删、尾追加
///
/// 生成的那一份、期望的那一份、实际被改的那一份都落在 `tests/` 下（见文件头那张表），
/// 跑完可以直接打开对比；被改的就是测试里读进来、套完差异写回去的那个文件本身。
///
/// 期望文本由用例自己按同一串编辑拼出来（倒着改，行号才不会被后面的改动带偏），
/// 不借 `apply_hunks` 构造 —— 那样等于自己验自己。
#[test]
fn big_text_multi_edit_matches_char_by_char() {
    let original = big_lines(50_000);
    let mut want = original.clone();
    want.splice(50_000..50_000, ["# 尾追加".to_string()]);
    want.splice(30_000..30_020, []);
    want.splice(
        12_345..12_350,
        ["# 中部替换 A".to_string(), "# 中部替换 B".to_string()],
    );
    want.splice(0..0, ["# 头插一行".to_string()]);
    let original_text = original.join("\n");
    let want_text = want.join("\n");

    // 先生成：原文、期望结果，以及待会儿真被改的那一份
    let input = sample_path("text_diff_big.txt");
    let expected = sample_path("text_diff_big.expected.txt");
    let result = sample_path("text_diff_big.result.txt");
    path_helper::write_text(&input, &original_text).unwrap();
    path_helper::write_text(&expected, &want_text).unwrap();
    path_helper::write_text(&result, &original_text).unwrap();

    // 再测：从盘上读回待改的那一份当"打开时的内容"，套差异写回，再读回来逐字符比对
    let text = path_helper::read_text(&result).unwrap();
    assert_same_text(&text, &original_text);

    let hunks = [
        hunk(0, 0, &["# 头插一行"]),
        hunk(12_345, 5, &["# 中部替换 A", "# 中部替换 B"]),
        hunk(30_000, 20, &[]),
        hunk(50_000, 0, &["# 尾追加"]),
    ];

    let written = write_diff(&result, &text, &hunks, want.len()).unwrap();
    assert_same_text(&written, &want_text);
    // 盘上那份也要逐字符对上：上面比的是返回值，这里比真的落盘结果，并与期望样本一致
    assert_same_text(&path_helper::read_text(&result).unwrap(), &want_text);
    assert_same_text(&path_helper::read_text(&expected).unwrap(), &want_text);
}

/// 大文本：整份替换（一段盖住全文）
#[test]
fn big_text_whole_replace() {
    let dir = case_dir("whole");
    let path = dir.join("options.txt");

    let original = big_lines(20_000);
    let text = write_and_read(&path, &original.join("\n"));
    let replaced: Vec<String> = original.iter().map(|line| format!("# {line}")).collect();
    let want_text = replaced.join("\n");

    let hunks = [LineHunk::new(0, original.len(), replaced)];
    let written = write_diff(&path, &text, &hunks, want_text.split('\n').count()).unwrap();
    assert_same_text(&written, &want_text);
    assert_same_text(&path_helper::read_text(&path).unwrap(), &want_text);

    let _ = fs::remove_dir_all(&dir);
}

/// 大文本：两百处小改动，改动点之间隔得远
#[test]
fn big_text_many_small_changes() {
    let dir = case_dir("many");
    let path = dir.join("kubejs.js");

    let original = big_lines(20_000);
    let text = write_and_read(&path, &original.join("\n"));

    // 每 100 行改一行，从前往后记 hunk；期望值倒着改，行号才不会被前一处带偏
    let starts: Vec<usize> = (0..200).map(|i| i * 100).collect();
    let hunks: Vec<LineHunk> = starts
        .iter()
        .map(|i| {
            let line = format!("# 改过的第 {i} 行");
            hunk(*i, 1, &[line.as_str()])
        })
        .collect();

    let mut want = original.clone();
    for start in starts.iter().rev() {
        want.splice(*start..*start + 1, [format!("# 改过的第 {start} 行")]);
    }
    let want_text = want.join("\n");

    let written = write_diff(&path, &text, &hunks, want.len()).unwrap();
    assert_same_text(&written, &want_text);
    assert_same_text(&path_helper::read_text(&path).unwrap(), &want_text);

    let _ = fs::remove_dir_all(&dir);
}

/// 超长单行（1 MB）替换：前后的行都不该被碰到
#[test]
fn huge_single_line_replace() {
    let dir = case_dir("longline");
    let path = dir.join("data.json");

    let original = format!("头\n{}\n尾", "x".repeat(1024 * 1024));
    let text = write_and_read(&path, &original);

    let replacement = format!("y{}", "z".repeat(1024 * 1024));
    let want_text = format!("头\n{replacement}\n尾");
    let hunks = [LineHunk::new(1, 1, vec![replacement])];

    let written = write_diff(&path, &text, &hunks, 3).unwrap();
    assert_same_text(&written, &want_text);
    assert_same_text(&path_helper::read_text(&path).unwrap(), &want_text);

    let _ = fs::remove_dir_all(&dir);
}

/// CRLF 与空行混排：切行只认 `\n`，`\r` 是行内容的一部分，改完不该被吞掉
#[test]
fn crlf_and_blank_lines_survive() {
    let dir = case_dir("crlf");
    let path = dir.join("old.properties");

    let original: Vec<String> = (0..5_000)
        .map(|i| if i % 5 == 0 { String::new() } else { format!("行 {i}\r") })
        .collect();
    let text = write_and_read(&path, &original.join("\n"));

    let mut want = original.clone();
    want.splice(
        1_000..1_003,
        ["改过的\r".to_string(), String::new(), "又一行\r".to_string()],
    );
    let want_text = want.join("\n");
    let hunks = [hunk(1_000, 3, &["改过的\r", "", "又一行\r"])];

    let written = write_diff(&path, &text, &hunks, want.len()).unwrap();
    assert_same_text(&written, &want_text);
    // `\r` 真的还在：逐字节比一遍
    assert_eq!(fs::read(&path).unwrap(), want_text.as_bytes());

    let _ = fs::remove_dir_all(&dir);
}

/// 差异对不上时**不落盘**：文件必须原样躺着，一个字都不能动
#[test]
fn rejected_diff_leaves_file_untouched() {
    let dir = case_dir("reject");
    let path = dir.join("config.toml");

    let text = write_and_read(&path, &big_lines(5_000).join("\n"));
    let before = fs::read(&path).unwrap();
    let line_count = text.split('\n').count();

    // 下标越界
    assert!(write_diff(&path, &text, &[hunk(99_999, 1, &["x"])], line_count).is_err());
    // 各段交叉
    assert!(
        write_diff(
            &path,
            &text,
            &[hunk(0, 3, &[]), hunk(1, 1, &["y"])],
            line_count - 2
        )
        .is_err()
    );
    // 行数与预期不符
    assert!(write_diff(&path, &text, &[hunk(0, 1, &["z"])], line_count + 5).is_err());

    assert_eq!(fs::read(&path).unwrap(), before);

    let _ = fs::remove_dir_all(&dir);
}
