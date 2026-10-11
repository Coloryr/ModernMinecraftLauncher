//! Java 运行时下载列表：按源查可用包，源有 Adoptium / Zulu / OpenJ9 / Foojay
//!
//! 从 `java_download/mod.rs` 拆出来的，`java_download_get_list` 一个函数就有 205 行。
//! 列表查询是"每个源一套参数与解析"，四个分支各自独立；命令带分组标记，
//! 值填原始组名 `java_download`，把组键钉回 `javaDownload`，见 AGENTS.md §4。

use mml_base::file_item::{FileHash, FileItemObj, LaterRun};
use mml_net::adoptium_api;
use uuid::Uuid;

use crate::dtos::{JavaDownloadItemDto, JavaTypes};

use super::JAVA_ITEMS;
use super::zulu_arch;

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
/// 每次调用先清空 `JAVA_ITEMS` 再填入本次结果，网络失败时返回 `Err`
#[gui_macros::ipc_group("java_download")]
#[tauri::command]
pub async fn java_download_get_list(
    source: JavaTypes,
    java_type: String,
    major: u32,
    system: String,
    arch: String,
) -> Result<Vec<JavaDownloadItemDto>, String> {
    {
        let mut locker = JAVA_ITEMS.write().unwrap();
        locker.clear();
    }
    let mut out = Vec::new();
    match source {
        JavaTypes::Adoptium => {
            let list = adoptium_api::get_java_list(major, &system)
                .await
                .map_err(|err| err.to_string())?;

            let mut locker = JAVA_ITEMS.write().unwrap();

            for item in list {
                if item.binary.image_type == "debugimage" {
                    continue;
                }
                if item.binary.architecture != arch || item.binary.image_type != java_type {
                    continue;
                }
                let uuid = Uuid::new_v4();
                let name = format!("{}_{}", item.binary.scm_ref, item.binary.image_type);
                locker.insert(
                    uuid,
                    FileItemObj {
                        name: name.clone(),
                        file: mml_downloader::get_download_path().join(&item.binary.name),
                        url: item.binary.package.link,
                        hash: FileHash::Sha256(item.binary.package.checksum),
                        later: LaterRun::None,
                    },
                );

                out.push(JavaDownloadItemDto {
                    uuid: uuid.to_string(),
                    name,
                    java_version: item.version.semver,
                    filename: item.binary.name,
                    size: item.binary.package.size as u64,
                });
            }
        }
        JavaTypes::Zulu => {
            let list = mml_net::zulu_api::get_java_list()
                .await
                .map_err(|err| err.to_string())?;

            // 先过滤再按 zulu 版本倒序，新版本在前
            let mut list: Vec<_> = list
                .into_iter()
                .filter(|item| {
                    item.bundle_type == java_type
                        && item.java_version.first().copied() == Some(major as i32)
                        && item.os == system
                        && zulu_arch(&item.arch, &item.hw_bitness) == arch
                        && unpackable(&item.url)
                })
                .collect();
            list.sort_by(|a, b| b.zulu_version.cmp(&a.zulu_version));

            let mut locker = JAVA_ITEMS.write().unwrap();

            for item in list {
                let uuid = Uuid::new_v4();
                let name = format!(
                    "{}_{}",
                    item.zulu_version
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join("."),
                    item.bundle_type
                );
                // Zulu 列表没有文件名，从下载地址取最后一段
                let filename = file_name_of(&item.url).to_string();
                locker.insert(
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
                    java_version: item
                        .java_version
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join("."),
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

            let mut locker = JAVA_ITEMS.write().unwrap();

            for item in obj.download {
                if item.version != major as i32 || item.os != system || item.arch != arch {
                    continue;
                }
                // 每个条目 jdk / jre 各一个直链，按所选类型取对应侧
                let side = if java_type == "jre" {
                    item.jre
                } else {
                    item.jdk
                };
                if side.opt1.download_link.is_empty() {
                    continue;
                }
                let url = side.opt1.download_link;
                if !unpackable(&url) {
                    continue;
                }
                let uuid = Uuid::new_v4();
                let name = format!("{}_{}", item.name, java_type);
                let filename = file_name_of(&url).to_string();
                locker.insert(
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

            let mut locker = JAVA_ITEMS.write().unwrap();

            for item in list {
                if !unpackable(&item.filename) {
                    continue;
                }
                let uuid = Uuid::new_v4();
                let name = format!("{}_{}", item.distribution_version, item.package_type);
                locker.insert(
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

    Ok(out)
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
/// msi / exe / pkg / dmg 是安装器 / 磁盘镜像，无法解包注册，不应进列表
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
        || lower.ends_with(".dmg"))
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
