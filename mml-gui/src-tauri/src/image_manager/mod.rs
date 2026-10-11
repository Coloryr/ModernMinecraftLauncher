//! 图像管理：`mml-image` 自定义协议的后端
//!
//! 前端用 `image_base_url()` 拼地址，请求经 [`url_image`] 按首段路由：
//! `instance`：实例图标；
//! `head`：账户头像；`skin`：皮肤全身图；`cape`：披风正面图；`capeback`：披风背面图
//! ——渲染方式由后端读配置决定，
//! 前端只挑"要哪种图"，不挑"怎么渲染"；
//! `skinraw` / `caperaw`：原始皮肤 / 披风贴图，供前端 skinview3d 数据用；
//! `icon`：远程图片，带内存 + 磁盘 + ETag 缓存；
//! `block`：方块贴图；`item`：物品贴图；`screenshot`：实例截图。
//!
//! 皮肤 / 披风纹理一律按**内容 SHA1** 取，与游戏 assets objects 同款布局，
//! 同一份贴图跨账户只存一份：`skinraw/<sha1>`、`caperaw/<sha1>`、
//! `skin/<sha1>[/<skin_type>]`、`cape/<sha1>`、`capeback/<sha1>`。
//! 账户型 URI `head/skin/cape*/<账户类型>/<uuid>` 保留给账户列表的
//! "当前选中"预览：解析账户 → 下载定 sha1 → 转调 sha1 型渲染。
//! 渲染缓存键也用内容 sha1，同一贴图的渲染结果跨账户共享。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, OnceLock, RwLock},
    time::{Duration, Instant},
};

use mml_names::names;
use tauri::{UriSchemeResponder, http::Request};
use uuid::Uuid;

mod cape;
mod head;
mod icon;
mod load;
mod respond;
mod skin;

use self::cape::{load_cape_back_image, load_cape_image, load_caperaw_image};
use self::head::load_head_image;
use self::icon::{icon_name, load_icon_image, load_url_image, save_url_image};
use self::load::{load_block_image, load_instance_image, load_item_image, load_screenshot_image};
use self::respond::{send_bad, split_path};
use self::skin::{load_skin_image, load_skinraw_image};

/// 实例图标内存缓存
pub(super) static INSTANCE_IMAGE: LazyLock<RwLock<HashMap<Uuid, Vec<u8>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 渲染图缓存的键：皮肤内容 sha1 加渲染配置
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PairImageKey {
    /// 皮肤 / 披风内容的 sha1
    pub(super) sha1: String,
    /// 渲染配置，决定同一份皮肤出哪张图
    pub(super) config: String,
}

/// 「(皮肤内容 sha1, 渲染配置) → PNG 字节」的内存缓存类型，头像与皮肤全身图同形状
pub(super) type PairImageCache = LazyLock<RwLock<HashMap<PairImageKey, Vec<u8>>>>;

/// 账户头像内存缓存，键是皮肤内容 sha1 加头像渲染配置；同一皮肤的头像跨账户共享
pub(super) static HEAD_IMAGE: PairImageCache = LazyLock::new(|| RwLock::new(HashMap::new()));
/// 皮肤全身渲染图内存缓存，键是内容 sha1、显示模式与皮肤类型段
pub(super) static SKIN_IMAGE: PairImageCache = LazyLock::new(|| RwLock::new(HashMap::new()));
/// 皮肤原图 PNG 内存缓存，键是内容 sha1
pub(super) static SKINRAW_IMAGE: LazyLock<RwLock<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 披风原图 PNG 内存缓存，键是内容 sha1
pub(super) static CAPERAW_IMAGE: LazyLock<RwLock<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 披风渲染图内存缓存，键是内容 sha1
pub(super) static CAPE_IMAGE: LazyLock<RwLock<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 披风背面渲染图内存缓存，键是内容 sha1
pub(super) static CAPE_BACK_IMAGE: LazyLock<RwLock<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 纹理内容 sha1 → 源 URL：本地文件被清后按登记地址重新下载
pub(super) static TEXTURE_URLS: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 远程图标内存缓存，键是请求名，也就是网址的 sha256
pub(super) static ICON_IMAGE: LazyLock<RwLock<HashMap<String, IconCache>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
/// 图标磁盘缓存目录，即 `<运行目录>/image`
pub(super) static ICON_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 图标内存缓存存活时长，超时后删除并回源重新下载
pub(super) const ICON_TIMEOUT: Duration = Duration::from_secs(60);

/// 图标缓存条目
#[derive(Clone)]
pub(super) struct IconCache {
    /// 图片数据
    data: Vec<u8>,
    /// 内容类型
    mime: &'static str,
    /// 写入时刻
    time: Instant,
}

/// 图标请求名即网址的 sha256 → 远程网址
pub(super) static URL_IMAGE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 初始化图标磁盘缓存目录，启动时调用
pub fn init() {
    ICON_DIR.get_or_init(|| mml_downloader::get_cache_path().join(names::IMAGE_DIR));
    // 登记表也读回来：没有它，收藏夹里存下来的 `.../icon/<sha>` 地址就找不回远程网址，
    // 既回不了源、也做不了过期校验，只能一直吃磁盘缓存
    load_url_image();
}

/// 实例图标内存缓存失效，实例图标被外部更换后调用，让下次请求重读磁盘
///
/// - `uuid`: 实例 uuid
pub fn clear_instance_image(uuid: &Uuid) {
    INSTANCE_IMAGE.write().unwrap().remove(uuid);
}

/// 清空全部纹理类内存缓存，供账户列表"刷新皮肤"用
///
/// 渲染缓存键是内容 sha1 而非账户维度，量小全清最稳，重渲染开销可忽略；
/// 同时清掉 sha1→URL 登记表，下次取图重新走会话服务器下载并登记。
/// 本地皮肤文件按内容缓存，服务器换肤后贴图内容变化自动落新文件，无需删文件；
/// webview 侧的旧图由前端 bump 图片版本号破掉。
pub fn clear_texture_images() {
    HEAD_IMAGE.write().unwrap().clear();
    SKIN_IMAGE.write().unwrap().clear();
    SKINRAW_IMAGE.write().unwrap().clear();
    CAPERAW_IMAGE.write().unwrap().clear();
    CAPE_IMAGE.write().unwrap().clear();
    CAPE_BACK_IMAGE.write().unwrap().clear();
    TEXTURE_URLS.write().unwrap().clear();
}

/// 登记纹理的源 URL，键是内容 sha1；本地文件被清后 sha1 型请求能回源
///
/// - `sha1`: 纹理内容 SHA1，40 位十六进制
/// - `url`: 下载源地址；任一项为空串就忽略
pub fn push_texture_url(sha1: &str, url: &str) {
    if sha1.is_empty() || url.is_empty() {
        return;
    }
    TEXTURE_URLS
        .write()
        .unwrap()
        .insert(sha1.to_string(), url.to_string());
}

/// `mml-image` 协议入口：按 URI 首段路由到各加载函数
///
/// - `req`: 协议请求
/// - `res`: 响应回写句柄
pub async fn url_image(req: Request<Vec<u8>>, res: UriSchemeResponder) {
    let uri = split_path(req.uri().path());
    let image_type = uri.first().copied().unwrap_or_default();

    if image_type == "instance" {
        load_instance_image(&uri, res);
    } else if image_type == "head" {
        load_head_image(&uri, res).await;
    } else if image_type == "skin" {
        load_skin_image(&uri, res).await;
    } else if image_type == "skinraw" {
        load_skinraw_image(&uri, res).await;
    } else if image_type == "caperaw" {
        load_caperaw_image(&uri, res).await;
    } else if image_type == "cape" {
        load_cape_image(&uri, res).await;
    } else if image_type == "capeback" {
        load_cape_back_image(&uri, res).await;
    } else if image_type == "icon" {
        load_icon_image(&uri, res).await;
    } else if image_type == "block" {
        load_block_image(&uri, res);
    } else if image_type == "item" {
        load_item_image(&uri, res);
    } else if image_type == "screenshot" {
        load_screenshot_image(&uri, res);
    } else {
        send_bad(res);
    }
}

/// `mml-image` 自定义协议的访问前缀
///
/// Windows / Android 上自定义协议被映射到 `http://<scheme>.localhost`，
/// 其余平台是 `<scheme>://localhost`，与 Tauri `convertFileSrc` 的规则一致。
///
/// # 返回值
///
/// 可直接拼接图片 URI 的协议前缀
pub fn image_base_url() -> &'static str {
    if cfg!(windows) || cfg!(target_os = "android") {
        "http://mml-image.localhost"
    } else {
        "mml-image://localhost"
    }
}

/// 登记远程图片地址并返回可直接用于 `src` 的网址
///
/// - `url`: 远程图片网址
///
/// # 返回值
///
/// 请求名取网址的 sha256，所以返回值形如 `http://mml-image.localhost/icon/<sha256>`，
/// 已含协议前缀，前端无需再拼接；同一个网址每次返回同一个地址，重复渲染不会产生新地址。
pub fn push_image_url(url: &str) -> String {
    let name = icon_name(url);

    // 登记表要落盘：只放内存里的话，重启后收藏夹存下来的 `.../icon/<sha>` 地址
    // 就查不回远程网址了，既回不了源、也走不了 ETag 过期校验。
    // 同一个网址重复登记很常见，各窗口都会渲染，所以只在真的变了才写
    let changed = {
        let mut lock = URL_IMAGE.write().unwrap();
        lock.insert(name.clone(), url.to_string()).as_deref() != Some(url)
    };
    if changed {
        save_url_image();
    }

    format!("{}/icon/{}", image_base_url(), name)
}

/// 把"本地图片协议地址"还原成登记时的远程网址
///
/// [`push_image_url`] 返回的 `.../icon/<sha256>` 只是个**进程内**的映射键，
/// 凡是要落盘的地方都得先用这个换回原始网址，存下来重启后就是个死地址，
/// 既回不了源，也走不了 ETag 过期校验。
///
/// - `local`: 形如 `.../icon/<sha256>` 的地址，带 `?v=` 查询串也能认
///
/// # 返回值
///
/// 登记表里有就返回原始网址，否则 `None`
pub fn remote_url(local: &str) -> Option<String> {
    // 去掉可能的 `?v=…` 查询串，再取最后一段，也就是 sha256
    let name = local
        .split('?')
        .next()?
        .trim_end_matches('/')
        .rsplit('/')
        .next()?;

    URL_IMAGE.read().unwrap().get(name).cloned()
}
