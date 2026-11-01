use regex::Regex;
use std::{path::Path, sync::LazyLock};

/// 匹配含非数字字符的正则
static REGEX_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^0-9]+").unwrap());
/// 匹配纯英文数字的正则
static REGEX_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9]+$").unwrap());

/// 检查输入是否包含非数字字符
///
/// - `input`: 需要检查的内容
///
/// # 返回值
///
/// 空串与纯空白串视为含非数字字符
pub fn check_is_not_number(input: &str) -> bool {
    if input.trim().is_empty() {
        return true;
    }
    REGEX_NUMBER.is_match(input)
}

/// 检查是否为英文数字
///
/// - `input`: 需要检查的内容
///
/// # 返回值
///
/// 仅由 ASCII 字母与数字组成时为 `true`，空串为 `false`
pub fn check_is_word(input: &str) -> bool {
    REGEX_WORD.is_match(input)
}

/// 截取起始标记与结束标记之间的内容
///
/// - `input`: 输入内容
/// - `start`: 起始标记
/// - `end`: 结束标记
///
/// # 返回值
///
/// 返回起始标记后一个字符到结束标记之间的内容；任一标记缺失时原样返回 `input`
pub fn get_string(input: &str, start: &str, end: &str) -> String {
    if let Some(start_byte) = input.find(start) {
        let start_end_byte = start_byte + start.len();

        let tail = &input[start_end_byte..];
        let skip_bytes = match tail.chars().next() {
            Some(ch) => ch.len_utf8(),
            None => return input.to_string(),
        };
        let search_start_byte = start_end_byte + skip_bytes;

        if let Some(rel_end_byte) = input[search_start_byte..].find(end) {
            let end_byte = search_start_byte + rel_end_byte;
            return input[start_end_byte..end_byte].to_string();
        }
    }
    input.to_string()
}

/// 将字符串列表转换为以换行符分隔的单个字符串
///
/// 每个原始字符串后附加平台对应的换行符，Windows 为 `\r\n`，其余平台为 `\n`
///
/// - `vec`: 输入数据
///
/// # 返回值
///
/// 列表为空时返回空串
pub fn build_vec_string(vec: &Vec<String>) -> String {
    let mut str = String::new();

    for item in vec.iter() {
        str.push_str(item);
        str.push_str(&mml_names::get_line_ending());
    }

    str
}

/// 拆分后的路径两段
pub struct PathPart {
    /// 父目录路径，没有上级目录时为空串
    pub parent: String,
    /// 文件名，即路径的最后一段
    pub file: String,
}

/// 将输入路径拆分为父目录与文件名两段
///
/// 例如 `home/user/text.txt` 拆成 `home/user` 与 `text.txt`
///
/// - `input`: 输入路径
///
/// # 返回值
///
/// 没有上级目录时返回的 `parent` 为空串
pub fn get_path_part(input: &str) -> PathPart {
    let path = Path::new(input);

    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "".to_string());

    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| "".to_string());

    PathPart { parent, file }
}

/// 按空白切分命令行字符串，双引号内的空白保留
///
/// - `input`: 命令行字符串
///
/// # 返回值
///
/// 返回切分后的参数列表；空串与纯空白串返回空列表
pub fn arg_parse(input: &str) -> Vec<String> {
    let quote_char = '"';
    let escape_char = '\\';
    let mut inside_quote = false;
    let mut inside_escape = false;

    let mut current_arg = String::new();
    let mut current_arg_char_count = 0;
    let mut result = Vec::new();

    for c in input.chars() {
        if c == quote_char {
            current_arg_char_count += 1;

            if inside_escape {
                current_arg.push(c);
                inside_escape = false;
            } else if inside_quote {
                inside_quote = false;
            } else {
                inside_quote = true;
            }
        } else if c == escape_char {
            current_arg_char_count += 1;

            if inside_escape {
                current_arg.push_str(&format!("{}{}", escape_char, escape_char));
            }

            inside_escape = !inside_escape;
        } else if c.is_whitespace() {
            if inside_quote {
                current_arg_char_count += 1;
                current_arg.push(c);
            } else {
                if current_arg_char_count > 0 {
                    result.push(current_arg.clone());
                }
                current_arg_char_count = 0;
                current_arg.clear();
            }
        } else {
            current_arg_char_count += 1;
            if inside_escape {
                current_arg.push(escape_char);
                current_arg_char_count = 0;
                inside_escape = false;
            }
            current_arg.push(c);
        }
    }

    if current_arg_char_count > 0 {
        result.push(current_arg);
    }

    result
}

#[cfg(test)]
#[path = "../tests/unit/tools_tests.rs"]
mod tests;
