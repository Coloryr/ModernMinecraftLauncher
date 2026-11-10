//! 拖入路径探测
//!
//! 只做"文件系统 + 后缀 + 内容"的轻量判定：文件夹 / 压缩包 / 服务器包 / 登录快捷方式。
//! 真正读条目、识别类型由目标窗口再做一次；IPC 命令在 `drop.rs`，
//! 这里放实现，让文本拖入与文件拖入共用同一套地址规则。

use crate::dtos::DropPathDto;

/// 拖入路径的实际判定，跑在阻塞线程上
///
/// - `path`: 拖入文件的绝对路径
///
/// # 返回值
///
/// 见 [`DropPathDto`]
pub(crate) fn probe_drop_inner(path: &str) -> DropPathDto {
    let other = |kind: &str| DropPathDto {
        kind: kind.to_string(),
        url: String::new(),
    };

    let result = if std::fs::metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
    {
        other("folder")
    } else {
        match path
            .to_ascii_lowercase()
            .rsplit_once('.')
            .map(|(_, ext)| ext)
    {
        Some("zip") if crate::windows::export::server_pack::is_server_pack(std::path::Path::new(path)) => {
            other("serverPack")
        }
        Some("zip" | "mrpack" | "modpack") => other("archive"),
        Some("url" | "webloc" | "website") => shortcut_of(path),
        // 卡片拖到桌面后不一定是 `.url`：有的站点给的是 `.json` / `.txt`。
        // 这两类**只在"整份内容就只有一个地址"时才认**，见 single_url_of_text ——
        // 否则 modpack 的 manifest.json 这类里面也含地址的文件会被误判成登录快捷方式。
        Some("json" | "txt") if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) <= 4096 => {
            let url = read_whole_text(path)
                .map(|text| single_url_of_text(&text))
                .unwrap_or_default();
            if url.is_empty() {
                other("other")
            } else {
                DropPathDto {
                    kind: String::from("shortcut"),
                    url,
                }
            }
        }
        _ => other("other"),
        }
    };

    result
}

/// 快捷方式文件 `.url` / `.webloc` / `.website` → 解析出的地址
fn shortcut_of(path: &str) -> DropPathDto {
    let url = read_shortcut_url(path);
    if url.is_empty() {
        // 快捷方式坏了、读不出地址：当不认识的文件处理，别让上层弹个空表单
        DropPathDto {
            kind: String::from("other"),
            url: String::new(),
        }
    } else {
        DropPathDto {
            kind: String::from("shortcut"),
            url,
        }
    }
}

/// 整份读成文本；只给拖入探测用，调用方已按后缀与大小筛过
fn read_whole_text(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// 文本里**只有一个地址**时取它；卡片拖到桌面可能是 `.json` / `.txt`
///
/// 判据：整份内容里只有一个 http(s) 地址，且除它以外的字符不超过 64 个 ——
/// 这样 `"https://…"`、`{"url":"https://…"}` 都认，而 modpack 的 manifest.json
/// 这种一堆地址的、或夹着大段说明的文本不会被误判成登录快捷方式。
fn single_url_of_text(text: &str) -> String {
    let urls = all_urls(text);
    if urls.len() != 1 {
        return String::new();
    }
    let url = &urls[0];
    if text.len().saturating_sub(url.len()) > 64 {
        return String::new();
    }
    url.clone()
}

/// 文本里出现的全部 http(s) 地址
fn all_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = text;
    while let Some(url) = find_http(rest) {
        let offset = rest.find(&url).unwrap_or(0);
        rest = &rest[offset + url.len()..];
        urls.push(url);
    }
    urls
}

/// 读快捷方式文件里的地址：`.url` / `.website` 是 INI 的 `URL=`，`.webloc` 是 plist
///
/// plist 有 XML 与**二进制**两种：二进制那种字符串也是原样存的，所以兜底一路
/// "在文件里找第一个 http(s) 地址"两种情况都能捞到，不必引 plist 解析库。
pub(crate) fn read_shortcut_url(path: &str) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    // 快捷方式可能有 BOM / UTF-16：统一按 lossy 文本处理，地址都是 ASCII，够用
    let text = String::from_utf8_lossy(&bytes);

    for line in text.lines() {
        let line = line.trim();
        // 大小写不敏感：Windows 自己写的是 URL=，但别人生成的可能是 url=
        if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("URL")
        {
            let url = clean_url(value);
            if !url.is_empty() {
                return url;
            }
        }
    }

    find_http(&text).unwrap_or_default()
}

/// 规范化快捷方式里的地址：去掉首尾空白与结尾的斜杠，后端认证时自己会补 `/`
///
/// 只有协议头、没有主机的，如 `https://`，算无效，返回空串 —— 免得拿一个残地址
/// 去弹账户表单。
pub(crate) fn clean_url(value: &str) -> String {
    let url = value.trim().trim_end_matches(['/', '\\']);
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match host {
        Some(host) if !host.is_empty() => url.to_string(),
        _ => String::new(),
    }
}

/// 在文本里找第一个 http(s) 地址，到空白 / 引号 / 尖括号 / NUL 为止
pub(crate) fn find_http(text: &str) -> Option<String> {
    let start = match (text.find("https://"), text.find("http://")) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    let rest = &text[start..];
    let end = rest
        .find(|item: char| {
            item.is_whitespace() || item == '"' || item == '\'' || item == '<' || item == '\0'
        })
        .unwrap_or(rest.len());
    let url = clean_url(&rest[..end]);
    if url.is_empty() { None } else { Some(url) }
}

/// 从拖入的文本里取地址：先按整段规范化，取不到再找第一个 http(s)
pub(crate) fn url_of_text(text: &str) -> String {
    let direct = clean_url(text);
    if !direct.is_empty() {
        return direct;
    }
    find_http(text).unwrap_or_default()
}
