//! 界面侧日志文案
//!
//! 界面自己打的日志也要跟界面语言走，但文案**不进内核的 i18n 表**：`mml_names::i18_items`
//! 是内核自己的条目，界面文案混进去，两边的归属都会变糊。这里自留一套：条目枚举 +
//! 中英两套文案，语言取 [`gui_config`](crate::gui_config) 的 `locale`。
//!
//! 用法：`mml_log::error(LogText::IconCacheSaveFailed { path, error }.text())`。

use mml_names::Lang;

use crate::gui_config;

/// 界面日志条目
pub enum LogText {
    /// 图标网址登记表落盘失败
    IconUrlsSaveFailed {
        /// 落盘的文件路径
        path: String,
        /// 失败原因
        error: String,
    },
    /// 图标 ETag 旁车文件落盘失败
    IconEtagSaveFailed {
        /// 落盘的文件路径
        path: String,
        /// 失败原因
        error: String,
    },
    /// 图标磁盘缓存落盘失败
    IconCacheSaveFailed {
        /// 落盘的文件路径
        path: String,
        /// 失败原因
        error: String,
    },
    /// 装配文件拖放路径桥失败
    FilePathBridgeFailed {
        /// WebView2 给的错误，已 Debug 展开
        error: String,
    },
    /// 拖放路径桥收到了消息却取不到文件路径
    FilePathBridgeNoPath {
        /// 窗口标签，用来看是哪个窗口出的问题
        label: String,
    },
    /// 建 WebView2 数据目录失败，退回默认位置
    WebviewDataDirFailed {
        /// 想用的数据目录
        dir: String,
        /// 失败原因
        error: String,
    },
    /// 系统材质没铺上，窗口退回不透明底色
    WindowEffectFailed {
        /// 窗口标签，用来看是哪个窗口出的问题
        label: String,
        /// 请求的材质，`auto` / `mica` / `acrylic` / `blur`
        mode: String,
        /// 失败原因，库给的原文
        error: String,
    },
    /// 取 WebView2 设置失败，快捷键没能关掉
    WebviewSettingsFailed,
    /// 关 DevTools 失败
    DevtoolsDisableFailed {
        /// 失败原因
        error: String,
    },
    /// 关浏览器快捷键失败
    AcceleratorKeysDisableFailed {
        /// 失败原因
        error: String,
    },
    /// 当前 WebView2 没有 Settings3，关不了浏览器快捷键
    AcceleratorKeysUnsupported,
    /// 加载器查询被中断后重试
    LoaderQueryRetry {
        /// 第几次重试
        attempt: usize,
        /// 游戏版本
        mc: String,
    },
    /// ColorMC 迁移任务失败
    ColormcMigrateFailed {
        /// 失败原因
        error: String,
    },
    /// 取运行中实例时主窗口模型还没起来
    MainWindowModelMissing,
    /// 已保存网络 / 代理设置
    NetworkSettingsSaved {
        /// 通用客户端的代理策略
        work_proxy: String,
        /// 通用客户端的代理类型
        work_proxy_type: String,
        /// 代理地址，两个客户端共用
        proxy_ip: String,
        /// 代理端口，两个客户端共用
        proxy_port: String,
        /// 登录客户端的代理策略
        login_proxy: String,
        /// 登录客户端的代理类型
        login_proxy_type: String,
    },
}

impl LogText {
    /// 按当前界面语言渲染
    ///
    /// # 返回值
    ///
    /// 当前界面语言的成品字符串
    pub fn text(&self) -> String {
        self.text_in(&gui_config::get().locale)
    }

    /// 按指定语言渲染
    ///
    /// - `lang`: 界面语言
    ///
    /// # 返回值
    ///
    /// 该语言的成品字符串
    pub fn text_in(&self, lang: &Lang) -> String {
        match lang {
            Lang::zh_cn => self.zh(),
            Lang::en_us => self.en(),
        }
    }

    /// 中文文案
    fn zh(&self) -> String {
        match self {
            LogText::IconUrlsSaveFailed { path, error } => {
                format!("[image] 图标网址登记表落盘失败 {path}：{error}")
            }
            LogText::IconEtagSaveFailed { path, error } => {
                format!("[image] ETag 落盘失败 {path}：{error}")
            }
            LogText::IconCacheSaveFailed { path, error } => {
                format!("[image] 图标磁盘缓存落盘失败 {path}：{error}")
            }
            LogText::FilePathBridgeFailed { error } => {
                format!("[drop] 装配文件路径桥失败（文件拖入将拿不到路径）: {error}")
            }
            LogText::FilePathBridgeNoPath { label } => {
                format!("[drop] 路径桥取不到路径（窗口 {label}）")
            }
            LogText::WebviewDataDirFailed { dir, error } => {
                format!("[window] 创建 webview 数据目录失败，退回默认位置：{dir} ({error})")
            }
            LogText::WindowEffectFailed { label, mode, error } => format!(
                "[window] 铺系统材质失败，窗口退回不透明底色：mode={mode} 窗口={label} ({error})"
            ),
            LogText::WebviewSettingsFailed => {
                String::from("[webview] 取 WebView2 设置失败，快捷键未能关闭")
            }
            LogText::DevtoolsDisableFailed { error } => {
                format!("[webview] 关闭 DevTools 失败: {error}")
            }
            LogText::AcceleratorKeysDisableFailed { error } => {
                format!("[webview] 关闭浏览器快捷键失败: {error}")
            }
            LogText::AcceleratorKeysUnsupported => {
                String::from("[webview] 当前 WebView2 不支持关闭浏览器快捷键（Settings3 缺失）")
            }
            LogText::LoaderQueryRetry { attempt, mc } => {
                format!("加载器查询被中断，用新客户端重试（第 {attempt} 次）：mc={mc}")
            }
            LogText::ColormcMigrateFailed { error } => {
                format!("[colormc] 迁移任务失败：{error}")
            }
            LogText::MainWindowModelMissing => {
                String::from("[main_get_running] 主窗口模型未初始化")
            }
            LogText::NetworkSettingsSaved {
                work_proxy,
                work_proxy_type,
                proxy_ip,
                proxy_port,
                login_proxy,
                login_proxy_type,
            } => format!(
                "保存网络设置：workProxy={work_proxy} ({work_proxy_type} {proxy_ip}:{proxy_port}) \
                 loginProxy={login_proxy} ({login_proxy_type} {proxy_ip}:{proxy_port})"
            ),
        }
    }

    /// 英文文案
    fn en(&self) -> String {
        match self {
            LogText::IconUrlsSaveFailed { path, error } => {
                format!("[image] failed to write the icon URL registry {path}: {error}")
            }
            LogText::IconEtagSaveFailed { path, error } => {
                format!("[image] failed to write the ETag sidecar {path}: {error}")
            }
            LogText::IconCacheSaveFailed { path, error } => {
                format!("[image] failed to write the icon disk cache {path}: {error}")
            }
            LogText::FilePathBridgeFailed { error } => format!(
                "[drop] failed to set up the file path bridge, \
                 dropped files will not carry paths: {error}"
            ),
            LogText::FilePathBridgeNoPath { label } => format!(
                "[drop] the path bridge returned no path (window {label})"
            ),
            LogText::WebviewDataDirFailed { dir, error } => format!(
                "[window] failed to create the webview data directory, \
                 falling back to the default: {dir} ({error})"
            ),
            LogText::WindowEffectFailed { label, mode, error } => format!(
                "[window] failed to apply the system backdrop, the window falls back to an opaque background: mode={mode} window={label} ({error})"
            ),
            LogText::WebviewSettingsFailed => String::from(
                "[webview] failed to get WebView2 settings, browser shortcuts stay enabled",
            ),
            LogText::DevtoolsDisableFailed { error } => {
                format!("[webview] failed to disable DevTools: {error}")
            }
            LogText::AcceleratorKeysDisableFailed { error } => {
                format!("[webview] failed to disable browser accelerator keys: {error}")
            }
            LogText::AcceleratorKeysUnsupported => String::from(
                "[webview] this WebView2 cannot disable browser accelerator keys \
                 (Settings3 missing)",
            ),
            LogText::LoaderQueryRetry { attempt, mc } => format!(
                "loader query aborted, retrying with a fresh client (attempt {attempt}): mc={mc}"
            ),
            LogText::ColormcMigrateFailed { error } => {
                format!("[colormc] migration task failed: {error}")
            }
            LogText::MainWindowModelMissing => {
                String::from("[main_get_running] main window model not initialized")
            }
            LogText::NetworkSettingsSaved {
                work_proxy,
                work_proxy_type,
                proxy_ip,
                proxy_port,
                login_proxy,
                login_proxy_type,
            } => format!(
                "saving network settings: workProxy={work_proxy} \
                 ({work_proxy_type} {proxy_ip}:{proxy_port}) loginProxy={login_proxy} \
                 ({login_proxy_type} {proxy_ip}:{proxy_port})"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use mml_names::Lang;

    use super::LogText;

    /// 两套文案都得有内容，参数要真的落到文案里
    #[test]
    fn both_languages_render() {
        let failed = LogText::ColormcMigrateFailed {
            error: String::from("boom"),
        };
        assert_eq!(
            failed.text_in(&Lang::zh_cn),
            String::from("[colormc] 迁移任务失败：boom")
        );
        assert_eq!(
            failed.text_in(&Lang::en_us),
            String::from("[colormc] migration task failed: boom")
        );

        let retry = LogText::LoaderQueryRetry {
            attempt: 2,
            mc: String::from("1.20.1"),
        };
        assert!(retry.text_in(&Lang::zh_cn).contains("第 2 次"));
        assert!(retry.text_in(&Lang::en_us).contains("attempt 2"));
        assert!(retry.text_in(&Lang::en_us).contains("mc=1.20.1"));

        let net = LogText::NetworkSettingsSaved {
            work_proxy: String::from("Auto"),
            work_proxy_type: String::from("http"),
            proxy_ip: String::from("127.0.0.1"),
            proxy_port: String::from("7890"),
            login_proxy: String::from("None"),
            login_proxy_type: String::from("http"),
        };
        assert!(net.text_in(&Lang::zh_cn).starts_with("保存网络设置："));
        assert!(
            net.text_in(&Lang::en_us)
                .starts_with("saving network settings:")
        );
        assert!(net.text_in(&Lang::en_us).contains("127.0.0.1:7890"));

        // 没有参数的条目也两边都有字
        assert!(
            !LogText::WebviewSettingsFailed
                .text_in(&Lang::zh_cn)
                .is_empty()
        );
        assert_ne!(
            LogText::MainWindowModelMissing.text_in(&Lang::zh_cn),
            LogText::MainWindowModelMissing.text_in(&Lang::en_us)
        );
    }
}
