//! OptiFine API
//!
//! 提供 OptiFine 高清修复的版本列表获取与下载地址解析。
//!
//! 官方源解析 HTML 页面拿版本列表和下载链接，用 `scraper` crate；
//! BMCLAPI 镜像源直接调 JSON 接口。两个源统一成 [`GetOptifineObj`]。

use std::{collections::HashSet, path::Path, sync::OnceLock};

use itertools::Itertools;
use mml_base::serialize_tools;
use mml_config::config_obj::SourceLocal;
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::{
    url_helper::{self, get_source},
    urls,
};

/// BMCLAPI 返回的 OptiFine 版本条目
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct OptifineListObj {
    /// 对应的 Minecraft 版本
    pub mcversion: String,
    /// OptiFine 补丁版本
    pub patch: String,
    /// 版本类型，如 HD_U
    #[serde(rename = "type")]
    pub rtype: String,
    /// 下载文件名
    pub filename: String,
    /// 兼容的 Forge 版本
    pub forge: String,
}

/// 高清修复信息
#[derive(Clone)]
pub struct GetOptifineObj {
    /// 版本号
    pub version: String,
    /// 游戏版本号
    pub mc_version: String,
    /// 兼容的 Forge 版本，可为 unavailable / N/A
    pub forge: String,
    /// 文件名
    pub file_name: String,
    /// 发布日期，官方源已把页面的 `日.月.年` 统一成 `年.月.日`，镜像源为空串
    pub date: String,
    /// 下载地址，官方源为下载页，镜像源为 JAR 地址
    pub url1: Option<String>,
    /// 镜像下载地址，仅官方源有，用于二次解析
    pub url2: Option<String>,
    /// 下载源
    pub source: SourceLocal,
}

/// 已支持 OptiFine 的游戏版本缓存
static OPTIFINE_MC_VERSION: OnceLock<HashSet<String>> = OnceLock::new();

/// 获取高清修复版本
///
/// # 返回值
///
/// 返回所有可用的 OptiFine 版本；官方源解析不出条目时返回 `DataNotFound`
pub async fn get_optifine_version() -> CoreResult<Vec<GetOptifineObj>> {
    let url = url_helper::get_optifine_meta();
    let mut list = Vec::<GetOptifineObj>::new();

    let data = crate::get_work_client().get_text(&url).await?;
    if get_source() == SourceLocal::Offical {
        let html = Html::parse_document(&data);
        let select = Selector::parse("tr.downloadLine").unwrap();
        let rows: Vec<_> = html.select(&select).collect();

        for row in rows.iter() {
            let mut col_download = None;
            let mut col_mirror = None;
            let mut col_forge = None;
            let mut col_date = None;
            let mut col_file = None;

            let td_selector = Selector::parse("td").unwrap();

            for td in row.select(&td_selector) {
                if let Some(class) = td.value().attr("class") {
                    match class {
                        "colDownload" => {
                            if let Some(child) = td.children().next() {
                                if let Some(child_elem) = ElementRef::wrap(child) {
                                    col_download =
                                        child_elem.value().attr("href").map(String::from);
                                }
                            }
                        }
                        "colMirror" => {
                            if let Some(child) = td.children().next() {
                                if let Some(child_elem) = ElementRef::wrap(child) {
                                    col_mirror = child_elem.value().attr("href").map(String::from);
                                }
                            }
                        }
                        "colForge" => {
                            col_forge = Some(td.text().collect::<String>());
                        }
                        "colDate" => {
                            col_date = Some(td.text().collect::<String>());
                        }
                        "colFile" => {
                            col_file = Some(td.text().collect::<String>());
                        }
                        _ => {}
                    }
                }
            }

            let (temp, temp1, temp2, temp3, temp4) =
                match (col_download, col_mirror, col_forge, col_date, col_file) {
                    (Some(a), Some(b), Some(c), Some(d), Some(e)) => (a, b, c, d, e),
                    _ => continue,
                };

            // 镜像链接形如 `adloadx?f=<真实文件名>`，取路径末段后再剥掉前缀
            let file_name = Path::new(&temp1)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("")
                .replace("adloadx?f=", "");

            let mc_version = file_name
                .replace("preview_OptiFine_", "")
                .replace("OptiFine_", "");

            let mc_version = if let Some(idx) = mc_version.find('_') {
                &mc_version[..idx]
            } else {
                &mc_version
            };

            // 页面日期是 `日.月.年`，统一成 `年.月.日`
            let date_parts: Vec<&str> = temp3.split('.').collect();
            let formatted_date = if date_parts.len() == 3 && date_parts[2].len() == 4 {
                format!("{}.{}.{}", date_parts[2], date_parts[1], date_parts[0])
            } else {
                temp3
            };

            let version = temp4.replace("OptiFine ", "").replace(" ", "_");

            list.push(GetOptifineObj {
                file_name,
                version,
                mc_version: mc_version.to_string(),
                forge: temp2.trim().to_string(),
                date: formatted_date,
                url1: Some(temp),
                url2: Some(temp1),
                source: SourceLocal::Offical,
            });
        }

        if list.is_empty() {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Url));
        }
    } else {
        let mut obj = serialize_tools::json_from_str::<Vec<OptifineListObj>>(&data)
            .map_err(|err| crate::serialize_err_context::<Vec<OptifineListObj>>(err, &url))?;

        for item in obj.drain(..) {
            let url = url_helper::get_optifine_jar(&item);
            list.push(GetOptifineObj {
                version: format!("{}_{}", item.rtype, item.patch),
                mc_version: item.mcversion,
                forge: item.forge,
                file_name: item.filename,
                date: String::new(),
                url1: Some(url),
                url2: None,
                source: SourceLocal::Bmclapi,
            });
        }
    }

    Ok(list)
}

/// 获取 OptiFine 下载地址
///
/// - `source`: 下载源，官方源需二次解析，镜像源直接返回 `url1`
/// - `url1`: 一级地址，见 [`GetOptifineObj::url1`]
/// - `url2`: 二级地址，见 [`GetOptifineObj::url2`]
///
/// # 返回值
///
/// 返回 JAR 文件直接下载地址；官方源解析不到链接时返回 `None`
pub async fn get_optifine_download(
    source: &SourceLocal,
    url1: &Option<String>,
    url2: &Option<String>,
) -> CoreResult<Option<String>> {
    match source {
        SourceLocal::Offical => {
            crate::get_work_client()
                .get_text(&url1.as_ref().unwrap())
                .await?;

            let data = crate::get_work_client()
                .get_text(&url2.as_ref().unwrap())
                .await?;
            let html = Html::parse_document(&data);

            let selector =
                Selector::parse("table tr td table tbody tr td table tbody tr td span a").unwrap();

            let links: Vec<_> = html.select(&selector).collect();

            if links.is_empty() {
                return Ok(None);
            }

            let href = links[0].value().attr("href").unwrap();

            Ok(Some(format!("{}{href}", urls::OPTIFINE)))
        }
        SourceLocal::Bmclapi => Ok(url1.clone()),
    }
}

/// 获取支持的游戏版本
///
/// # 返回值
///
/// 返回支持 OptiFine 的 Minecraft 版本集合，首次查询后缓存
pub async fn get_support_version() -> CoreResult<Option<HashSet<String>>> {
    match OPTIFINE_MC_VERSION.get() {
        Some(data) => Ok(Some(data.clone())),
        None => {
            let list = get_optifine_version().await?;
            let list1 = list.iter().chunk_by(|item| &item.mc_version);

            let mut list2 = HashSet::<String>::new();
            for (key, _) in &list1 {
                list2.insert(key.clone());
            }

            Ok(Some(OPTIFINE_MC_VERSION.get_or_init(|| list2).clone()))
        }
    }
}
