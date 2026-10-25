//! 主题包的类型判定与导入期校验
//!
//! 三种换皮包（自定义主页面 / 自定义 CSS / 完全自定义界面）都是 zip，入口签名有重叠：
//! Home 与 Page 都是 `index.html` 或 `custom.js`，单看文件分不出来。所以先看包根的
//! `theme.json` 清单，没有清单再退回「导入入口」做兼容校验：样式包只收不带
//! `index.html` / `custom.js` 的纯样式包，入口与其它类型重合时要求补清单。
//!
//! 校验在替换旧包之前完成，分三层：
//! 1. 容器层：能否打开、加密条目、条目类型、条目数、解压后大小与压缩比；
//! 2. 路径层：绝对路径、`..`、控制字符、空路径与重名；
//! 3. 布局层：整包套一层目录的兜底、入口文件必须在包根、清单同根查找。
//!
//! 通过后调用方只做「先写临时文件、再 rename」的原子替换，坏包不会顶掉正在用的包。
//! 请求期的越界防护仍由 `web_assets::resolve_entry` 兜底，这里只保证坏包根本不入库。

mod archive;

use std::path::Path;

use serde::Deserialize;

use crate::err_keys;
use crate::log_text::LogText;
use crate::windows::web_assets::WebArchive;

/// 主题清单文件名，必须与入口文件同根
pub(crate) const MANIFEST_FILE: &str = "theme.json";
/// 入口页文件名，Home 与 Page 共用
pub(crate) const INDEX_FILE: &str = "index.html";
/// 可选的入口脚本名，Home / Page 的替代入口，CSS 包的附加入口
pub(crate) const SCRIPT_FILE: &str = "custom.js";
/// 自定义 CSS 的入口样式表名
pub(crate) const CSS_FILE: &str = "custom.css";

/// 清单 JSON 的大小上限，单位字节
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// 主题包类型，决定要哪些入口文件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// 自定义主页面：只替换主窗口主页内容区
    Home,
    /// 自定义 CSS：多份，按条目启用
    Css,
    /// 完全自定义界面：整页 / 按 kind 替换窗口
    Page,
}

impl ThemeKind {
    /// 解析清单里的 type 取值，未知取值返回 None
    ///
    /// - `value`: 清单里的原始字符串
    fn from_manifest(value: &str) -> Option<Self> {
        match value {
            "home" => Some(Self::Home),
            "css" => Some(Self::Css),
            "page" => Some(Self::Page),
            _ => None,
        }
    }

    /// 该类型允许的入口文件；命中任意一个即可导入
    fn entry_names(self) -> &'static [&'static str] {
        match self {
            Self::Home | Self::Page => &[INDEX_FILE, SCRIPT_FILE],
            Self::Css => &[CSS_FILE],
        }
    }
}

/// 一次类型判定的结果，导入命令与状态查询共用
///
/// declared 为 false 表示包里没有可用的清单，类型是按导入入口推断的，
/// 设置页据此提示「未声明类型」。
#[derive(Debug)]
pub struct ThemeInfo {
    /// 实际类型
    pub kind: ThemeKind,
    /// 主题名，来自清单；没有清单时为空串
    pub name: String,
    /// 主题版本，来自清单；没有清单时为空串
    pub version: String,
    /// 是否由清单明确声明了类型
    pub declared: bool,
}

impl ThemeInfo {
    /// 没有清单时的结果：类型按调用方给的入口推断，名字与版本都为空
    ///
    /// - `kind`: 导入入口对应的类型
    pub(crate) fn plain(kind: ThemeKind) -> Self {
        Self {
            kind,
            name: String::new(),
            version: String::new(),
            declared: false,
        }
    }
}

impl From<ThemeKind> for crate::dtos::ThemeKindDto {
    fn from(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Home => Self::Home,
            ThemeKind::Css => Self::Css,
            ThemeKind::Page => Self::Page,
        }
    }
}

/// 包根清单，字段都可缺省时给默认值
#[derive(Debug, Deserialize)]
struct Manifest {
    /// 类型，必填，取值见 ThemeKind::from_manifest
    #[serde(rename = "type")]
    kind: String,
    /// 主题名，可选，给设置页显示
    #[serde(default)]
    name: Option<String>,
    /// 主题版本，可选，给设置页显示
    #[serde(default)]
    version: Option<String>,
    /// 主题接口版本，可选，默认 1；目前只认 1
    #[serde(default)]
    api: Option<u32>,
}

/// 判定待导入包的类型并做导入期的三层校验
///
/// - `path`: 待导入的 zip 路径
/// - `expected`: 用户点的导入入口；None 表示没有上下文，只按包内容推断
///
/// # 返回值
///
/// 类型判定结果；zip 非法、路径越界、重名、入口缺失或与入口不匹配时返回对应错误键
pub fn detect_theme(path: &Path, expected: Option<ThemeKind>) -> Result<ThemeInfo, String> {
    let mut archive = archive::Package::open(path)?;

    if let Some(manifest) = read_manifest(&mut archive)? {
        let kind = ThemeKind::from_manifest(&manifest.kind)
            .ok_or_else(|| error(err_keys::THEME_MANIFEST_INVALID))?;
        // 先比类型：包明显不该走这个按钮时报不匹配，比报缺入口更好定位
        if expected.is_some_and(|expected| expected != kind) {
            return Err(error(err_keys::THEME_TYPE_MISMATCH));
        }
        ensure_entry(&archive, kind)?;
        return Ok(ThemeInfo {
            kind,
            name: manifest.name.unwrap_or_default(),
            version: manifest.version.unwrap_or_default(),
            declared: true,
        });
    }

    // 没有清单：类型取导入入口，只做入口文件匹配；Home 与 Page 仍然分不出来，
    // 样式包与它们重合时直接要求补清单
    let has_home = has_entry(&archive, INDEX_FILE) || has_entry(&archive, SCRIPT_FILE);
    let has_css = has_entry(&archive, CSS_FILE);
    let kind = match expected {
        Some(ThemeKind::Css) => {
            if !has_css {
                return Err(error(err_keys::THEME_MISSING_ENTRY));
            }
            // custom.js 与主页面 / 主题包重合：没有清单时只收纯样式包，
            // 免得整页包因为多带了一个 custom.css 就被当成样式包导入
            if has_home {
                return Err(error(err_keys::THEME_TYPE_UNKNOWN));
            }
            ThemeKind::Css
        }
        Some(ThemeKind::Home) => match (has_home, has_css) {
            (true, _) => ThemeKind::Home,
            // 只有 custom.css：包里其实是一份 CSS 主题
            (false, true) => return Err(error(err_keys::THEME_TYPE_MISMATCH)),
            (false, false) => return Err(error(err_keys::THEME_MISSING_ENTRY)),
        },
        Some(ThemeKind::Page) => match (has_home, has_css) {
            (true, _) => ThemeKind::Page,
            (false, true) => return Err(error(err_keys::THEME_TYPE_MISMATCH)),
            (false, false) => return Err(error(err_keys::THEME_MISSING_ENTRY)),
        },
        None => {
            // Home 与 Page 入口相同，没有清单时无法判定，不猜
            if has_home {
                return Err(error(err_keys::THEME_TYPE_UNKNOWN));
            }
            if has_css {
                ThemeKind::Css
            } else {
                return Err(error(err_keys::THEME_TYPE_UNKNOWN));
            }
        }
    };
    Ok(ThemeInfo::plain(kind))
}

/// 从已导入的常驻句柄里读回清单，供状态查询拼 DTO
///
/// 导入期已校验过，这里只解析不校验；清单缺失或损坏时按 fallback 处理，
/// 不能让一个旧包把整个状态查询变成错误。
///
/// - `archive`: 常驻包句柄
/// - `fallback`: 没有可用清单时按导入入口推断的类型
pub fn meta_of_archive(archive: &WebArchive, fallback: ThemeKind) -> ThemeInfo {
    if let Some(manifest) = manifest_of_archive(archive)
        && let Some(kind) = ThemeKind::from_manifest(&manifest.kind)
        && manifest.api.is_none_or(|api| api == 1)
    {
        return ThemeInfo {
            kind,
            name: manifest.name.unwrap_or_default(),
            version: manifest.version.unwrap_or_default(),
            declared: true,
        };
    }
    ThemeInfo::plain(fallback)
}

/// 读包根清单并解析；清单太大、不是合法 JSON 或 api 不认识时返回错误键
///
/// - `archive`: 已通过容器与路径校验的包
fn read_manifest(archive: &mut archive::Package) -> Result<Option<Manifest>, String> {
    let Some(found) = archive.locate(MANIFEST_FILE) else {
        return Ok(None);
    };
    if found.case_mismatch {
        warn_case(&found.name);
    }
    if found.size > MAX_MANIFEST_BYTES {
        return Err(error(err_keys::THEME_MANIFEST_INVALID));
    }
    let Some(bytes) = archive.read(found.index, MAX_MANIFEST_BYTES)? else {
        return Err(error(err_keys::THEME_MANIFEST_INVALID));
    };
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|_| error(err_keys::THEME_MANIFEST_INVALID))?;
    if manifest.api.is_some_and(|api| api != 1) {
        return Err(error(err_keys::THEME_MANIFEST_INVALID));
    }
    Ok(Some(manifest))
}

/// 包里有没有这个入口，大小写不一致时按小写匹配并记一条日志
///
/// - `archive`: 已通过容器与路径校验的包
/// - `name`: 约定的小写入口文件名
///
/// # 返回值
///
/// 命中时为 `true`
fn has_entry(archive: &archive::Package, name: &str) -> bool {
    match archive.locate(name) {
        Some(found) => {
            if found.case_mismatch {
                warn_case(&found.name);
            }
            true
        }
        None => false,
    }
}

/// 确认该类型至少有一个入口文件
///
/// - `archive`: 已通过容器与路径校验的包
/// - `kind`: 清单声明的类型
fn ensure_entry(archive: &archive::Package, kind: ThemeKind) -> Result<(), String> {
    if kind.entry_names().iter().any(|name| has_entry(archive, name)) {
        Ok(())
    } else {
        Err(error(err_keys::THEME_MISSING_ENTRY))
    }
}

/// 从常驻句柄里找清单：先看包根，再看唯一顶层目录
///
/// - `archive`: 常驻包句柄
fn manifest_of_archive(archive: &WebArchive) -> Option<Manifest> {
    let name = match archive.single_top_dir() {
        Some(dir) if archive.entry_name(MANIFEST_FILE).is_none() => {
            format!("{dir}/{MANIFEST_FILE}")
        }
        _ => String::from(MANIFEST_FILE),
    };
    let bytes = archive.read(&name)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// 入口文件名大小写与约定不一致时的提示，按小写匹配收下
///
/// - `name`: 包里的实际条目名
fn warn_case(name: &str) {
    mml_log::warn(
        LogText::ThemeEntryCaseMismatch {
            name: name.to_string(),
        }
        .text(),
    );
}

/// 错误键转成命令返回的字符串
///
/// - `key`: crate::err_keys 里的常量
fn error(key: &str) -> String {
    String::from(key)
}
