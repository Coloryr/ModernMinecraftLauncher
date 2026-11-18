//! 自定义界面 DTO：TS 命名，camelCase wire
//!
//! 三种自定义界面各自的开关都在 `gui_config.client`，产物在运行目录 `custom_ui/` 下：
//! `css/<id>.zip` 是自定义 CSS 包，`pages/<id>.zip` 是主题包。磁盘上不存派生状态，
//! CSS 地址、整页入口与版本号每次都由 `custom_ui_status` 现算。

use serde::{Deserialize, Serialize};

use super::ThemeKindDto;

/// 状态里的一条自定义 CSS，带协议地址
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomUiCssDto {
    /// 条目 id，对应 `custom_ui/css/<id>.zip`
    pub id: String,
    /// 显示名，优先取包根 theme.json 的 name，没有时用导入文件名
    pub name: String,
    /// 是否启用
    pub enabled: bool,
    /// 样式地址，已带破缓存参数；文件缺失时为空串
    pub url: String,
    /// 入口脚本地址，已带破缓存参数；包里没有 custom.js 时为空串
    pub script_url: String,
}

/// 状态里的一条主题包
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomUiPageDto {
    /// 条目 id，对应 `custom_ui/pages/<id>.zip`
    pub id: String,
    /// 显示名，优先取包根 theme.json 的 name
    pub name: String,
    /// 包内文件数，不含目录条目；文件缺失时为 0
    pub file_count: u32,
    /// 主题包的实际类型；没有清单时按导入入口推断
    pub kind: ThemeKindDto,
    /// 主题包版本，来自包根 theme.json；没有清单或没写 version 时为空串
    pub version: String,
    /// 是否由 theme.json 声明了类型
    pub declared: bool,
}

/// 自定义界面状态，`custom_ui_status` 与主题包导入命令的返回值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomUiInfoDto {
    /// 自定义 CSS 总开关，对应 client.custom_css_on
    pub css_on: bool,
    /// 已安装的自定义 CSS，顺序即注入顺序
    pub css: Vec<CustomUiCssDto>,
    /// 完全自定义界面总开关，对应 client.custom_page_on
    pub page_on: bool,
    /// 当前选中的主题包 id；空串表示没选
    pub page_id: String,
    /// 已导入的主题包
    pub pages: Vec<CustomUiPageDto>,
    /// 选中的主题包是否可用，即压缩包存在且能打开（有 index.html 或 custom.js）
    pub page_installed: bool,
    /// 整页包的实际类型；没有清单时按导入入口推断，对整页包就是 page
    pub kind: ThemeKindDto,
    /// 整页包的主题名，来自包根 theme.json；没有清单或没写 name 时为空串
    pub name: String,
    /// 整页包的主题版本，来自包根 theme.json；没有清单或没写 version 时为空串
    pub version: String,
    /// 整页包是否由 theme.json 声明了类型；为 false 时设置页提示「未声明类型」
    pub declared: bool,
    /// 入口页完整 URL，未导入或包里没有 index.html 时为空串；已带破缓存参数
    pub page_entry_url: String,
    /// 整页入口脚本 URL；包里没有 custom.js 时为空串
    pub page_script_url: String,
    /// 整页附加样式 URL；包里没有 custom.css 时为空串
    pub page_style_url: String,
    /// 整页包的内容版本号，取压缩包 mtime
    pub page_version: String,
    /// 包内文件数，不含目录条目
    pub page_file_count: u32,
}
