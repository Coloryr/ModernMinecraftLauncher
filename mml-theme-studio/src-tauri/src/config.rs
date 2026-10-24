//! 工具自己的配置：studio.json（语言、最近项目、界面配色）
//!
//! 落盘键用项目约定的大驼峰；读取时兼容首版的小驼峰键。

use crate::messages::{Locale, StudioError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 工具界面配色
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    /// 暗色
    #[serde(rename = "Dark")]
    Dark,
    /// 亮色
    #[serde(rename = "Light")]
    Light,
}

impl Theme {
    /// 解析配置 / 前端传来的取值
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Dark" => Some(Self::Dark),
            "Light" => Some(Self::Light),
            _ => None,
        }
    }

    /// 落盘与前端使用的取值
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    /// 对应 Tauri 的原生窗口主题
    pub fn native(self) -> tauri::Theme {
        match self {
            Self::Dark => tauri::Theme::Dark,
            Self::Light => tauri::Theme::Light,
        }
    }

    /// 窗口底色，避免加载前端前闪一下白底
    pub fn color(self) -> tauri::window::Color {
        match self {
            Self::Dark => tauri::window::Color(20, 22, 26, 255),
            Self::Light => tauri::window::Color(238, 241, 245, 255),
        }
    }
}

/// 工具配置
#[derive(Clone, Debug)]
pub struct Config {
    /// 界面语言
    pub locale: Locale,
    /// 最近打开的项目，最多 8 条
    pub recent: Vec<String>,
    /// 界面配色
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locale: Locale::ZhCn,
            recent: Vec::new(),
            theme: Theme::Dark,
        }
    }
}

/// 读取配置；文件不存在或内容非法时全部用默认值
///
/// - `path`: studio.json 路径
pub fn load(path: &Path) -> Config {
    let mut config = Config::default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return config;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return config;
    };
    let field = |name: &str, legacy: &str| value.get(name).or_else(|| value.get(legacy));
    if let Some(locale) = field("Locale", "locale")
        .and_then(|v| v.as_str())
        .and_then(Locale::parse)
    {
        config.locale = locale;
    }
    if let Some(theme) = field("Theme", "theme")
        .and_then(|v| v.as_str())
        .and_then(Theme::parse)
    {
        config.theme = theme;
    }
    if let Some(recent) = field("Recent", "recent").and_then(|v| v.as_array()) {
        config.recent = recent
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::to_string)
            .take(8)
            .collect();
    }
    config
}

/// 写回配置，UTF-8 无 BOM
///
/// - `path`: studio.json 路径
/// - `config`: 要写入的配置
pub async fn save(path: &Path, config: &Config) -> Result<(), StudioError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let value = serde_json::json!({
        "Locale": config.locale.as_str(),
        "Recent": config.recent,
        "Theme": config.theme.as_str(),
    });
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| StudioError::raw(error.to_string()))?;
    tokio::fs::write(path, text + "\n").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 首版的小驼峰键仍然能读，非法取值回落到默认
    #[test]
    fn reads_legacy_keys() {
        let dir = mml_testutil::temp_path(format!("theme-studio-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("studio.json");
        std::fs::write(
            &path,
            r#"{"locale":"en-US","recent":["D:/a","D:/b"],"theme":"Light"}"#,
        )
        .unwrap();
        let config = load(&path);
        assert_eq!(config.locale, Locale::EnUs);
        assert_eq!(config.theme, Theme::Light);
        assert_eq!(config.recent, vec!["D:/a".to_string(), "D:/b".to_string()]);

        std::fs::write(&path, r#"{"Locale":"xx","Theme":"invalid"}"#).unwrap();
        let config = load(&path);
        assert_eq!(config.locale, Locale::ZhCn);
        assert_eq!(config.theme, Theme::Dark);
    }
}
