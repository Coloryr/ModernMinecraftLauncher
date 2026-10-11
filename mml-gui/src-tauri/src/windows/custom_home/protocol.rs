//! `mml-home` 自定义协议：把用户导入的整包网页资源当站点伺服
//!
//! 从 `custom_home/mod.rs` 拆出来的。路径一律按"ZIP 内相对路径"处理，
//! **必须先过 [`is_safe_rel`]**，它会拒绝 `..`、绝对路径与盘符，否则自定义主页就成了
//! 任意文件读取入口 —— 这是本模块最要紧的不变量。`url_custom_home` 由 `lib.rs`
//! 注册为协议处理器，按原路径再导出。

use std::path::Path;

use tauri::UriSchemeResponder;
use tauri::http::{Request, Response, StatusCode};

use super::archive::{HomeArchive, INDEX_FILE, open_handle};

/// 持久压缩包名，在运行根目录下；前缀统一，改名时两处一起改
pub(crate) const ZIP_PREFIX: &str = "custom_home";

/// 桥接脚本的请求路径；内置资源，zip 里的同名文件不生效
pub(super) const BRIDGE_PATH: &str = "/__mml_bridge.js";

/// 桥接脚本本体，独立 js 文件放 `src-tauri/resources/`，别塞成 Rust 字符串字面量
///
/// `include_str!` 在编译期嵌入二进制，运行时不需要分发这个文件，因此**不要**把它
/// 加进 tauri.conf.json 的 `bundle.resources`，那会多打一份没人读的副本。
pub(super) const BRIDGE_JS: &str = include_str!("../../../resources/custom_home_bridge.js");

/// 注入到 HTML 里的桥接脚本标签
pub(super) const BRIDGE_TAG: &str = "<script src=\"/__mml_bridge.js\"></script>";

/// 以 200 响应，并带上 CORS 头
///
/// iframe 是 opaque origin，取子资源要过 CORS，所以响应必须带
/// `Access-Control-Allow-Origin`。
///
/// - `res`: 协议响应器
/// - `data`: 响应体字节
/// - `mime`: 响应体的 Content-Type
pub(super) fn send_ok(res: UriSchemeResponder, data: Vec<u8>, mime: &str) {
    res.respond(
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime)
            .header("Access-Control-Allow-Origin", "*")
            .body(data)
            .unwrap(),
    );
}

/// 以 404 响应，用于未导入、文件不存在或路径越界
///
/// - `res`: 协议响应器
pub(super) fn send_not_found(res: UriSchemeResponder) {
    res.respond(
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Access-Control-Allow-Origin", "*")
            .body(Vec::new())
            .unwrap(),
    );
}

/// 十六进制半字节
///
/// - `b`: 单个 ASCII 字符
///
/// # 返回值
///
/// `0-9` / `a-f` / `A-F` 对应的数值；其余字符返回 `None`
pub(super) fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// 百分号解码，把 `%XX` 还原成字节
///
/// 非法转义原样保留，比如真实文件名里的 `100%.png`，交给后面的安全检查处理；
/// 而 `%2e%2e` 这类编码穿越会在解码后被 `..` 检查挡掉。
///
/// - `s`: 待解码的字符串
///
/// # 返回值
///
/// 解码后的字符串，非 UTF-8 字节按替换符处理
pub(super) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 请求的相对路径是否安全
///
/// 拒绝反斜杠、`:` 与 NUL，以及任何 `..` 段；反斜杠在 Windows 上是路径分隔符，
/// `:` 可能是盘符或协议。
///
/// - `rel`: 待检查的相对路径
///
/// # 返回值
///
/// 安全返回 `true`，命中任一条拒绝规则返回 `false`
pub(super) fn is_safe_rel(rel: &str) -> bool {
    if rel.contains('\\') || rel.contains(':') || rel.contains('\0') {
        return false;
    }
    !rel.split('/').any(|seg| seg == "..")
}

/// 请求路径 → 包内条目名
///
/// 路径穿越防护，本次唯一强制安全项：
/// 1. 原始串与百分号解码后都要过 [`is_safe_rel`]，挡掉 `..` / `\` / `:`；
/// 2. 只接受**规范化后能精确命中包内条目**的结果，[`HomeArchive::entry_name`] 是精确匹配
///    而不是前缀拼接，所以不存在拼出包外路径的可能；
/// 3. 入口页允许两种布局：包根 `index.html`，或整包套一层目录时的 `<目录>/index.html`。
///
/// - `archive`: 常驻的整包句柄
/// - `raw_path`: 请求 URI 里的路径部分
///
/// # 返回值
///
/// 包内原始条目名；未导入、越界或不存在时返回 `None`
pub(super) fn resolve_entry<'a>(archive: &'a HomeArchive, raw_path: &str) -> Option<&'a str> {
    let rel = if raw_path.is_empty() || raw_path == "/" {
        INDEX_FILE.to_string()
    } else {
        raw_path.to_string()
    };
    if !is_safe_rel(&rel) {
        return None;
    }
    let decoded = percent_decode(&rel);
    if !is_safe_rel(&decoded) {
        return None;
    }
    // 去掉前导 / 与 ./，拼成规范化条目名
    let name = decoded
        .trim_start_matches('/')
        .trim_start_matches("./")
        .to_string();
    if name.is_empty() {
        return None;
    }

    if let Some(found) = archive.entry_name(&name) {
        return Some(found);
    }
    // 整包套一层目录：把唯一顶层目录名的前缀试一遍；只在根请求时兜底，避免每次都花这份开销
    if name == INDEX_FILE
        && let Some(dir) = archive.single_top_dir()
    {
        let wrapped = format!("{dir}/{INDEX_FILE}");
        return archive.entry_name(&wrapped);
    }
    None
}

/// 按扩展名给 MIME
///
/// - `name`: 包内条目名
///
/// # 返回值
///
/// 对应的 MIME；未知扩展名一律 `application/octet-stream`
pub(super) fn mime_of(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") | Some("mjs") => "application/javascript",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

/// 大小写不敏感地找 ASCII 子串
///
/// - `hay`: 被查找的文本
/// - `needle`: 要查找的子串
///
/// # 返回值
///
/// 子串首个匹配位置的字节下标；没找到返回 `None`
pub(super) fn find_ignore_ascii_case(hay: &str, needle: &str) -> Option<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// 在 HTML 的 `</head>` 之前插入桥接脚本；没有 `</head>` 就插在最前面
///
/// 响应体不是合法 UTF-8 时原样返回，页面仍能显示，只是拿不到 `window.mml`。
///
/// - `data`: HTML 响应体字节
///
/// # 返回值
///
/// 注入脚本后的响应体字节
pub(super) fn inject_bridge(data: Vec<u8>) -> Vec<u8> {
    let text = match String::from_utf8(data) {
        Ok(text) => text,
        Err(err) => return err.into_bytes(),
    };
    let mut out = String::with_capacity(text.len() + BRIDGE_TAG.len());
    match find_ignore_ascii_case(&text, "</head>") {
        Some(pos) => {
            out.push_str(&text[..pos]);
            out.push_str(BRIDGE_TAG);
            out.push_str(&text[pos..]);
        }
        None => {
            out.push_str(BRIDGE_TAG);
            out.push_str(&text);
        }
    }
    out.into_bytes()
}

/// `mml-home` 协议入口：从常驻的压缩包里按条目读取，按 MIME 返回
///
/// `/` 或空路径落到 `index.html`，含「整包套一层目录」的兜底；`/__mml_bridge.js`
/// 是内置资源，直接返回桥接脚本，不在包里；query string 与 fragment 由
/// `uri().path()` 天然滤掉，`?v=...` 只是破缓存；全程不落盘，内容是
/// `BaseArchive::read` 从 zip 条目现读出来的。
///
/// - `req`: 协议请求
/// - `res`: 协议响应器
pub(crate) async fn url_custom_home(req: Request<Vec<u8>>, res: UriSchemeResponder) {
    let path = req.uri().path();

    if path == BRIDGE_PATH {
        send_ok(res, BRIDGE_JS.as_bytes().to_vec(), "application/javascript");
        return;
    }

    let Some(archive) = open_handle() else {
        send_not_found(res);
        return;
    };
    let Some(name) = resolve_entry(&archive, path) else {
        send_not_found(res);
        return;
    };
    let Some(data) = archive.read(name) else {
        send_not_found(res);
        return;
    };

    let mime = mime_of(name);
    let data = if mime == "text/html" {
        inject_bridge(data)
    } else {
        data
    };
    send_ok(res, data, mime);
}
