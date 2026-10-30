//! OptiFine API
//!
//! 提供 OptiFine 高清修复的版本列表获取与下载地址解析。
//!
//! 官方源解析 HTML 页面拿版本列表和下载链接，用 `html_scan` 里的轻量扫描；
//! BMCLAPI 镜像源直接调 JSON 接口。两个源统一成 [`GetOptifineObj`]。

use std::{collections::HashSet, path::Path, sync::OnceLock};

use crate::html_scan::{attr, find_tag, text};
use itertools::Itertools;
use mml_base::serialize_tools;
use mml_config::config_obj::SourceLocal;
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorType};
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

/// 下载页里的一行
#[derive(Default)]
struct DownloadRow {
    /// 主下载链接
    download: Option<String>,
    /// 镜像下载链接
    mirror: Option<String>,
    /// 兼容的 Forge 版本原文
    forge: String,
    /// 发布日期原文
    date: String,
    /// 文件名原文
    file: String,
}

/// 取出下载页里全部 `<tr class="downloadLine">`
///
/// - `html`: 下载页原文
///
/// # 返回值
///
/// 按页面顺序排好的行；没写 class 的 `<tr>` 会被跳过
fn download_rows(html: &str) -> Vec<DownloadRow> {
    let mut rows = Vec::new();
    let mut from = 0;
    while let Some((start, end)) = find_tag(html, from, "tr") {
        let row_end = html[end..]
            .find("</tr>")
            .map(|pos| end + pos)
            .unwrap_or(html.len());
        let is_row = attr(&html[start..end], "class")
            .is_some_and(|class| class.split_whitespace().any(|item| item == "downloadLine"));
        if is_row {
            rows.push(parse_row(&html[end + 1..row_end]));
        }
        // 页面按版本分块，行可能套在外层表格的单元格里，所以不能跳过整段，只前进本标签
        from = end + 1;
    }
    rows
}

/// 解析一行里的五个单元格
///
/// - `body`: 行内、`<tr>` 之后的原文
fn parse_row(body: &str) -> DownloadRow {
    let mut row = DownloadRow::default();
    let mut from = 0;
    while let Some((start, end)) = find_tag(body, from, "td") {
        let cell_end = body[end..]
            .find("</td>")
            .map(|pos| end + pos)
            .unwrap_or(body.len());
        let cell = &body[end + 1..cell_end];
        match attr(&body[start..end], "class")
            .unwrap_or_default()
            .as_str()
        {
            "colDownload" => row.download = link(cell),
            "colMirror" => row.mirror = link(cell),
            "colForge" => row.forge = text(cell),
            "colDate" => row.date = text(cell),
            "colFile" => row.file = text(cell),
            _ => {}
        }
        from = cell_end + "</td>".len();
    }
    row
}

/// 取单元格里第一个 `<a>` 的 href
///
/// - `cell`: 单元格内、`<td>` 之后的原文
fn link(cell: &str) -> Option<String> {
    let (start, end) = find_tag(cell, 0, "a")?;
    attr(&cell[start..end], "href")
}

/// 下载页里真正的 JAR 链接
///
/// 页面把下载按钮包在多层表格里，但正文只有 `downloadx?f=` 开头的锚点才是文件
fn download_link(html: &str) -> Option<String> {
    let mut from = 0;
    while let Some((start, end)) = find_tag(html, from, "a") {
        if let Some(href) = attr(&html[start..end], "href") {
            if href.starts_with("downloadx?") {
                return Some(href);
            }
        }
        from = end + 1;
    }
    None
}
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
        for row in download_rows(&data) {
            let (Some(temp), Some(temp1)) = (row.download, row.mirror) else {
                continue;
            };
            if row.file.is_empty() {
                continue;
            }

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
            let date_parts: Vec<&str> = row.date.split('.').collect();
            let formatted_date = if date_parts.len() == 3 && date_parts[2].len() == 4 {
                format!("{}.{}.{}", date_parts[2], date_parts[1], date_parts[0])
            } else {
                row.date.clone()
            };

            let version = row.file.replace("OptiFine ", "").replace(" ", "_");

            list.push(GetOptifineObj {
                file_name,
                version,
                mc_version: mc_version.to_string(),
                forge: row.forge.trim().to_string(),
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
            let Some(href) = download_link(&data) else {
                return Ok(None);
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实下载页里的一行，属性统一用单引号
    const ROW: &str = "<tr class='downloadLine downloadLinePreview'> <td class='colFile'>OptiFine HD U K2 pre5</td> <td class='colDownload'><a href=\"http://adfoc.us/serve/sitelinks/?id=1&url=http://optifine.net/adloadx?f=preview_OptiFine_26.2_HD_U_K2_pre5.jar&x=bc68\">Download</a></td> <td class='colMirror'><a href=\"http://optifine.net/adloadx?f=preview_OptiFine_26.2_HD_U_K2_pre5.jar\">(Mirror)</a></td> <td class='colForge'>Forge N/A</td> <td class='colDate'>20.10.2026</td> </tr>";

    #[test]
    fn parses_download_row() {
        let rows = download_rows(ROW);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.file, "OptiFine HD U K2 pre5");
        assert_eq!(row.forge, "Forge N/A");
        assert_eq!(row.date, "20.10.2026");
        assert!(
            row.mirror
                .as_ref()
                .unwrap()
                .ends_with("preview_OptiFine_26.2_HD_U_K2_pre5.jar")
        );
    }

    #[test]
    fn skips_rows_without_download_line_class() {
        let html = "<tr><td>head</td></tr><tr class='downloadLine'><td class='colFile'>a</td></tr>";
        let rows = download_rows(html);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].file, "a");
    }

    #[test]
    fn finds_download_link() {
        let html = "<span id='Download'><a href='downloadx?f=x.jar&x=abc'>dl</a></span>";
        assert_eq!(
            download_link(html).as_deref(),
            Some("downloadx?f=x.jar&x=abc")
        );
    }
}
