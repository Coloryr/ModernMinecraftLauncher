//! 自定义整页站点的公共部分：整包句柄、越界防护与协议响应
//!
//! 自定义主页面（`mml-home`）与自定义界面的整页模式（`mml-ui`）都是「用户导入一个 zip，
//! 当站点伺服」：整包**不落盘解压**，请求时按条目从包里现读。两者的差别只有压缩包路径、
//! 入口页名与额外的静态文件，路径解析与越界防护因此只写这一份 ——
//! [`resolve_entry`] 是唯一的安全边界。

use std::collections::HashMap;
use std::path::Path;

use mml_base::archives::{ArchiveType, BaseArchive};
use tauri::UriSchemeResponder;
use tauri::http::{Response, StatusCode};

/// 单次导入允许的最大条目数，含目录条目
pub(crate) const MAX_ENTRIES: usize = 4096;
/// 单个条目解压后的最大字节数
pub(crate) const MAX_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
/// 全部条目解压后的最大字节数
pub(crate) const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
/// 单个压缩包文件的最大字节数
pub(crate) const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
/// 单条目的最大压缩比，超过按 zip 炸弹拒绝
pub(crate) const MAX_COMPRESSION_RATIO: u64 = 200;

/// 桥接脚本的请求路径；内置资源，整包里的同名文件不生效
pub(crate) const BRIDGE_PATH: &str = "/__mml_bridge.js";

/// 桥接脚本本体，独立 js 文件放 `src-tauri/resources/`，别塞成 Rust 字符串字面量
///
/// `include_str!` 在编译期嵌入二进制，运行时不需要分发这个文件，因此**不要**把它
/// 加进 tauri.conf.json 的 `bundle.resources`，那会多打一份没人读的副本。
pub(crate) const BRIDGE_JS: &str = include_str!("../../resources/custom_home_bridge.js");

/// 注入到 HTML 里的桥接脚本标签
pub(crate) const BRIDGE_TAG: &str = "<script src=\"/__mml_bridge.js\"></script>";

/// 常驻的只读整包句柄：条目名 → 包内原始条目名，读取时按条目解压到内存
///
/// `BaseArchive` 内部是 `Mutex<Box<dyn ArchiveHandle>>` 且 `Send + Sync`，
/// 所以能常驻一份给协议 handler 反复按条目读；条目表只在打开时读一次。
pub(crate) struct WebArchive {
    /// 条目索引：规范化后的名字 → 包内原始条目名，名字里的分隔符统一成正斜杠
    ///
    /// 单独建索引是因为 zip 里的名字可能用反斜杠，PowerShell 的 `Compress-Archive`
    /// 就是这么写的，而 URL 路径一定是正斜杠。
    index: HashMap<String, String>,
    /// 大小写不敏感索引：小写名 → 规范化名，只给 Windows 上大小写写错的包兜底
    ///
    /// 只差大小写的重名条目不进这张表，宁可 404 也不猜是哪一份。
    case_index: HashMap<String, String>,
    /// 常驻的只读整包句柄，按条目名读取内容
    archive: BaseArchive,
}

impl WebArchive {
    /// 打开压缩包并建好条目索引，同时校验条目数与解压后大小
    ///
    /// 限额防的是 zip 炸弹：条目数、单条大小、总大小任一超限都拒绝，
    /// 免得一次请求就把内存吃满。按 Zip 显式打开，不看扩展名：自定义主页面存成
    /// 不带扩展名的 `home`。
    ///
    /// - `path`: 压缩包路径
    ///
    /// # 返回值
    ///
    /// 打开好的句柄；打不开时返回底层错误信息，超限时返回 `err.pageArchiveLimit`
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        let archive =
            BaseArchive::open_readonly_as(path, ArchiveType::Zip).map_err(|err| err.to_string())?;
        let entries = archive.entries();
        if entries.len() > MAX_ENTRIES {
            return Err(String::from(crate::err_keys::PAGE_ARCHIVE_LIMIT));
        }
        let mut total = 0u64;
        for entry in entries {
            if entry.is_dir {
                continue;
            }
            if entry.size > MAX_ENTRY_BYTES {
                return Err(String::from(crate::err_keys::PAGE_ARCHIVE_LIMIT));
            }
            total = total.saturating_add(entry.size);
            if total > MAX_TOTAL_BYTES {
                return Err(String::from(crate::err_keys::PAGE_ARCHIVE_LIMIT));
            }
        }

        let mut index = HashMap::with_capacity(entries.len());
        let mut case_index: HashMap<String, String> = HashMap::with_capacity(entries.len());
        for entry in entries {
            if entry.is_dir {
                continue;
            }
            // 正斜杠化 + 去掉前导 ./，让 URL 路径能直接查到；包内原名照旧用于读取
            let normalized = entry.name.replace('\\', "/");
            let normalized = normalized.trim_start_matches("./").to_string();
            match case_index.entry(normalized.to_ascii_lowercase()) {
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(normalized.clone());
                }
                std::collections::hash_map::Entry::Occupied(slot) => {
                    slot.remove();
                }
            }
            index.insert(normalized, entry.name.clone());
        }
        Ok(Self {
            index,
            case_index,
            archive,
        })
    }

    /// 按规范化名字取包内原始条目名
    ///
    /// 依次尝试：原名 → 补 `./` 前缀 → 大小写不敏感版本。最后一步只给 Windows 上
    /// 大小写写错的包兜底，命中时调用方按原样伺服。
    ///
    /// - `name`: 规范化后的条目名，分隔符为正斜杠
    ///
    /// # 返回值
    ///
    /// 包内原始条目名；不存在返回 `None`
    pub(crate) fn entry_name(&self, name: &str) -> Option<&str> {
        if let Some(found) = self.index.get(name) {
            return Some(found.as_str());
        }
        let dot = format!("./{name}");
        if let Some(found) = self.index.get(&dot) {
            return Some(found.as_str());
        }
        self.case_index
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// 读一个条目，内容全部读进内存
    ///
    /// - `name`: 规范化后的条目名
    ///
    /// # 返回值
    ///
    /// 条目内容；条目不存在或解压失败返回 `None`
    pub(crate) fn read(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entry_name(name)?;
        self.archive.read(entry).ok()
    }

    /// 包内文件数，不含目录条目
    ///
    /// # 返回值
    ///
    /// 条目索引里的文件条数
    pub(crate) fn file_count(&self) -> u32 {
        self.index.len() as u32
    }

    /// 唯一顶层目录名
    ///
    /// 用来兜底「整包套一层目录」的布局：入口页在 `<目录>/index.html` 时，
    /// URL 里的 `/index.html` 也能命中。
    ///
    /// # 返回值
    ///
    /// 唯一的顶层目录名；包根就散放着文件时返回 `None`
    pub(crate) fn single_top_dir(&self) -> Option<String> {
        let mut first: Option<&str> = None;
        for name in self.index.keys() {
            let top = name.split('/').next()?;
            // 根目录下的文件名字里没有 /，说明不是「整包套一层」
            if !name.contains('/') || top.is_empty() {
                return None;
            }
            match first {
                None => first = Some(top),
                Some(seen) if seen == top => {}
                Some(_) => return None,
            }
        }
        first.map(str::to_string)
    }

    /// 入口页在不在
    ///
    /// - `index`: 入口页文件名，如 `index.html`
    ///
    /// # 返回值
    ///
    /// 包根或唯一顶层目录下存在入口页时为 `true`
    pub(crate) fn has_entry(&self, index: &str) -> bool {
        self.entry_name(index).is_some()
            || self
                .single_top_dir()
                .is_some_and(|dir| self.entry_name(&format!("{dir}/{index}")).is_some())
    }
}

// ================= 协议 =================

/// 以 200 响应，并带上 CORS 头
///
/// iframe 是 opaque origin，取子资源要过 CORS，所以响应必须带
/// `Access-Control-Allow-Origin`。
///
/// - `res`: 协议响应器
/// - `data`: 响应体字节
/// - `mime`: 响应体的 Content-Type
pub(crate) fn send_ok(res: UriSchemeResponder, data: Vec<u8>, mime: &str) {
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
pub(crate) fn send_not_found(res: UriSchemeResponder) {
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
fn hex_val(b: u8) -> Option<u8> {
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
fn percent_decode(s: &str) -> String {
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
fn is_safe_rel(rel: &str) -> bool {
    if rel.contains('\\') || rel.contains(':') || rel.contains('\0') {
        return false;
    }
    !rel.split('/').any(|seg| seg == "..")
}

/// 请求路径 → 包内条目名
///
/// 路径穿越防护，两个整页协议唯一的安全边界：
/// 1. 原始串与百分号解码后都要过 [`is_safe_rel`]，挡掉 `..` / `\` / `:`；
/// 2. 只接受**规范化后能精确命中包内条目**的结果，[`WebArchive::entry_name`] 是精确匹配
///    而不是前缀拼接，所以不存在拼出包外路径的可能；
/// 3. 入口页允许两种布局：包根 `<入口>`，或整包套一层目录时的 `<目录>/<入口>`。
///
/// - `archive`: 常驻的整包句柄
/// - `raw_path`: 请求 URI 里的路径部分
/// - `index`: 入口页文件名
///
/// # 返回值
///
/// 包内原始条目名；未导入、越界或不存在时返回 `None`
pub(crate) fn resolve_entry<'a>(
    archive: &'a WebArchive,
    raw_path: &str,
    index: &str,
) -> Option<&'a str> {
    let rel = if raw_path.is_empty() || raw_path == "/" {
        index.to_string()
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
    // 整包套一层目录：把唯一顶层目录名的前缀试一遍，子资源与入口脚本同样适用
    if let Some(dir) = archive.single_top_dir() {
        let wrapped = format!("{dir}/{name}");
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
pub(crate) fn mime_of(name: &str) -> &'static str {
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
fn find_ignore_ascii_case(hay: &str, needle: &str) -> Option<usize> {
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
/// - `tag`: 要注入的脚本标签
///
/// # 返回值
///
/// 注入脚本后的响应体字节
pub(crate) fn inject_bridge(data: Vec<u8>, tag: &str) -> Vec<u8> {
    let text = match String::from_utf8(data) {
        Ok(text) => text,
        Err(err) => return err.into_bytes(),
    };
    let mut out = String::with_capacity(text.len() + tag.len());
    match find_ignore_ascii_case(&text, "</head>") {
        Some(pos) => {
            out.push_str(&text[..pos]);
            out.push_str(tag);
            out.push_str(&text[pos..]);
        }
        None => {
            out.push_str(tag);
            out.push_str(&text);
        }
    }
    out.into_bytes()
}

/// 把源压缩包原子地替换到目标位置
///
/// 先复制到目标同目录的 `<目标>.tmp`，成功后再改名上架。`rename` 会覆盖已存在的
/// 目标，所以不能先删旧包：万一改名失败，旧包还得原样留在那里。任何一步失败都只删
/// 临时文件，常驻句柄在替换成功后才重开。
///
/// - `src`: 用户选的压缩包
/// - `dst`: 落盘位置，带不带扩展名都行
///
/// # 返回值
///
/// 替换成功返回 `()`；复制或改名失败时返回 IO 错误信息
pub(crate) fn replace_atomic(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    // 临时名直接在原名后加 .tmp：目标可能没有扩展名，set_extension 会拼出 home.zip.tmp
    let mut tmp = dst.to_path_buf();
    tmp.as_mut_os_string().push(".tmp");
    if tmp.exists() {
        std::fs::remove_file(&tmp).map_err(|err| err.to_string())?;
    }

    let result = (|| {
        std::fs::copy(src, &tmp).map_err(|err| err.to_string())?;
        std::fs::rename(&tmp, dst).map_err(|err| err.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mml_base::archives::{ArchiveType, BaseArchive};

    use super::*;

    /// 路径穿越：`..` 段、反斜杠与冒号一律拒绝
    #[test]
    fn rejects_unsafe_relative_paths() {
        assert!(is_safe_rel("assets/app.js"));
        assert!(is_safe_rel("index.html"));
        assert!(!is_safe_rel("../secret.txt"));
        assert!(!is_safe_rel("a/../../b"));
        assert!(!is_safe_rel("a\\b.js"));
        assert!(!is_safe_rel("C:/Windows/win.ini"));
        assert!(!is_safe_rel("a\0b"));
    }

    /// 编码后的穿越要在解码之后再过一次检查
    #[test]
    fn percent_decode_restores_bytes() {
        assert_eq!(percent_decode("..%2fsecret"), "../secret");
        assert_eq!(percent_decode("100%.png"), "100%.png");
    }

    /// 造一个只有入口页与一个资源文件的整页包
    fn test_archive() -> (PathBuf, WebArchive) {
        let dir = mml_testutil::temp_path(format!("web-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let zip = dir.join("page.zip");
        let mut archive = BaseArchive::create_empty(ArchiveType::Zip, &zip).expect("建包");
        archive.add_data("index.html", b"<html></html>").expect("写入口页");
        archive.add_data("assets/app.js", b"console.log(1)").expect("写资源");
        drop(archive);
        let opened = WebArchive::open(&zip).expect("打开包");
        (dir, opened)
    }

    /// 整包套一层目录时，入口页、子资源与入口脚本都能解析
    #[test]
    fn resolves_entries_inside_single_top_dir() {
        let dir = mml_testutil::temp_path(format!("web-wrapped-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let zip = dir.join("wrapped.zip");
        let mut archive = BaseArchive::create_empty(ArchiveType::Zip, &zip).expect("建包");
        archive.add_data("demo/index.html", b"<html></html>").expect("写入口页");
        archive.add_data("demo/assets/app.js", b"console.log(1)").expect("写资源");
        archive.add_data("demo/custom.js", b"console.log(2)").expect("写入口脚本");
        drop(archive);
        let archive = WebArchive::open(&zip).expect("打开包");

        assert_eq!(resolve_entry(&archive, "/", "index.html"), Some("demo/index.html"));
        assert_eq!(
            resolve_entry(&archive, "/assets/app.js", "index.html"),
            Some("demo/assets/app.js")
        );
        assert_eq!(
            resolve_entry(&archive, "/custom.js", "custom.css"),
            Some("demo/custom.js")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 只认包内条目：根路径落到入口页，越界与不存在的路径都返回 None
    #[test]
    fn resolves_only_entries_inside_archive() {
        let (dir, archive) = test_archive();
        assert_eq!(resolve_entry(&archive, "/", "index.html"), Some("index.html"));
        assert_eq!(
            resolve_entry(&archive, "/assets/app.js", "index.html"),
            Some("assets/app.js")
        );
        assert_eq!(resolve_entry(&archive, "/%2e%2e/secret.txt", "index.html"), None);
        assert_eq!(resolve_entry(&archive, "/../secret.txt", "index.html"), None);
        assert_eq!(resolve_entry(&archive, "/missing.js", "index.html"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 扩展名决定 MIME，未知类型一律二进制流
    #[test]
    fn mime_follows_extension() {
        assert_eq!(mime_of("index.html"), "text/html");
        assert_eq!(mime_of("a.WOFF2"), "font/woff2");
        assert_eq!(mime_of("blob"), "application/octet-stream");
    }
}
