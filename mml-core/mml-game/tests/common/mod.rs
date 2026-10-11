//! 共享测试工具：网络样本的下载与缓存。
//!
//! 整合包 / 光影包等二进制样本不允许提交进 git，改为在测试运行时通过
//! Modrinth API 解析下载地址、下载并缓存到系统临时目录。
//! 网络不可用时调用方跳过对应测试，打印提示而不 fail。
//!
//! 该模块按测试二进制单独编译，各二进制只用到部分函数，故关闭死代码警告。

#![allow(dead_code)]

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use mml_base::serialize_tools::json_from_bytes;
use mml_net::modrinth_api::version_obj::ModrinthVersionObj;

/// 网络请求所需的一次性初始化，含配置、语言与 HTTP 客户端
fn init_net() {
    static NET: OnceLock<()> = OnceLock::new();
    NET.get_or_init(|| {
        let dir = mml_testutil::temp_dir().join("mml-test-net-init");
        std::fs::create_dir_all(&dir).ok();
        mml_config::init(&dir);
        mml_names::init(&dir);
        mml_net::init();
    });
}

/// 在同步测试中执行异步请求
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("构建测试 tokio runtime 失败")
    });
    runtime.block_on(future)
}

/// 下载缓存目录，位于系统临时目录下，跨测试与跨进程复用
fn cache_dir() -> PathBuf {
    let dir = mml_testutil::temp_dir().join("mml-test-data");
    std::fs::create_dir_all(&dir).expect("创建测试数据缓存目录失败");
    dir
}

/// 由 URL 推导稳定的缓存文件名，取文件名字段加 URL 哈希
fn cache_path(url: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();
    let name = url
        .rsplit(['/', '?'])
        .find(|s| !s.is_empty() && s.contains('.'))
        .map(|s| {
            s.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .unwrap_or_else(|| "download".to_string());
    cache_dir().join(format!("{hash:016x}-{name}"))
}

/// 下载 URL 到缓存目录并返回本地路径
///
/// 已有缓存直接复用，不重新下载；下载失败返回 `None`，由调用方决定跳过
pub fn download(url: &str) -> Option<PathBuf> {
    let path = cache_path(url);
    if path.exists() {
        return Some(path);
    }
    init_net();
    let data = block_on(mml_net::get_work_client().get_bytes(url)).ok()?;
    if data.is_empty() {
        return None;
    }
    std::fs::write(&path, &data).ok()?;
    Some(path)
}

/// Modrinth 版本里的一个待用文件
pub struct ProjectFile {
    /// 文件名，如 `fabulously-optimized-14.0.0-beta.3.mrpack`
    pub name: String,
    /// 下载地址，绝对 URL
    pub url: String,
}

/// 通过 Modrinth API 解析指定项目与版本号的主文件下载地址
///
/// - `project`: Modrinth 项目 slug
/// - `version`: 版本号
///
/// # 返回值
///
/// 见 [`ProjectFile`]；项目或版本不存在时返回 `None`
pub fn resolve_project_version(project: &str, version: &str) -> Option<ProjectFile> {
    let url = format!("https://api.modrinth.com/v2/project/{project}/version");
    let versions: Vec<ModrinthVersionObj> = get_json(&url)?;
    let target = versions.into_iter().find(|v| v.version_number == version)?;
    pick_file(target)
}

/// 下载测试整合包 Fabulously Optimized 14.0.0-beta.3 到缓存目录
///
/// 版本号是硬编码的：解析测试断言依赖该版本的具体字段与文件数，
/// 若跟随最新版本，字段会随上游变动而失效
pub fn download_mrpack() -> Option<PathBuf> {
    let file = resolve_project_version("fabulously-optimized", "14.0.0-beta.3")?;
    download(&file.url)
}

/// 常用光影包在 Modrinth 上的项目 slug，已逐一验证存在
pub const SHADERPACK_SLUGS: &[&str] = &[
    "bsl-shaders",
    "complementary-reimagined",
    "complementary-unbound",
    "makeup-ultra-fast-shaders",
    "solas-shader",
    "miniature-shader",
    "spooklementary",
];

/// 常用资源包在 Modrinth 上的项目 slug，已逐一验证存在
pub const RESOURCEPACK_SLUGS: &[&str] = &[
    "faithful-32x",
    "faithful-64x",
    "fresh-animations",
    "3d-default",
    "fancy-foliage",
];

/// 解析项目最新版本的主文件并下载到缓存目录
///
/// - `slug`: Modrinth 项目 slug
///
/// # 返回值
///
/// 返回缓存到本地的文件路径；解析或下载失败返回 `None`
pub fn download_latest(slug: &str) -> Option<PathBuf> {
    let url = format!("https://api.modrinth.com/v2/project/{slug}/version");
    let versions: Vec<ModrinthVersionObj> = get_json(&url)?;
    let target = versions.into_iter().next()?;
    let file = pick_file(target)?;
    download(&file.url)
}

/// 取项目的全部版本，Modrinth 的返回顺序是**最新在前**
///
/// 与 [`resolve_project_version`] 的区别：那个只要某个版本号的主文件地址，这个要
/// **完整版本对象**，升级用例需要把它交给 `upgrade_modpack`，并且要能挑出两个版本做比对。
///
/// 异步版：`#[tokio::test]` 里必须用这个，同步版走 `block_on`，在异步上下文里会 panic，
/// 报 "Cannot start a runtime from within a runtime"。
///
/// - `slug`: Modrinth 项目 slug
///
/// # 返回值
///
/// 返回该项目的版本列表；请求或解析失败返回 `None`
pub async fn project_versions_async(slug: &str) -> Option<Vec<ModrinthVersionObj>> {
    init_net();
    let data = mml_net::get_work_client()
        .get_bytes(&format!("https://api.modrinth.com/v2/project/{slug}/version"))
        .await
        .ok()?;
    json_from_bytes(&data).ok()
}

/// 下载某个版本对象的主文件到缓存目录，已有缓存直接复用
///
/// 异步版，理由同上。
///
/// - `version`: Modrinth 版本对象
///
/// # 返回值
///
/// 返回缓存到本地的文件路径；下载失败返回 `None`
pub async fn download_version_async(version: &ModrinthVersionObj) -> Option<PathBuf> {
    let file = version
        .files
        .iter()
        .find(|f| f.primary || !f.url.is_empty())?;
    let path = cache_path(&file.url);
    if path.exists() {
        return Some(path);
    }
    init_net();
    let data = mml_net::get_work_client().get_bytes(&file.url).await.ok()?;
    if data.is_empty() {
        return None;
    }
    std::fs::write(&path, &data).ok()?;
    Some(path)
}

/// 请求 Modrinth API 并反序列化
fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Option<T> {
    init_net();
    let data = block_on(mml_net::get_work_client().get_bytes(url)).ok()?;
    json_from_bytes(&data).ok()
}

/// 从版本对象中挑选主文件，无主文件时取第一个非空地址
fn pick_file(target: ModrinthVersionObj) -> Option<ProjectFile> {
    let file = target
        .files
        .into_iter()
        .find(|f| f.primary || !f.url.is_empty())?;
    Some(ProjectFile {
        name: file.filename,
        url: file.url,
    })
}

// ===================== 程序化造包用的条目 =====================

/// 压缩包里的一个二进制条目
pub struct ZipEntry<'a> {
    /// 压缩包内的条目路径，用 `/` 分隔
    pub name: &'a str,
    /// 条目的原始字节内容，不解码、不改码
    pub data: &'a [u8],
}

/// 造 [`ZipEntry`]，供各造包函数把 `&[(&str, &[u8])]` 换成 `&[entry("a", b"x")]`
pub fn entry<'a>(name: &'a str, data: &'a [u8]) -> ZipEntry<'a> {
    ZipEntry { name, data }
}

/// 压缩包里的一个文本条目
pub struct TextEntry<'a> {
    /// 压缩包内的条目路径，用 `/` 分隔
    pub name: &'a str,
    /// 条目的文本内容，写盘时按 UTF-8 取字节
    pub text: &'a str,
}

/// 造 [`TextEntry`]，用途同 [`entry`]
pub fn text_entry<'a>(name: &'a str, text: &'a str) -> TextEntry<'a> {
    TextEntry { name, text }
}
