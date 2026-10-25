//! 从页面原文里抠标签、属性与文本
//!
//! 只服务几处固定的官方页面：不追求完整 HTML 支持，够用且不必为此引第三方解析器。
//! 页面里的标签属性单双引号混用，这里统一认。

/// 从开标签的属性和里取某个属性值
///
/// - `attrs`: 标签名之后、`>` 之前的原文
/// - `name`: 属性名，按完整单词匹配，不会命中 `data-id` 里的 `id`
///
/// # 返回值
///
/// 属性不存在或没写值时为 `None`
pub(crate) fn attr(attrs: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(pos) = attrs[from..].find(name) {
        let at = from + pos;
        let boundary = at == 0 || attrs[..at].ends_with(char::is_whitespace);
        if boundary {
            let after = &attrs[at + name.len()..];
            for quote in ['\u{22}', '\u{27}'] {
                let head = format!("={quote}");
                if let Some(rest) = after.strip_prefix(head.as_str()) {
                    if let Some(end) = rest.find(quote) {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
        from = at + name.len();
    }
    None
}

/// 找下标 `from` 之后的某个开标签
///
/// - `html`: 页面原文
/// - `from`: 开始查找的下标
/// - `name`: 标签名，如 `tr`；按完整标签名匹配，不会命中 `track` 这类前缀相同的标签
///
/// # 返回值
///
/// 命中时返回标签名之后到 `>` 之前的字节范围，供 [`attr`] 解析属性
pub(crate) fn find_tag(html: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let mut at = from;
    while let Some(pos) = html[at..].find('<') {
        let begin = at + pos + 1;
        if let Some(tail) = html[begin..].strip_prefix(name) {
            if tail
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '>' || c == '/')
            {
                let len = tail.find('>')?;
                let start = begin + name.len();
                return Some((start, start + len));
            }
        }
        at = begin;
    }
    None
}

/// 去掉标签、还原常见实体，并收紧空白
///
/// - `html`: 含标签的片段
///
/// # 返回值
///
/// 纯文本；连续空白压成一个空格，首尾空白去掉
pub(crate) fn text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find('<') {
        out.push_str(&rest[..pos]);
        let Some(end) = rest[pos..].find('>') else {
            rest = "";
            break;
        };
        rest = &rest[pos + end + 1..];
    }
    out.push_str(rest);

    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\u{22}")
        .replace("&#39;", "\u{27}");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}
