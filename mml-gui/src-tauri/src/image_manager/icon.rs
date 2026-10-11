//! 远程图标 `icon/<sha256>`：内存缓存 + 磁盘缓存 + ETag 校验 + 网址登记表
//!
//! 从 `image_manager/mod.rs` 拆出来的。三级缓存：内存 `ICON_IMAGE`，带 TTL →
//! 磁盘 `<图标缓存目录>/<sha256>`，落盘前统一转 PNG → 回源，带 `If-None-Match`。
//! 网址登记表 `urls.json` 把 `sha256 → 远程网址` 存下来，否则重启后回不了源。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use mml_base::hash_helper::{self, HashType};
use mml_net::get_work_client;
use mml_sys::path_helper;
use tauri::UriSchemeResponder;

use super::respond::{IconBytes, normalize_icon, send_bad, send_icon};
use super::{ICON_DIR, ICON_IMAGE, ICON_TIMEOUT, IconCache, URL_IMAGE};

/// 远程图标，URI 形如 `icon/<请求名>`：内存 → 磁盘 + ETag 校验 → 网络
///
/// 内存未命中时读磁盘缓存，并带记录的 ETag 发一次条件请求回源校验：
/// 服务器 304 图没变就不拉图片体，直接用硬盘缓存；200 才重新拉图更新本地；
/// 网络失败时用本地副本兜底
///
/// - `uri`: 协议路径片段，首段 `icon` 已剥，第 2 段是网址的 sha256
/// - `res`: 响应回写句柄
pub(super) async fn load_icon_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 2 {
        send_bad(res);
        return;
    }

    let name = uri[1];

    let url = URL_IMAGE.read().unwrap().get(name).cloned();
    let Some(url) = url else {
        // 映射里没有 ≠ 图片没有：`sha → 远程网址` 只在内存里，重启后就空了，
        // 但图片字节是按 sha256 落在磁盘缓存上的，所以先按 name 查盘再说
        let file = icon_file_by_name(name);
        match read_icon_bytes(&file) {
            Some(icon) => {
                // 顺手写回内存缓存：这条路径是重启后收藏夹里的老地址，本来每次刷新
                // 都要读盘 + 归一化一遍，不缓存等于每次全量解一次
                set_icon_image(name, icon.data.clone(), icon.mime);
                send_icon(res, &icon);
            }
            None => send_bad(res),
        }
        return;
    };

    // 内存缓存
    if let Some(icon) = get_icon_image(name) {
        send_icon(
            res,
            &IconBytes {
                data: icon.data,
                mime: icon.mime,
            },
        );
        return;
    }

    let file = icon_file(&url);
    let etag_file = icon_etag_file(&url);

    // 磁盘缓存的本地副本，供 304 续用 / 网络失败兜底
    let cached = read_icon_bytes(&file);
    if cached.is_none() && file.is_file() {
        // 内容损坏：删掉重新下载
        let _ = path_helper::delete(&file);
    }

    // 有磁盘缓存就带 ETag 去校验，两家 CDN 的 ETag 都是图片内容的 MD5，
    // 没有就直接下载——每次最多一个请求，图没变时 304 只回头部不带体
    let etag = read_icon_etag(&etag_file);
    match get_work_client()
        .get_bytes_with_check(&url, etag.as_deref())
        .await
    {
        Ok(check) if check.not_modified => {
            if let Some(icon) = &cached {
                set_icon_image(name, icon.data.clone(), icon.mime);
                send_icon(res, icon);
            } else {
                // 只有 ETag 没有图片属于异常状态，下次不带 ETag 重新下载
                let _ = path_helper::delete(&etag_file);
                send_bad(res);
            }
        }
        Ok(check) => {
            let Some(icon) = normalize_icon(&check.data) else {
                send_bad(res);
                return;
            };

            if let Some(etag) = check.etag {
                write_icon_etag(&etag_file, &etag);
            }
            write_icon_file(&file, &icon.data);
            set_icon_image(name, icon.data.clone(), icon.mime);

            send_icon(res, &icon);
        }
        Err(_) => {
            // 网络失败：有本地副本就先用着
            if let Some(icon) = &cached {
                set_icon_image(name, icon.data.clone(), icon.mime);
                send_icon(res, icon);
            } else {
                send_bad(res);
            }
        }
    }
}

/// 取图标内存缓存，已过期的条目会被删除
///
/// - `info`: 图标请求名，即网址的 sha256
///
/// # 返回值
///
/// 命中且未过期返回缓存条目，否则 `None`，由调用方回源
pub(super) fn get_icon_image(info: &str) -> Option<IconCache> {
    let mut lock = ICON_IMAGE.write().unwrap();
    let alive = lock
        .get(info)
        .is_some_and(|item| item.time.elapsed() < ICON_TIMEOUT);

    if alive {
        return lock.get(info).cloned();
    }

    lock.remove(info);

    None
}

/// 写入图标内存缓存并记录时刻
///
/// - `info`: 图标请求名，即网址的 sha256
/// - `data`: 图片字节，已归一化为 PNG
/// - `mime`: 内容类型，与 `data` 匹配
pub(super) fn set_icon_image(info: &str, data: Vec<u8>, mime: &'static str) {
    ICON_IMAGE.write().unwrap().insert(
        info.to_string(),
        IconCache {
            data,
            mime,
            time: Instant::now(),
        },
    );
}

/// 网址登记表的落盘位置，图标缓存目录还没初始化时返回 `None`
///
/// # 返回值
///
/// `<图标缓存目录>/urls.json`
pub(super) fn url_image_file() -> Option<PathBuf> {
    ICON_DIR.get().map(|dir| dir.join(ICON_URLS_FILE))
}

/// 把「sha256 → 远程网址」登记表写到磁盘
pub(super) fn save_url_image() {
    let Some(file) = url_image_file() else {
        return;
    };
    let Ok(json) = serde_json::to_vec(&*URL_IMAGE.read().unwrap()) else {
        return;
    };
    if let Err(err) = path_helper::write_bytes(&file, &json) {
        // 落盘失败的表现是"重启后收藏夹里的图标回不了源、只能吃磁盘缓存"——
        // 属于原因不明显的问题，必须留一条日志，见 AGENTS.md §13
        mml_log::error(format!(
            "[image] 图标网址登记表落盘失败 {}：{err}",
            file.display()
        ));
    }
}

/// 启动时读回登记表；首次运行没有这个文件，忽略即可
pub(super) fn load_url_image() {
    let Some(file) = url_image_file() else {
        return;
    };
    let Ok(data) = path_helper::read_byte(&file) else {
        return;
    };
    let Ok(map) = serde_json::from_slice::<HashMap<String, String>>(&data) else {
        return;
    };

    URL_IMAGE.write().unwrap().extend(map);
}

/// 图标请求名：网址的 sha256，十六进制小写，与旧启动器的缓存命名一致
///
/// 同一个网址每次都得到同一个名字，前端拿到的地址因此是稳定的，
/// 浏览器缓存与磁盘缓存都能直接命中。
///
/// - `url`: 远程图片网址
///
/// # 返回值
///
/// 64 位十六进制字符串
pub(super) fn icon_name(url: &str) -> String {
    hash_helper::gen_hash_from_string(HashType::Sha256, url)
}

/// 远程图片的磁盘缓存位置
///
/// - `url`: 远程图片网址
///
/// # 返回值
///
/// `<网址 sha256>.png` 的完整路径
pub(super) fn icon_file(url: &str) -> PathBuf {
    icon_file_by_name(&icon_name(url))
}

/// 已知 sha256 名字时的磁盘缓存位置
///
/// 内存里的「sha → 远程网址」映射是重启就没的，而缓存文件就叫这个名字，
/// 所以拿到 name 就能直接查盘，不必先知道远程网址。
///
/// - `name`: 图标请求名，即网址的 sha256
///
/// # 返回值
///
/// `<name>.png` 的完整路径
pub(super) fn icon_file_by_name(name: &str) -> PathBuf {
    ICON_DIR.get().unwrap().join(format!("{name}.png"))
}

/// 图标的 ETag 旁车文件
///
/// - `url`: 远程图片网址
///
/// # 返回值
///
/// `<网址 sha256>.etag` 的完整路径
pub(super) fn icon_etag_file(url: &str) -> PathBuf {
    ICON_DIR
        .get()
        .unwrap()
        .join(format!("{}.etag", icon_name(url)))
}

/// 读取记录的 ETag
///
/// - `file`: 旁车文件路径
///
/// # 返回值
///
/// 文件不存在或内容为空返回 `None`
pub(super) fn read_icon_etag(file: &Path) -> Option<String> {
    let text = fs::read_to_string(file).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// 记录 ETag，供下次回源时做 If-None-Match 校验
///
/// - `file`: 旁车文件路径
/// - `etag`: 服务器给的 ETag
pub(super) fn write_icon_etag(file: &Path, etag: &str) {
    if let Err(err) = fs::write(file, etag) {
        // 写不上 ETag 的表现是"每次都全量回源下载图标"，同样需要一条线索
        mml_log::error(format!("[image] ETag 落盘失败 {}：{err}", file.display()));
    }
}

/// 读取磁盘缓存的图标内容，不管新鲜度，有效性由 ETag 校验
///
/// - `file`: 缓存文件路径
///
/// # 返回值
///
/// 读不到或归一化失败返回 `None`
pub(super) fn read_icon_bytes(file: &Path) -> Option<IconBytes> {
    if !file.is_file() {
        return None;
    }

    path_helper::read_byte(file)
        .ok()
        .and_then(|data| normalize_icon(&data))
}

/// 写入磁盘缓存的图标
///
/// - `file`: 缓存文件路径
/// - `data`: 已归一化的图片字节
pub(super) fn write_icon_file(file: &Path, data: &[u8]) {
    if let Err(err) = path_helper::write_bytes(file, data) {
        // 磁盘缓存写不上 = 每次启动都要重新下载图标，留日志便于排查
        mml_log::error(format!(
            "[image] 图标磁盘缓存落盘失败 {}：{err}",
            file.display()
        ));
    }
}

/// 登记表落盘文件，放在图标缓存目录下
pub(super) const ICON_URLS_FILE: &str = "urls.json";
