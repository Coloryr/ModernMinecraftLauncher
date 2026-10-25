use std::sync::OnceLock;

use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorData, ErrorType};
use serde::{Deserialize, Serialize};

use crate::html_scan::attr;
use crate::urls;

/// 任务出错转 TaskError
fn task_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::TaskError(ErrorData::from_error(err))
}

/// Semeru 下载页接口返回的单条结果
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ResultsObj {
    /// 页面 HTML 正文，用于解析主版本 / 系统 / 架构下拉框
    pub content: String,
    /// 页面内嵌的 JS 脚本，含全部下载条目 `sourceDataJson`
    pub pagepost_custom_js_value: String,
}

/// Semeru 下载页接口的响应
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct OpenJ9Obj {
    /// 接口是否报错，为 true 时 `results` 不可用
    pub error: bool,
    /// 页面结果列表，代码只取第一条
    pub results: Vec<ResultsObj>,
}

/// 单个下载选项
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct OptObj {
    /// 下载直链
    #[serde(rename = "downloadLink")]
    pub download_link: String,
    /// 文件校验值，算法由页面给定
    pub checksum: String,
}

/// 某一种包类型下的下载选项集合，字段名沿用页面里的 `opt1`
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct JdkObj {
    /// 页面中该包类型只给出 opt1 一个选项
    pub opt1: OptObj,
}

/// 单条 Java 下载条目
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DownloadObj {
    /// 条目名称，页面原文以 `<br>` 分三段，初始化时改写成 `{第二段}+{第三段}`
    pub name: String,
    /// Java 主版本号
    pub version: i32,
    /// 目标操作系统
    pub os: String,
    /// 目标架构
    pub arch: String,
    /// JDK 包下载信息
    pub jdk: JdkObj,
    /// JRE 包下载信息
    pub jre: JdkObj,
}

/// 页面 JS 里 `sourceDataJson` 的结构
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OpenJ9FileObj {
    /// 全部下载条目
    pub downloads: Vec<DownloadObj>,
}

/// 解析后的 IBM Semeru 可下载列表
#[derive(Debug, Clone)]
pub struct OpenJ9ListObj {
    /// 架构候选值，已剔除页面里的 any
    pub arch: Vec<String>,
    /// 操作系统候选值，已剔除页面里的 any
    pub os: Vec<String>,
    /// Java 主版本候选值
    pub main_version: Vec<String>,
    /// 全部下载条目
    pub download: Vec<DownloadObj>,
}

/// 页面下拉框里的一个选项
struct SelectOption {
    /// `<option>` 的 value 属性，没有就是空串
    value: String,
    /// `<option>` 的 class 属性，没有就是空串
    class: String,
}

/// 取指定 id 的 `<select>` 下的全部 `<option>`
///
/// - `html`: 页面原文
/// - `id`: select 的 id
///
/// # 返回值
///
/// 找不到该 select、或标签没闭合时返回 `None`
fn select_options(html: &str, id: &str) -> Option<Vec<SelectOption>> {
    let rest = &html[find_select(html, id)?..];
    let end = rest.find("</select").unwrap_or(rest.len());
    let body = &rest[..end];

    let mut list = Vec::new();
    let mut offset = 0;
    while let Some(pos) = body[offset..].find("<option") {
        let begin = offset + pos;
        let Some(len) = body[begin..].find('>') else {
            break;
        };
        let attrs = &body[begin..begin + len];
        list.push(SelectOption {
            value: attr(attrs, "value").unwrap_or_default(),
            class: attr(attrs, "class").unwrap_or_default(),
        });
        offset = begin + len + 1;
    }
    Some(list)
}

/// 找指定 id 的 `<select>` 开标签，返回标签之后的下标
///
/// - `html`: 页面原文
/// - `id`: select 的 id
fn find_select(html: &str, id: &str) -> Option<usize> {
    let mut offset = 0;
    while let Some(pos) = html[offset..].find("<select") {
        let begin = offset + pos;
        let len = html[begin..].find('>')?;
        if attr(&html[begin..begin + len], "id").as_deref() == Some(id) {
            return Some(begin + len + 1);
        }
        offset = begin + len + 1;
    }
    None
}

/// 解析结果缓存，首次查询后复用
static OPENJ9: OnceLock<OpenJ9ListObj> = OnceLock::new();

/// 抓取并解析 Semeru 下载页，结果写入 `OPENJ9` 缓存
async fn init() -> CoreResult<OpenJ9ListObj> {
    let data: OpenJ9Obj = crate::get_work_client().get_json(urls::OPENJ9).await?;
    let data = &data.results[0];

    let html = &data.content;

    let mut obj = OpenJ9ListObj {
        arch: Default::default(),
        os: Default::default(),
        main_version: Default::default(),
        download: Default::default(),
    };

    let Some(versions) = select_options(html, "runtimeVersion") else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    for item in versions {
        // 主版本列表里还混着占位项，只认带该 class 的
        if item.class.contains("bx--select-option") {
            obj.main_version.push(item.value);
        }
    }

    let Some(os_list) = select_options(html, "operatingSystem") else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    for item in os_list {
        if !item.value.eq_ignore_ascii_case("any") {
            obj.os.push(item.value);
        }
    }

    let Some(arch_list) = select_options(html, "arch") else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };
    for item in arch_list {
        if !item.value.eq_ignore_ascii_case("any") {
            obj.arch.push(item.value);
        }
    }

    // 脚本里 `function` 之后是页面渲染逻辑，截掉后只剩 `let sourceDataJson = {...}`
    let script = match data.pagepost_custom_js_value.find("function") {
        Some(pos) => &data.pagepost_custom_js_value[..pos],
        None => data.pagepost_custom_js_value.as_str(),
    };
    let Some(json) = js_object_to_json(script) else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };

    // 页面数据里有重复键，JS 取最后一个；serde 的强类型反序列化会报错，先过一遍 Value
    let value: serde_json::Value = serde_json::from_str(&json).map_err(task_err)?;
    let files: OpenJ9FileObj = serde_json::from_value(value).map_err(task_err)?;

    for mut item in files.downloads {
        let names: Vec<&str> = item.name.split("<br>").collect();
        item.name = if names.len() == 3 {
            format!("{}+{}", names[1], names[2])
        } else {
            item.name
        };
        obj.download.push(item);
    }

    Ok(OPENJ9.get_or_init(|| obj).clone())
}

/// 把页面里的 JS 对象字面量转成 JSON 文本
///
/// 页面给的是 `let sourceDataJson = {...}`：字符串一律单引号、键不加引号，标准 JSON 解析器读不了。
/// 这里只处理这两种差异与尾随逗号，其余原样保留。
///
/// - `script`: 页面内嵌脚本，函数定义之前的部分
///
/// # 返回值
///
/// 第一个 `{` 到最后一个 `}` 之间的 JSON 文本；找不到对象时为 `None`
fn js_object_to_json(script: &str) -> Option<String> {
    let bytes = script.as_bytes();
    let mut out = String::with_capacity(script.len() + script.len() / 8);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'\'' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'\'' {
                    end += 1;
                }
                out.push('"');
                // 页面字符串里没有引号和反斜杠，仍按 JSON 规则转义一遍更稳
                for ch in script[start..end].chars() {
                    if ch == '"' || ch == '\\' {
                        out.push('\\');
                    }
                    out.push(ch);
                }
                out.push('"');
                i = end + 1;
            }
            b',' => {
                // JS 允许尾随逗号，JSON 不允许：后面紧跟收尾括号时整颗逗号丢掉
                let mut next = i + 1;
                while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                    next += 1;
                }
                if next < bytes.len() && matches!(bytes[next], b'}' | b']') {
                    i += 1;
                    continue;
                }
                out.push(',');
                i += 1;
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'$' => {
                let start = i;
                let mut end = i;
                while end < bytes.len() && is_ident(bytes[end]) {
                    end += 1;
                }
                let prev = out.trim_end().bytes().next_back().unwrap_or(0);
                let mut next = end;
                while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                    next += 1;
                }
                let is_key =
                    matches!(prev, b'{' | b',') && next < bytes.len() && bytes[next] == b':';
                if is_key {
                    out.push('"');
                    out.push_str(&script[start..end]);
                    out.push('"');
                } else {
                    out.push_str(&script[start..end]);
                }
                i = end;
            }
            _ => {
                let len = if b < 0x80 {
                    1
                } else if b < 0xE0 {
                    2
                } else if b < 0xF0 {
                    3
                } else {
                    4
                };
                out.push_str(&script[i..i + len]);
                i += len;
            }
        }
    }

    let start = out.find('{')?;
    let end = out.rfind('}')?;
    Some(out[start..=end].to_string())
}

/// 标识符字符：字母、数字、下划线、`$`
fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// 获取 IBM Semeru 的 Java 列表
///
/// # 返回值
///
/// 首次调用抓取并解析下载页，之后直接返回缓存
pub async fn get_java_list() -> CoreResult<OpenJ9ListObj> {
    match OPENJ9.get() {
        Some(data) => Ok(data.clone()),
        None => init().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_select_options() {
        let html = "<select id='arch'><option value='any'>Any</option><option value='x64'>x64</option></select>";
        let list = select_options(html, "arch").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].value, "any");
        assert_eq!(list[1].value, "x64");
    }

    #[test]
    fn keeps_option_class() {
        let html = "<select id='runtimeVersion'><option value='8' class='bx--select-option'>8</option></select>";
        let list = select_options(html, "runtimeVersion").unwrap();
        assert!(list[0].class.contains("bx--select-option"));
    }

    #[test]
    fn missing_select_returns_none() {
        assert!(select_options("<select id='os'></select>", "arch").is_none());
    }

    #[test]
    fn converts_js_literal_to_json() {
        let script = "let sourceDataJson = {\n  downloads: [\n    { name: 'a&b', version: 8, os: 'Linux', arch: 'x64', },\n  ],\n};\n";
        let json = js_object_to_json(script).unwrap();
        assert!(!json.contains("downloads:"));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["downloads"][0]["name"], "a&b");
        assert_eq!(value["downloads"][0]["version"], 8);
    }
}
