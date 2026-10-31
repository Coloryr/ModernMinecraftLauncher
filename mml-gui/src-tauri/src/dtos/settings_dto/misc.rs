//! 设置窗口的零散 DTO：窗口尺寸 / 启动前检查 / 默认值聚合 / 背景图信息
//!
//! 从 `settings_dto/mod.rs` 拆出来的。

use serde::{Deserialize, Serialize};

use mml_config::config_obj::WindowSettingObj;

use super::launch::RunArgSettingDto;
use super::network::NetworkSettingDto;
use crate::dtos::JavaInfoDto;

/// 设置项的出厂默认值，供「恢复默认」使用
///
/// 与各 getter 返回同一批 DTO 形状，值取自 core 的 `Default` / `new()`：
/// 前端不再自己复制一套默认值，避免两处口径漂移。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDefaultsDto {
    /// 网络与下载，即 HttpObj / DnsObj / GameCheckObj 的默认值
    pub network: NetworkSettingDto,
    /// 启动参数，取自 `RunArgObj::new()`
    pub run: RunArgSettingDto,
    /// 游戏窗口，取自 `WindowSettingObj::new()`
    pub window: WindowSettingDto,
}

/// 批量资源映射结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ResourceMapResultDto {
    /// 处理的实例数
    pub instances: u32,
    /// 映射成功的文件数
    pub mapped: u32,
    /// 映射失败的文件数
    pub failed: u32,
    /// 实例文件不存在、禁用或缺少哈希而跳过的文件数
    pub skipped: u32,
}

/// 批量资源映射进度
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ResourceMapProgressDto {
    /// 实例总数
    pub instances: u32,
    /// 待处理条目总数
    pub total: u32,
    /// 已处理条目数
    pub done: u32,
}

/// 游戏窗口设置，前端 wire 为 camelCase，字段即 core `WindowSettingObj`
///
/// core 侧是 `Option` 字段，DTO 落到 `WindowSettingObj::new()` 同款默认值；
/// `title_delay` core 尚无既有默认值，取 3000ms。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowSettingDto {
    /// 是否启动全屏模式
    pub full_screen: bool,
    /// 窗口宽度，单位像素
    pub width: u16,
    /// 窗口高度，单位像素
    pub height: u16,
    /// 是否启用自定义标题
    pub edit_title: bool,
    /// 自定义游戏窗口标题
    pub game_title: String,
    /// 是否使用随机标题
    pub random_title: bool,
    /// 是否循环切换标题
    pub cycle_title: bool,
    /// 循环标题切换间隔，单位毫秒
    pub title_delay: u32,
}

/// 启动设置，前端 wire 为 camelCase
///
/// Java 列表只读，增删走 `settings_scan_java` / `settings_add_java` /
/// `settings_remove_java`，启动参数与窗口设置走 `settings_save_launch`。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchSettingDto {
    /// 已添加的 Java 运行时列表
    pub java_list: Vec<JavaInfoDto>,
    /// 启动参数，即 core `RunArgObj`
    pub run: RunArgSettingDto,
    /// 游戏窗口设置，即 core `WindowSettingObj`
    pub window: WindowSettingDto,
}

/// 背景图信息，前端 wire 为 camelCase，即 `settings_get_bg` 返回值
///
/// 源地址与显示参数来自 `gui_config.json`，图片是后端处理落盘的
/// `bg_image.png`，base64 dataURL。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BgInfoDto {
    /// 背景图来源，文件路径或网址
    pub source: String,
    /// 处理后的背景图，dataURL
    pub data_url: String,
    /// 不透明度，百分比
    pub opacity: u32,
    /// 模糊，单位 px
    pub blur: u32,
    /// 原始分辨率，百分比
    pub native_size: u32,
}

impl From<&WindowSettingObj> for WindowSettingDto {
    /// 由 core 配置构造前端 DTO，core 侧的 `None` 落到 `WindowSettingObj::new()` 同款默认值
    ///
    /// - `w`: `config.json` 里的游戏窗口设置，字段多为 `Option`
    ///
    /// # 返回值
    ///
    /// 前端可直接绑定的窗口设置
    fn from(w: &WindowSettingObj) -> Self {
        Self {
            full_screen: w.full_screen.unwrap_or(false),
            width: w.width.unwrap_or(1280),
            height: w.height.unwrap_or(720),
            edit_title: w.edit_title.unwrap_or(false),
            game_title: w.game_title.clone().unwrap_or_default(),
            random_title: w.random_title.unwrap_or(false),
            cycle_title: w.cycle_title.unwrap_or(false),
            title_delay: w.title_delay.unwrap_or(3000),
        }
    }
}

impl From<WindowSettingDto> for WindowSettingObj {
    /// 由前端 DTO 写回 core 配置，所有字段都写 `Some(...)`，不留 `None`
    ///
    /// - `d`: 前端保存上来的窗口设置
    ///
    /// # 返回值
    ///
    /// 待写入 `config.json` 的窗口设置
    fn from(d: WindowSettingDto) -> Self {
        Self {
            full_screen: Some(d.full_screen),
            width: Some(d.width),
            height: Some(d.height),
            game_title: Some(d.game_title),
            edit_title: Some(d.edit_title),
            random_title: Some(d.random_title),
            cycle_title: Some(d.cycle_title),
            title_delay: Some(d.title_delay),
        }
    }
}
