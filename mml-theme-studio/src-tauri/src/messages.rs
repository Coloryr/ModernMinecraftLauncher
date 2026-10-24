//! 主进程写到输出面板、错误提示和原生对话框里的文案
//!
//! 界面文案另有 `lib/messages.mjs`（预览脚本也用它）。这里只维护 Rust 侧会
//! 产生的那些，同样同时维护中英两套，不把中文写死在调用处。

use serde::{Deserialize, Serialize};

/// 工具语言
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    /// 简体中文
    #[serde(rename = "zh-CN")]
    ZhCn,
    /// 英文
    #[serde(rename = "en-US")]
    EnUs,
}

impl Locale {
    /// 解析落盘 / 前端传来的语言标识
    ///
    /// - `value`: `zh-CN` 或 `en-US`
    ///
    /// # 返回值
    ///
    /// 无法识别时返回 `None`
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "zh-CN" => Some(Self::ZhCn),
            "en-US" => Some(Self::EnUs),
            _ => None,
        }
    }

    /// 落盘与前端使用的语言标识
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }
}

/// Rust 侧使用的文案条目
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKey {
    /// 窗口标题
    Title,
    /// 正在下载 demo
    Downloading,
    /// 正在准备预览
    Preparing,
    /// 预览已启动，参数为 `url`
    PreviewReady,
    /// 项目已创建，参数为 `path`
    Created,
    /// 依赖安装完成
    Installed,
    /// 构建完成，参数为 `path`
    Built,
    /// 文件已保存，参数为 `path`
    FileSaved,
    /// 上一个操作尚未结束
    Busy,
    /// 操作已取消
    Cancelled,
    /// 找不到外部工具，参数为 `tool`
    MissingTool,
    /// Node.js 版本过低，参数为 `version`
    NodeVersion,
    /// 项目名非法
    InvalidName,
    /// 目标已存在，参数为 `path`
    Exists,
    /// 仓库缺少 demo 或开发契约，参数为 `path`
    BadSource,
    /// 路径越出项目，参数为 `path`
    Outside,
    /// 不是可编辑的文本文件，参数为 `path`
    InvalidFile,
    /// 不是受支持的主题项目
    InvalidProject,
    /// 缺少开发契约
    MissingSdk,
    /// 命令执行失败，参数为 `command` / `code`
    CommandFailed,
    /// 构建没有产出 ZIP
    MissingBuild,
    /// 文件被外部修改，参数为 `path`
    Conflict,
    /// 预览启动失败
    PreviewFailed,
    /// 配色取值非法
    InvalidTheme,
    /// 选择项目父目录
    ChooseParent,
    /// 选择主题项目
    ChooseProject,
    /// 选择启动器源码仓库
    ChooseRepo,
    /// 有未保存修改
    ConfirmDiscard,
    /// 关闭会停止预览与任务
    ConfirmClose,
    /// 关闭按钮
    Close,
    /// 取消按钮
    Cancel,
}

/// 带文案键与占位符取值的失败原因
#[derive(Clone, Debug)]
pub struct StudioError {
    key: Option<MessageKey>,
    values: Vec<(&'static str, String)>,
    raw: Option<String>,
}

impl StudioError {
    /// 只有文案键的错误
    pub fn new(key: MessageKey) -> Self {
        Self {
            key: Some(key),
            values: Vec::new(),
            raw: None,
        }
    }

    /// 文案键加一个占位符取值
    pub fn value(key: MessageKey, name: &'static str, value: impl Into<String>) -> Self {
        Self {
            key: Some(key),
            values: vec![(name, value.into())],
            raw: None,
        }
    }

    /// 再补一个占位符取值
    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.values.push((name, value.into()));
        self
    }

    /// 原样透传的错误（IO 异常、子进程启动失败等）
    pub fn raw(message: impl Into<String>) -> Self {
        Self {
            key: None,
            values: Vec::new(),
            raw: Some(message.into()),
        }
    }

    /// 当前语言下的可见文案
    ///
    /// - `locale`: 工具当前语言
    ///
    /// # 返回值
    ///
    /// 已替换占位符的成品文案
    pub fn text(&self, locale: Locale) -> String {
        match (self.key, &self.raw) {
            (Some(key), _) => render(locale, key, &self.values),
            (None, Some(raw)) => raw.clone(),
            (None, None) => String::new(),
        }
    }
}

impl From<std::io::Error> for StudioError {
    fn from(error: std::io::Error) -> Self {
        Self::raw(error.to_string())
    }
}

impl From<tauri::Error> for StudioError {
    fn from(error: tauri::Error) -> Self {
        Self::raw(error.to_string())
    }
}

/// 按语言取文案并替换 `{name}` 占位符
///
/// - `locale`: 工具当前语言
/// - `key`: 文案条目
/// - `values`: 占位符名与取值
///
/// # 返回值
///
/// 成品文案
pub fn render(locale: Locale, key: MessageKey, values: &[(&'static str, String)]) -> String {
    let mut text = match (locale, key) {
        (Locale::ZhCn, MessageKey::Title) => "M²L 主题开发工具",
        (Locale::EnUs, MessageKey::Title) => "M²L Theme Studio",
        (Locale::ZhCn, MessageKey::Downloading) => "正在下载 demo 和同一版本的开发契约…",
        (Locale::EnUs, MessageKey::Downloading) => "Downloading the demo and matching development contracts…",
        (Locale::ZhCn, MessageKey::Preparing) => "正在准备预览…",
        (Locale::EnUs, MessageKey::Preparing) => "Preparing preview…",
        (Locale::ZhCn, MessageKey::PreviewReady) => "预览已启动：{url}",
        (Locale::EnUs, MessageKey::PreviewReady) => "Preview started: {url}",
        (Locale::ZhCn, MessageKey::Created) => "项目已创建：{path}",
        (Locale::EnUs, MessageKey::Created) => "Project created: {path}",
        (Locale::ZhCn, MessageKey::Installed) => "依赖安装完成",
        (Locale::EnUs, MessageKey::Installed) => "Dependencies installed",
        (Locale::ZhCn, MessageKey::Built) => "构建完成：{path}",
        (Locale::EnUs, MessageKey::Built) => "Build complete: {path}",
        (Locale::ZhCn, MessageKey::FileSaved) => "文件已保存：{path}",
        (Locale::EnUs, MessageKey::FileSaved) => "File saved: {path}",
        (Locale::ZhCn, MessageKey::Busy) => "上一个操作尚未结束",
        (Locale::EnUs, MessageKey::Busy) => "The previous operation is still running",
        (Locale::ZhCn, MessageKey::Cancelled) => "操作已取消",
        (Locale::EnUs, MessageKey::Cancelled) => "Operation cancelled",
        (Locale::ZhCn, MessageKey::MissingTool) => "找不到 {tool}。请安装 Node.js 24+ 与 Git，并重新打开工具",
        (Locale::EnUs, MessageKey::MissingTool) => "{tool} was not found. Install Node.js 24+ and Git, then restart the tool",
        (Locale::ZhCn, MessageKey::NodeVersion) => "需要 Node.js 24 或更新版本，当前版本：{version}",
        (Locale::EnUs, MessageKey::NodeVersion) => "Node.js 24 or newer is required. Current version: {version}",
        (Locale::ZhCn, MessageKey::InvalidName) => "项目名不能包含路径分隔符或 Windows 保留名称",
        (Locale::EnUs, MessageKey::InvalidName) => "Project names cannot contain path separators or Windows reserved names",
        (Locale::ZhCn, MessageKey::Exists) => "目标已存在，不能覆盖：{path}",
        (Locale::EnUs, MessageKey::Exists) => "The destination already exists and cannot be overwritten: {path}",
        (Locale::ZhCn, MessageKey::BadSource) => "所选仓库没有完整的 demo 或开发契约：{path}",
        (Locale::EnUs, MessageKey::BadSource) => "The repository does not contain the complete demo or development contracts: {path}",
        (Locale::ZhCn, MessageKey::Outside) => "路径必须位于当前项目内：{path}",
        (Locale::EnUs, MessageKey::Outside) => "Path must stay inside the current project: {path}",
        (Locale::ZhCn, MessageKey::InvalidFile) => "只支持编辑不超过 2 MiB 的文本文件：{path}",
        (Locale::EnUs, MessageKey::InvalidFile) => "Only text files up to 2 MiB can be edited: {path}",
        (Locale::ZhCn, MessageKey::InvalidProject) => "不是受支持的主题项目，需要 package.json 和 src/theme.json",
        (Locale::EnUs, MessageKey::InvalidProject) => "Unsupported theme project: package.json and src/theme.json are required",
        (Locale::ZhCn, MessageKey::MissingSdk) => "项目缺少开发契约，请先安装依赖或重新从 demo 创建",
        (Locale::EnUs, MessageKey::MissingSdk) => "Development contracts are missing. Install dependencies or create the project from a demo again",
        (Locale::ZhCn, MessageKey::CommandFailed) => "命令执行失败：{command}，退出码 {code}",
        (Locale::EnUs, MessageKey::CommandFailed) => "Command failed: {command}, exit code {code}",
        (Locale::ZhCn, MessageKey::MissingBuild) => "构建没有生成 ZIP，请查看输出日志",
        (Locale::EnUs, MessageKey::MissingBuild) => "The build did not produce a ZIP. Check the output log",
        (Locale::ZhCn, MessageKey::Conflict) => "文件已被外部修改，请重新打开后再保存：{path}",
        (Locale::EnUs, MessageKey::Conflict) => "File changed outside the editor. Reopen it before saving: {path}",
        (Locale::ZhCn, MessageKey::PreviewFailed) => "预览启动失败，请查看输出日志",
        (Locale::EnUs, MessageKey::PreviewFailed) => "Preview failed to start. Check the output log",
        (Locale::ZhCn, MessageKey::InvalidTheme) => "请选择亮色或暗色",
        (Locale::EnUs, MessageKey::InvalidTheme) => "Choose Light or Dark",
        (Locale::ZhCn, MessageKey::ChooseParent) => "选择项目的父目录",
        (Locale::EnUs, MessageKey::ChooseParent) => "Choose a parent folder for the project",
        (Locale::ZhCn, MessageKey::ChooseProject) => "选择主题项目",
        (Locale::EnUs, MessageKey::ChooseProject) => "Choose a theme project",
        (Locale::ZhCn, MessageKey::ChooseRepo) => "选择启动器源码仓库",
        (Locale::EnUs, MessageKey::ChooseRepo) => "Choose the launcher source repository",
        (Locale::ZhCn, MessageKey::ConfirmDiscard) => "有未保存修改。放弃这些修改并继续？",
        (Locale::EnUs, MessageKey::ConfirmDiscard) => "Discard unsaved changes and continue?",
        (Locale::ZhCn, MessageKey::ConfirmClose) => "关闭工具会停止预览和正在执行的任务。",
        (Locale::EnUs, MessageKey::ConfirmClose) => "Closing the tool stops the preview and any running tasks.",
        (Locale::ZhCn, MessageKey::Close) => "关闭",
        (Locale::EnUs, MessageKey::Close) => "Close",
        (Locale::ZhCn, MessageKey::Cancel) => "取消",
        (Locale::EnUs, MessageKey::Cancel) => "Cancel",
    }
    .to_string();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 中英两套都要有，且占位符能被替换
    #[test]
    fn renders_both_locales() {
        let zh = render(
            Locale::ZhCn,
            MessageKey::Created,
            &[("path", "D:/themes/a".to_string())],
        );
        let en = render(
            Locale::EnUs,
            MessageKey::Created,
            &[("path", "D:/themes/a".to_string())],
        );
        assert!(zh.contains("D:/themes/a"), "{zh}");
        assert!(en.contains("D:/themes/a"), "{en}");
        assert_ne!(zh, en);
        assert!(!zh.contains("{path}"));
    }

    /// 原样透传的错误不乱翻
    #[test]
    fn keeps_raw_errors() {
        let error = StudioError::raw("access denied");
        assert_eq!(error.text(Locale::ZhCn), "access denied");
    }
}
