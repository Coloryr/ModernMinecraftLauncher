//! 游戏日志窗口 DTO：设置项与切换目标实例事件的负载，前端 wire，camelCase
//!
//! 切换目标实例（[`LogFocusDto`]）那条链路只剩导出窗口在用，即 `export-focus`：
//! 日志窗口的目标实例在开窗时就锁进窗口身份，不再需要事件切换。

use serde::{Deserialize, Serialize};

use crate::gui_setting::{GameGuiSettingObj, GameLogSettingObj};

/// 切换目标实例事件：导出窗口已存在时再次打开，壳层推送 `export-focus` 让已开窗口切换实例
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFocusDto {
    /// 目标实例 UUID
    pub uuid: String,
}

/// 日志窗口设置，即 `log_get_setting` 的返回值与 `log_set_setting` 的入参
///
/// 落盘在**实例**的 `gui_setting.json` 里，见 [`GameLogSettingObj`]，所以是"一个实例一套"。
/// `auto_show` 只由 `log_set_auto_show` 写：日志窗口整份回写设置时不碰它。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSettingDto {
    /// 自动换行
    pub word_wrap: bool,
    /// 自动下拉，跟随最新的日志行滚动
    pub auto: bool,
    /// 显示「其他」等级，即解析不出等级的普通行
    pub enable_none: bool,
    /// 显示信息级
    pub enable_info: bool,
    /// 显示警告级
    pub enable_warn: bool,
    /// 显示错误级
    pub enable_error: bool,
    /// 显示调试级
    pub enable_debug: bool,
    /// 启动游戏时自动打开该实例的日志窗口
    pub auto_show: bool,
}

impl From<&GameGuiSettingObj> for LogSettingDto {
    fn from(obj: &GameGuiSettingObj) -> Self {
        let log = &obj.log;
        Self {
            word_wrap: log.word_wrap,
            auto: log.auto,
            enable_none: log.enable_none,
            enable_info: log.enable_info,
            enable_warn: log.enable_warn,
            enable_error: log.enable_error,
            enable_debug: log.enable_debug,
            auto_show: obj.log_auto_show,
        }
    }
}

// 只映射 `Log` 段里的字段；`auto_show` 在 `GameGuiSettingObj::log_auto_show`，只由日志设置以外的命令写
impl From<LogSettingDto> for GameLogSettingObj {
    fn from(obj: LogSettingDto) -> Self {
        Self {
            word_wrap: obj.word_wrap,
            auto: obj.auto,
            enable_none: obj.enable_none,
            enable_info: obj.enable_info,
            enable_warn: obj.enable_warn,
            enable_error: obj.enable_error,
            enable_debug: obj.enable_debug,
        }
    }
}
