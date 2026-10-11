use std::sync::OnceLock;

use itertools::Itertools;
use mml_base::serialize_tools;
use mml_names::i18_items::error_type::{CoreResult, DataNotFoundData, ErrorData, ErrorType};
use rquickjs::{AsyncContext, AsyncRuntime};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::urls;

/// Semeru 下载页接口返回的单条结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ResultsObj {
    /// 页面 HTML 正文，用于解析主版本 / 系统 / 架构下拉框
    pub content: String,
    /// 页面内嵌的 JS 脚本，含全部下载条目 `sourceDataJson`
    pub pagepost_custom_js_value: String,
}

impl Default for ResultsObj {
    fn default() -> Self {
        Self {
            content: Default::default(),
            pagepost_custom_js_value: Default::default(),
        }
    }
}

/// Semeru 下载页接口的响应
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct OpenJ9Obj {
    /// 接口是否报错，为 true 时 `results` 不可用
    pub error: bool,
    /// 页面结果列表，代码只取第一条
    pub results: Vec<ResultsObj>,
}

impl Default for OpenJ9Obj {
    fn default() -> Self {
        Self {
            error: Default::default(),
            results: Default::default(),
        }
    }
}

/// 单个下载选项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct OptObj {
    /// 下载直链
    #[serde(rename = "downloadLink")]
    pub download_link: String,
    /// 文件校验值，算法由页面给定
    pub checksum: String,
}

impl Default for OptObj {
    fn default() -> Self {
        Self {
            download_link: Default::default(),
            checksum: Default::default(),
        }
    }
}

/// 某一种包类型下的下载选项集合，字段名沿用页面里的 `opt1`
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct JdkObj {
    /// 页面中该包类型只给出 opt1 一个选项
    pub opt1: OptObj,
}

impl Default for JdkObj {
    fn default() -> Self {
        Self {
            opt1: Default::default(),
        }
    }
}

/// 单条 Java 下载条目
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct DownloadObj {
    /// 条目名称，页面原文以 `<br>` 分三段，初始化时改写成 `{第二段}+{第三段}`
    pub name: String,
    /// Java 主版本号
    pub version: i32,
    /// 目标操作系统
    pub os: String,
    /// 目标架构
    pub arch: String,
    /// JDK 包下载信息
    pub jdk: JdkObj,
    /// JRE 包下载信息
    pub jre: JdkObj,
}

impl Default for DownloadObj {
    fn default() -> Self {
        Self {
            name: Default::default(),
            version: Default::default(),
            os: Default::default(),
            arch: Default::default(),
            jdk: Default::default(),
            jre: Default::default(),
        }
    }
}

/// 页面 JS 里 `sourceDataJson` 的结构
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct OpenJ9FileObj {
    /// 全部下载条目
    pub downloads: Vec<DownloadObj>,
}

impl Default for OpenJ9FileObj {
    fn default() -> Self {
        Self {
            downloads: Default::default(),
        }
    }
}

/// 解析后的 IBM Semeru 可下载列表
#[derive(Debug, Clone)]
pub struct OpenJ9ListObj {
    /// 架构候选值，已剔除页面里的 any
    pub arch: Vec<String>,
    /// 操作系统候选值，已剔除页面里的 any
    pub os: Vec<String>,
    /// Java 主版本候选值
    pub main_version: Vec<String>,
    /// 全部下载条目
    pub download: Vec<DownloadObj>,
}

/// 解析结果缓存，首次查询后复用
static OPENJ9: OnceLock<OpenJ9ListObj> = OnceLock::new();

/// 抓取并解析 Semeru 下载页，结果写入 `OPENJ9` 缓存
async fn init() -> CoreResult<OpenJ9ListObj> {
    let data: OpenJ9Obj = crate::get_work_client().get_json(urls::OPENJ9).await?;
    let data = &data.results[0];

    let html = Html::parse_document(&data.content);
    let select = Selector::parse("select").unwrap();

    let mut obj = OpenJ9ListObj {
        arch: Default::default(),
        os: Default::default(),
        main_version: Default::default(),
        download: Default::default(),
    };

    {
        let mut list = html
            .select(&select)
            .filter(|item| item.value().attr("id").unwrap_or_default() == "runtimeVersion");

        let Some(item) = list.next() else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        let select1 = Selector::parse("option").unwrap();
        let node1 = item
            .select(&select1)
            .filter(|item| item.value().classes().contains("bx--select-option"));

        for item in node1 {
            obj.main_version
                .push(item.value().attr("value").unwrap_or_default().to_string());
        }
    }

    {
        let mut list = html
            .select(&select)
            .filter(|item| item.value().attr("id").unwrap_or_default() == "operatingSystem");

        let Some(item) = list.next() else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        let select1 = Selector::parse("option").unwrap();
        let node1 = item.select(&select1);

        for item in node1 {
            let os = item.value().attr("value").unwrap_or_default().to_string();
            if os.eq_ignore_ascii_case("any") {
                continue;
            }
            obj.os.push(os);
        }
    }

    {
        let mut list = html
            .select(&select)
            .filter(|item| item.value().attr("id").unwrap_or_default() == "arch");

        let Some(item) = list.next() else {
            return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
        };

        let select1 = Selector::parse("option").unwrap();
        let node1 = item.select(&select1);

        for item in node1 {
            let arch = item.value().attr("value").unwrap_or_default().to_string();
            if arch.eq_ignore_ascii_case("any") {
                continue;
            }
            obj.arch.push(arch);
        }
    }

    // 脚本里 `function` 之后是页面渲染逻辑，截掉后追加一句把 sourceDataJson 序列化出来
    let Some(temp) = data.pagepost_custom_js_value.find("function") else {
        return Err(ErrorType::DataNotFound(DataNotFoundData::Info));
    };

    let mut data = data.pagepost_custom_js_value[..temp].to_string();
    data.push_str(&mml_names::get_line_ending());
    data.push_str("JSON.stringify(sourceDataJson)");

    let rt = AsyncRuntime::new().map_err(|err| {
        ErrorType::TaskError(ErrorData {
            error: err.to_string(),
        })
    })?;
    let ctx = AsyncContext::full(&rt).await.map_err(|err| {
        ErrorType::TaskError(ErrorData {
            error: err.to_string(),
        })
    })?;

    let result: String = ctx.with(|ctx| ctx.eval(data).unwrap()).await;

    let files = serialize_tools::json_from_str::<OpenJ9FileObj>(&result)?;

    for mut item in files.downloads {
        let names: Vec<&str> = item.name.split("<br>").collect();
        item.name = if names.len() == 3 {
            format!("{}+{}", names[1], names[2])
        } else {
            item.name
        };
        obj.download.push(item);
    }

    Ok(OPENJ9.get_or_init(|| obj).clone())
}

/// 获取 IBM Semeru 的 Java 列表
///
/// # 返回值
///
/// 首次调用抓取并解析下载页，之后直接返回缓存
pub async fn get_java_list() -> CoreResult<OpenJ9ListObj> {
    match OPENJ9.get() {
        Some(data) => Ok(data.clone()),
        None => init().await,
    }
}
