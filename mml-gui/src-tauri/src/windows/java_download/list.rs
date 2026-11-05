//! Java 运行时下载列表：按源查可用包，源有 Adoptium / Zulu / OpenJ9 / Foojay
//!
//! 从 `java_download/mod.rs` 拆出来：四个源各一套查询参数与解析，分支互相独立。
//! 命令带分组标记，值填原始组名 `java_download`，把组键钉回 `javaDownload`，见 AGENTS.md §4。
//!
//! 列表项的 uuid 由下载地址派生（UUIDv5）：同一份包重新拉列表还是同一个 id，
//! 正在安装的判断、重复点击的拦截、解压目录都靠它，不再依赖每次都会变的随机值。

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_net::adoptium_api;
use uuid::Uuid;

use crate::dtos::{JavaDownloadItemDto, JavaTypes};

use super::JAVA_ITEMS;
use super::zulu_arch;

/// 列表请求代次：网络慢的旧请求晚到时不能覆盖新请求写下的缓存
static LIST_SEQ: AtomicU64 = AtomicU64::new(0);

/// 获取按当前筛选条件匹配的 Java 包列表，即窗口下方的文本列表
///
/// - `source`: 搜索源，四个源各一套查询参数与解析
/// - `java_type`: 发行类型，取 `jdk` 或 `jre`
/// - `major`: 主版本号，如 `21`
/// - `system`: 目标系统，如 `windows` / `linux`
/// - `arch`: 目标架构，如 `x64` / `aarch64`
///
/// # 返回值
///
/// 匹配到的包列表，每项含 uuid / 显示名 / 版本 / 文件名 / 大小，大小单位为字节、未知为 0；
/// 成功时用本次结果替换 `JAVA_ITEMS`，网络失败时返回 `Err` 且保留上一次的缓存
#[gui_macros::ipc_group("java_download")]
#[gui_macros::command]
pub async fn java_download_get_list(
    source: JavaTypes,
    java_type: String,
    major: u32,
    system: String,
    arch: String,
) -> Result<Vec<JavaDownloadItemDto>, String> {
    let seq = LIST_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    let mut cache = HashMap::new();
    let mut out = Vec::new();

    match source {
        JavaTypes::Adoptium => {
            let list = adoptium_api::get_java_list(major, &system)
                .await
                .map_err(|err| err.to_string())?;

            for item in list {
                // image_type 还有 debugimage / sbom / staticlibs，只留 jdk / jre
                if item.binary.architecture != arch || item.binary.image_type != java_type {
                    continue;
                }
                let filename = item.binary.name;
                if !unpackable(&filename) {
                    continue;
                }
                let ext = extension_of(&filename);
                let url = item.binary.package.link;
                let name = display_name(
                    "Temurin",
                    &item.version.semver,
                    &item.binary.image_type,
                    &item.binary.os,
                    &item.binary.architecture,
                    &ext,
                    false,
                );
                let uuid = uuid_of(&url);
                cache.insert(
                    uuid,
                    FileItemObj {
                        name: name.clone(),
                        file: mml_downloader::get_download_path().join(&filename),
                        url,
                        hash: FileHash::Sha256(item.binary.package.checksum),
                        later: LaterRun::None,
                    },
                );

                out.push(JavaDownloadItemDto {
                    uuid: uuid.to_string(),
                    name,
                    java_version: item.version.semver,
                    filename,
                    size: item.binary.package.size.max(0) as u64,
                });
            }
        }
        JavaTypes::Zulu => {
            let list = mml_net::zulu_api::get_java_list()
                .await
                .map_err(|err| err.to_string())?;

            // latest 是"该平台 / 类型下的最新版"，旧补丁与 EA 版不进列表；
            // 列表按 zulu 版本倒序，新版本在前
            let mut list: Vec<_> = list
                .into_iter()
                .filter(|item| {
                    item.latest
                        && item.release_status == "ga"
                        && item.bundle_type == java_type
                        && item.java_version.first().copied() == Some(major as i32)
                        && item.os == system
                        && zulu_arch(&item.arch, &item.hw_bitness) == arch
                        && unpackable(&item.url)
                })
                .collect();
            list.sort_by(|a, b| b.zulu_version.cmp(&a.zulu_version));

            for item in list {
                let filename = file_name_of(&item.url).to_string();
                let ext = extension_of(&filename);
                let java_version = item
                    .java_version
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                let name = display_name(
                    "Zulu",
                    &java_version,
                    &item.bundle_type,
                    &item.os,
                    &arch,
                    &ext,
                    item.javafx,
                );
                let uuid = uuid_of(&item.url);
                cache.insert(
                    uuid,
                    FileItemObj {
                        name: name.clone(),
                        file: mml_downloader::get_download_path().join(&filename),
                        url: item.url,
                        hash: FileHash::Sha256(item.sha256_hash),
                        later: LaterRun::None,
                    },
                );

                out.push(JavaDownloadItemDto {
                    uuid: uuid.to_string(),
                    name,
                    java_version,
                    filename,
                    // Zulu 列表接口不带包大小
                    size: 0,
                });
            }
        }
        JavaTypes::OpenJ9 => {
            // openj9_api 内部持有 rquickjs 运行时，future 非 Send，
            // 不能直接跨 Tauri 的异步命令 await；丢到阻塞线程上用
            // block_on 单线程驱动；阻塞线程不是异步 worker，block_on 合法
            let obj = tauri::async_runtime::spawn_blocking(move || {
                tauri::async_runtime::block_on(mml_net::openj9_api::get_java_list())
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string())?;

            for item in obj.download {
                if item.version != major as i32 || item.os != system || item.arch != arch {
                    continue;
                }
                // 每个条目 jdk / jre 各一个直链，按所选类型取对应侧
                let jdk = item.jdk;
                let jre = item.jre;
                let side = if java_type == "jre" { jre } else { jdk };
                let url = side.opt1.download_link;
                if url.is_empty() || !unpackable(&url) {
                    continue;
                }
                let filename = file_name_of(&url).to_string();
                let ext = extension_of(&filename);
                let name = display_name(
                    "Semeru",
                    &item.name,
                    &java_type,
                    &item.os,
                    &item.arch,
                    &ext,
                    false,
                );
                let uuid = uuid_of(&url);
                cache.insert(
                    uuid,
                    FileItemObj {
                        name: name.clone(),
                        file: mml_downloader::get_download_path().join(&filename),
                        url,
                        hash: FileHash::Sha256(side.opt1.checksum),
                        later: LaterRun::None,
                    },
                );

                out.push(JavaDownloadItemDto {
                    uuid: uuid.to_string(),
                    name,
                    java_version: item.name,
                    filename,
                    // OpenJ9 列表不带包大小
                    size: 0,
                });
            }
        }
        JavaTypes::Foojay => {
            let mut list = mml_net::foojay_api::get_java_list(major, &system, &arch, &java_type)
                .await
                .map_err(|err| err.to_string())?;

            // API 返回顺序是乱的，按 OpenJDK 版本号倒序，新版本在前
            list.sort_by_key(|a| std::cmp::Reverse(version_key(&a.java_version)));

            for item in list {
                if !unpackable(&item.filename) {
                    continue;
                }
                let ext = extension_of(&item.filename);
                let name = display_name(
                    &cap(&item.distribution),
                    &item.distribution_version,
                    &item.package_type,
                    &item.operating_system,
                    &item.architecture,
                    &ext,
                    item.javafx_bundled,
                );
                let uuid = uuid_of(&item.links.pkg_download_redirect);
                cache.insert(
                    uuid,
                    FileItemObj {
                        name: name.clone(),
                        file: mml_downloader::get_download_path().join(&item.filename),
                        // 服务端 302 到发行方 CDN 直链的重定向地址
                        url: item.links.pkg_download_redirect,
                        // Foojay /packages 不带校验值
                        hash: FileHash::None,
                        later: LaterRun::None,
                    },
                );

                out.push(JavaDownloadItemDto {
                    uuid: uuid.to_string(),
                    name,
                    java_version: item.java_version,
                    filename: item.filename,
                    // API 对未知大小返回 -1，归零让前端不显示大小
                    size: item.size.max(0) as u64,
                });
            }
        }
    }

    // 只有最新一发能写缓存：旧请求晚到就丢掉，免得它把新列表的 uuid 换回去
    if seq == LIST_SEQ.load(Ordering::SeqCst) {
        let mut locker = JAVA_ITEMS.write().unwrap();
        locker.clear();
        locker.extend(cache);
    }

    Ok(out)
}

/// 从下载地址生成稳定的包 id
///
/// 同一份包每次拉列表都得到同一个 id，正在安装的判断与解压目录都靠它；
/// 换包（发行版 / 平台 / 格式不同）时地址不同，id 随之不同。
///
/// - `url`: 下载地址
///
/// # 返回值
///
/// 该地址对应的 UUIDv5
pub(super) fn uuid_of(url: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, url.as_bytes())
}

/// 拼列表显示名：发行版 + 版本 + 类型 / 平台 / 格式
///
/// - `dist`: 发行版名，如 `Temurin`
/// - `version`: 发行版版本号
/// - `java_type`: `jdk` / `jre`
/// - `os`: 目标系统
/// - `arch`: 目标架构
/// - `ext`: 包格式，如 `zip` / `tar.gz`
/// - `fx`: 是否内置 JavaFX
///
/// # 返回值
///
/// 形如 `Temurin 21.0.13+9 (jdk, windows x64, zip)`
pub(super) fn display_name(
    dist: &str,
    version: &str,
    java_type: &str,
    os: &str,
    arch: &str,
    ext: &str,
    fx: bool,
) -> String {
    let fx = if fx { ", FX" } else { "" };
    format!("{dist} {version} ({java_type}, {os} {arch}, {ext}{fx})")
}

/// 取包格式，保留 `tar.gz` / `tar.xz` 这样的一段
///
/// - `name`: 文件名或下载地址
///
/// # 返回值
///
/// 小写格式名；没有扩展名时为空串
pub(super) fn extension_of(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if let Some((_, tail)) = lower.rsplit_once(".tar.") {
        return format!("tar.{tail}");
    }
    lower
        .rsplit_once('.')
        .map(|(_, tail)| tail.to_string())
        .unwrap_or_default()
}

/// 发行版标识转显示名首字母大写
///
/// - `value`: 如 `temurin`
///
/// # 返回值
///
/// 如 `Temurin`
fn cap(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// 从下载地址取文件名，即最后一段路径
///
/// - `url`: 下载地址
///
/// # 返回值
///
/// 最后一段路径；地址里没有 `/` 时原样返回
pub(super) fn file_name_of(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or(url)
}

/// 只保留能被 `mml_jvms::unzip_java` 解包的压缩包
///
/// msi / exe / pkg / dmg 是安装器 / 磁盘镜像，deb / rpm / apk 是包管理器格式，
/// 都解不开，不应进列表
///
/// - `name`: 文件名或下载地址，只看扩展名且大小写不敏感
///
/// # 返回值
///
/// 可以被解包时为 `true`
pub(super) fn unpackable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !(lower.ends_with(".msi")
        || lower.ends_with(".exe")
        || lower.ends_with(".pkg")
        || lower.ends_with(".dmg")
        || lower.ends_with(".deb")
        || lower.ends_with(".rpm")
        || lower.ends_with(".apk")
        || lower.ends_with(".zst")
        || lower.ends_with(".sh"))
}

/// 版本号拆成数值段供逐段比较排序，如 "21.0.5+11" 拆成 [21, 0, 5, 11]
///
/// 非数值段跳过；字典序比较会把 "9" 排在 "21" 后面，数值逐段比较不会
///
/// - `version`: 版本号字符串，以 `.` / `+` / `_` 分段
///
/// # 返回值
///
/// 各数值段；解析不出的段直接丢掉，可能为空
pub(super) fn version_key(version: &str) -> Vec<u32> {
    version
        .split(['.', '+', '_'])
        .filter_map(|item| item.parse().ok())
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpackable_skips_installer_formats() {
        for name in [
            "a.msi", "a.exe", "a.pkg", "a.dmg", "a.deb", "a.rpm", "a.apk", "a.tar.zst", "a.sh",
        ] {
            assert!(!unpackable(name), "{name} 不该进列表");
        }
        for name in ["a.zip", "a.tar.gz", "a.tgz", "a.tar.xz", "a.7z", "a.tar", "A.ZIP"] {
            assert!(unpackable(name), "{name} 应该能解包");
        }
    }

    #[test]
    fn extension_keeps_compound_suffix() {
        assert_eq!(extension_of("jdk-21.tar.gz"), "tar.gz");
        assert_eq!(extension_of("jdk-21.tar.xz"), "tar.xz");
        assert_eq!(extension_of("jdk-21.zip"), "zip");
        assert_eq!(extension_of("jdk-21"), "");
    }

    #[test]
    fn uuid_is_stable_and_url_specific() {
        let a = uuid_of("https://example.com/a.zip");
        assert_eq!(a, uuid_of("https://example.com/a.zip"));
        assert_ne!(a, uuid_of("https://example.com/b.zip"));
    }
}
