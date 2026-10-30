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
///
/// 中文与英文两个 match 各有一个分支；加变体时两边一起补，
/// 参数一律作为字段传进来，不在调用处拼字符串。
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
    /// 设置窗口清理本地文件时删不掉某一项
    LocalFileClearFailed {
        /// 删不掉的路径
        path: String,
        /// 失败原因
        error: String,
    },
    /// 系统托盘建不起来，后台将没有托盘可用
    TrayInitFailed {
        /// 失败原因
        error: String,
    },
    /// 托盘显示或收起失败
    TrayVisibleFailed {
        /// true 为显示，false 为收起
        visible: bool,
        /// 失败原因
        error: String,
    },
    /// 从托盘唤回主窗口失败
    TraySummonFailed {
        /// 失败原因
        error: String,
    },
    /// 最后一个实例退出且主界面已关闭，启动器随之退出
    QuitAfterLastGame,
    /// IPC 服务没起来，单开与唤回失效
    IpcStartFailed {
        /// 失败原因
        error: String,
    },
    /// IPC 端口没记下来，下一次启动找不到本进程
    IpcPortSaveFailed {
        /// 端口文件路径
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
        /// 请求的材质，`mica` / `acrylic` / `blur`
        mode: String,
        /// 失败原因，库给的原文
        error: String,
    },
    /// 装饰激活失败，窗口退回无边框、按钮由界面自绘
    DecorationActivateFailed {
        /// 窗口标签，用来看是哪个窗口出的问题
        label: String,
        /// 失败原因，插件给的原文
        error: String,
    },
    /// macOS 原生红黄绿没摆到位，按钮留在默认位置
    TrafficLightsFailed {
        /// 失败原因，插件给的原文
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
    /// 游戏启动用的 tokio 运行时建不起来
    LaunchRuntimeFailed {
        /// 失败原因，运行时构建报的错
        error: String,
    },
    /// 自动标记模组时读文件算哈希失败
    AutoMarkReadFailed {
        /// 读失败的文件路径
        path: String,
        /// 失败原因
        error: String,
    },
    /// 自动标记模组时问 Modrinth 失败
    AutoMarkQueryFailed {
        /// 失败原因
        error: String,
    },
    /// 模组更新检查里有查询失败
    ModUpdateCheckFailed {
        /// 查询失败的条目数
        failed: usize,
        /// 第一条失败的原因
        error: String,
    },
    /// 模组更新任务异常结束
    ModUpdateAborted {
        /// 结束原因，panic 内容或 JoinError 的 Display
        reason: String,
    },
    /// 模组更新开始
    ModUpdateStarted {
        /// 这一批的条目总数
        total: usize,
    },
    /// 模组更新取 mods 目录失败
    ModUpdateModsDirFailed {
        /// 失败原因
        error: String,
    },
    /// 模组更新里单个条目失败
    ModUpdateItemFailed {
        /// 失败条目的项目号
        project_id: String,
        /// 失败原因
        error: String,
    },
    /// 模组更新的准备步骤异常
    ModUpdatePrepareFailed {
        /// 失败原因
        error: String,
    },
    /// 模组更新里可下载的条目数
    ModUpdateDownloadable {
        /// 这一刻可下载的条目数
        count: usize,
        /// 这一批的条目总数
        total: usize,
    },
    /// 模组更新开始下载
    ModUpdateDownloadStarted {
        /// 要下载的文件数
        count: usize,
    },
    /// 模组更新的下载阶段结束
    ModUpdateDownloadFinished {
        /// 下载结果，下载器给的 Debug 原文
        outcome: String,
    },
    /// 模组更新下载下来的文件校验不过
    ModUpdateHashMismatch {
        /// 校验不过的条目项目号
        project_id: String,
    },
    /// 模组更新收尾完成
    ModUpdateFinished {
        /// 换文件成功的条目数
        ok: u32,
        /// 失败的条目数
        failed: u32,
    },
    /// 日志窗口：运行库路径那一行
    LogRuntimeLib {
        /// 运行库文件路径
        path: String,
    },
    /// 日志窗口：Java 输出已重定向
    LogJavaRedirect,
    /// 日志窗口：Java 改回本地查找
    LogJavaLocalRedirect,
    /// 日志窗口：登录耗时
    LogLoginTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：服务器包检查耗时
    LogServerPackCheckTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：检查游戏文件耗时
    LogCheckGameFileTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：下载文件耗时
    LogDownloadFileTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：启动耗时
    LogLaunchTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：启动前命令耗时
    LogCmdPreTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：启动后命令耗时
    LogCmdPostTime {
        /// 耗时，单位秒
        secs: f64,
    },
    /// 日志窗口：实际用的 Java 路径
    LogJavaPath {
        /// Java 可执行文件路径
        path: String,
    },
    /// 主窗口：游戏正在启动
    LaunchStarting,
    /// 主窗口：游戏进程已经起来
    LaunchProcessStarted,
    /// 主窗口：启动线程 panic，已按失败收尾
    LaunchPanicked {
        /// panic 内容；取不到时为空串
        reason: String,
    },
    /// 主窗口：找不到可用的 Java
    LaunchNoJava {
        /// 主版本号
        java: i32,
    },
    /// 自定义主页压缩包里没有入口文件
    ArchiveIndexMissing {
        /// 入口文件名
        name: String,
    },
    /// 自定义主页压缩包打不开
    ArchiveOpenFailed {
        /// 失败原因
        error: String,
    },
    /// 自定义界面压缩包里没有入口文件
    CustomUiArchiveMissing {
        /// 入口文件名
        name: String,
    },
    /// 自定义界面压缩包打不开
    CustomUiArchiveOpenFailed {
        /// 失败原因
        error: String,
    },
    /// 主题包入口文件名大小写与约定不一致，按小写匹配收下
    ThemeEntryCaseMismatch {
        /// 包里的实际条目名
        name: String,
    },
    /// panic 载荷既不是字符串也不是静态串
    UnknownPanic,
    /// 批量取 CurseForge 文件信息失败
    FetchCurseForgeFailed {
        /// 失败原因
        error: String,
    },
    /// 批量取 Modrinth 版本信息失败
    FetchModrinthFailed {
        /// 失败原因
        error: String,
    },
    /// 致命错误弹窗：初始化失败
    FatalInitFailed,
    /// 致命错误弹窗：运行错误
    FatalRunFailed,
    /// 写 lang.txt 失败
    LangFileSaveFailed {
        /// 失败原因
        error: String,
    },
    /// 读自定义音乐文件失败
    MusicReadFailed {
        /// 音乐文件路径
        source: String,
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移开始
    ColormcMigrateStart {
        /// 源目录
        source: String,
        /// 目标目录
        target: String,
        /// 搬运方式，`move` 或 `copy`
        mode: String,
    },
    /// ColorMC 迁移时搬动单个条目失败
    ColormcMoveFailed {
        /// 条目名
        name: String,
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移时删不掉源目录
    ColormcRemoveSourceFailed {
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移时源目录非空，保留不删
    ColormcSourceKept,
    /// ColorMC 迁移时复制单个条目失败
    ColormcCopyFailed {
        /// 条目名
        name: String,
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移完成
    ColormcMigrateDone {
        /// 成功搬动的条目数
        ok: usize,
        /// 失败的条目数
        failed: usize,
        /// 兼容的实例数
        instances: usize,
        /// 检出的实例总数
        total: usize,
    },
    /// ColorMC 迁移时跳过复制失败的文件
    ColormcCopySkip {
        /// 相对路径
        path: String,
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移时处理实例目录失败
    ColormcInstancesFailed {
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移时写回实例配置失败
    ColormcInstancePatchFailed {
        /// 实例路径，相对迁移根目录
        name: String,
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移时合并分组失败
    ColormcGroupMergeFailed {
        /// 失败原因
        error: String,
    },
    /// ColorMC 迁移时合并账户
    ColormcAccountsImported {
        /// 新导入的账户数
        count: usize,
    },
    /// ColorMC 迁移时合并账户失败
    ColormcAccountsFailed {
        /// 失败原因
        error: String,
    },
    /// 探测到旧版 ColorMC 数据
    ColormcDetected {
        /// 数据目录
        dir: String,
        /// 来源标记
        from: String,
    },
    /// 用户跳过了 ColorMC 迁移
    ColormcMigrateSkipped,
    /// ColorMC 迁移标记落盘失败
    ColormcMarkerWriteFailed {
        /// 失败原因
        error: String,
    },
    /// 背景图缩放的等待失败
    BgResizeJoinFailed {
        /// 失败原因
        error: String,
    },
    /// 背景图缩放失败
    BgResizeFailed {
        /// 失败原因
        error: String,
    },
    /// 设置背景图时配置目录还没起来
    BgConfigDirMissing,
    /// 背景图落盘失败
    BgWriteFailed {
        /// 失败原因
        error: String,
    },
    /// 读取背景配置失败
    BgLoadFailed {
        /// 失败原因
        error: String,
    },
    /// 缩放背景图时读取失败
    BgResizeLoadFailed {
        /// 失败原因
        error: String,
    },
    /// 取背景图时读取失败
    BgReadFailed {
        /// 失败原因
        error: String,
    },
    /// 开始设置背景图
    BgSet {
        /// 缩放百分比，已夹到 10..=100
        percent: u32,
        /// 背景图来源，网址或本地文件路径
        source: String,
    },
    /// 开始按新比例缩放背景图
    BgResize {
        /// 缩放百分比，已夹到 10..=100
        percent: u32,
        /// 背景图来源，网址或本地文件路径
        source: String,
    },
    /// 拉起新启动器进程失败
    RestartLauncherFailed {
        /// 失败原因
        error: String,
    },
    /// 取当前可执行文件失败，重启不成
    RestartLauncherError {
        /// 失败原因
        error: String,
    },
    /// 导出耗时总览：准备、打包与合计
    ExportTime {
        /// 压缩档位，`fast` / `normal` / `solid`
        level: String,
        /// 准备阶段：解析勾选、统计在线来源，单位秒
        prepare: f64,
        /// 打包阶段：建包到写完文件，单位秒
        pack: f64,
        /// 合计，单位秒
        total: f64,
        /// 导出的目标文件路径，原样打印
        file: String,
    },
    /// 导出打包明细：建包、写条目、收尾、改名
    ExportPackDetail {
        /// 建包并把清单写好，单位秒；内核格式这一段还含联网查项目名
        open: f64,
        /// 逐条写入压缩包，单位秒
        write: f64,
        /// 收尾：写完目录并刷盘，单位秒；内核格式的进度记在写文件之前，
        /// 最后一个文件的写入时长会算进这一段
        finish: f64,
        /// 改名替换目标文件，单位秒；内核格式把这一步并在收尾里，取 `None`
        commit: Option<f64>,
        /// 写进压缩包的条目数
        entries: usize,
    },
    /// 换了自定义程序图标
    ProgramIconSet {
        /// 落盘的文件路径
        path: String,
    },
    /// 系统通知发不出去
    NotificationFailed {
        /// 失败原因
        error: String,
    },
    /// 系统通知权限申请失败
    NotificationPermissionFailed {
        /// 失败原因
        error: String,
    },
}

impl LogText {
    /// 按当前界面语言渲染
    ///
    /// # 返回值
    ///
    /// 当前界面语言的成品字符串
    pub fn text(&self) -> String {
        self.text_in(&gui_config::locale())
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
            LogText::LocalFileClearFailed { path, error } => {
                format!("[settings] 清理本地文件失败 {path}：{error}")
            }
            LogText::TrayInitFailed { error } => {
                format!("[tray] 建系统托盘失败，后台将没有托盘可用：{error}")
            }
            LogText::TrayVisibleFailed { visible, error } => {
                let action = if *visible { "显示" } else { "收起" };
                format!("[tray] {action}托盘失败：{error}")
            }
            LogText::TraySummonFailed { error } => {
                format!("[tray] 从托盘唤回主窗口失败：{error}")
            }
            LogText::QuitAfterLastGame => {
                String::from("[tray] 最后一个实例已退出且主界面已关闭，退出启动器")
            }
            LogText::IpcStartFailed { error } => {
                format!("[ipc] 启动 IPC 服务失败，单开与唤回失效：{error}")
            }
            LogText::IpcPortSaveFailed { path, error } => {
                format!("[ipc] 端口文件写入失败 {path}：{error}")
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
            LogText::DecorationActivateFailed { label, error } => format!(
                "[window] 装饰激活失败，窗口退回无边框、按钮由界面自绘：窗口={label} ({error})"
            ),
            LogText::TrafficLightsFailed { error } => {
                format!("[window] 摆放原生窗口按钮失败，按钮留在默认位置: {error}")
            }
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
            LogText::LaunchRuntimeFailed { error } => {
                format!("游戏启动运行时创建失败: {error}")
            }
            LogText::AutoMarkReadFailed { path, error } => {
                format!("自动标记：读文件失败 {path}：{error}")
            }
            LogText::AutoMarkQueryFailed { error } => {
                format!("自动标记：查询 Modrinth 失败：{error}")
            }
            LogText::ModUpdateCheckFailed { failed, error } => {
                format!("模组更新检查：{failed} 个查询失败，首个：{error}")
            }
            LogText::ModUpdateAborted { reason } => format!("模组更新异常结束：{reason}"),
            LogText::ModUpdateStarted { total } => format!("模组更新开始：{total} 条"),
            LogText::ModUpdateModsDirFailed { error } => {
                format!("模组更新失败：取 mods 目录失败 {error}")
            }
            LogText::ModUpdateItemFailed { project_id, error } => {
                format!("模组更新失败 {project_id}：{error}")
            }
            LogText::ModUpdatePrepareFailed { error } => {
                format!("模组更新准备异常：{error}")
            }
            LogText::ModUpdateDownloadable { count, total } => {
                format!("模组更新：{count}/{total} 条可下载")
            }
            LogText::ModUpdateDownloadStarted { count } => {
                format!("模组更新：开始下载 {count} 个文件")
            }
            LogText::ModUpdateDownloadFinished { outcome } => {
                format!("模组更新：下载结束 {outcome}")
            }
            LogText::ModUpdateHashMismatch { project_id } => {
                format!("模组更新失败 {project_id}：下载的文件校验不过")
            }
            LogText::ModUpdateFinished { ok, failed } => {
                format!("模组更新：收尾完成 ok={ok} failed={failed}")
            }
            LogText::LogRuntimeLib { path } => format!("运行库：{path}"),
            LogText::LogJavaRedirect => String::from("Java 输出已重定向"),
            LogText::LogJavaLocalRedirect => String::from("Java 切换回本地查找"),
            LogText::LogLoginTime { secs } => format!("登录用时：{secs:.2}s"),
            LogText::LogServerPackCheckTime { secs } => format!("服务器包检查用时：{secs:.2}s"),
            LogText::LogCheckGameFileTime { secs } => format!("检查游戏文件用时：{secs:.2}s"),
            LogText::LogDownloadFileTime { secs } => format!("文件下载用时：{secs:.2}s"),
            LogText::LogLaunchTime { secs } => format!("启动用时：{secs:.2}s"),
            LogText::LogCmdPreTime { secs } => format!("启动前执行用时：{secs:.2}s"),
            LogText::LogCmdPostTime { secs } => format!("启动后执行用时：{secs:.2}s"),
            LogText::ExportTime {
                level,
                prepare,
                pack,
                total,
                file,
            } => format!(
                "[export] 导出耗时：准备 {}、打包 {}、合计 {}，压缩档位 {level}，文件 {file}",
                time_text(*prepare),
                time_text(*pack),
                time_text(*total)
            ),
            LogText::ExportPackDetail {
                open,
                write,
                finish,
                commit,
                entries,
            } => {
                let mut text = format!(
                    "[export] 打包明细：建包 {}、写入 {}、收尾 {}",
                    time_text(*open),
                    time_text(*write),
                    time_text(*finish)
                );
                if let Some(commit) = commit {
                    text.push_str(&format!("、改名 {}", time_text(*commit)));
                }
                text.push_str(&format!("，共 {entries} 个条目"));
                text
            }
            LogText::LogJavaPath { path } => format!("Java 路径：{path}"),
            LogText::LaunchStarting => String::from("游戏启动中…"),
            LogText::LaunchProcessStarted => String::from("游戏进程已启动"),
            LogText::LaunchPanicked { reason } => {
                format!("游戏启动线程异常退出：{reason}")
            }
            LogText::LaunchNoJava { java } => {
                format!("没有找到 Java {java}，请在实例设置里指定 Java 路径")
            }
            LogText::ArchiveIndexMissing { name } => {
                format!("[custom_home] 压缩包里没有 {name}")
            }
            LogText::ArchiveOpenFailed { error } => {
                format!("[custom_home] 打开压缩包失败：{error}")
            }
            LogText::CustomUiArchiveMissing { name } => {
                format!("[custom_ui] 压缩包里没有 {name}")
            }
            LogText::CustomUiArchiveOpenFailed { error } => {
                format!("[custom_ui] 打开压缩包失败：{error}")
            }
            LogText::ThemeEntryCaseMismatch { name } => {
                format!("[theme] 入口文件名大小写不一致，按小写匹配：{name}")
            }
            LogText::UnknownPanic => String::from("未知 panic"),
            LogText::FetchCurseForgeFailed { error } => {
                format!("批量取 CurseForge 文件失败：{error}")
            }
            LogText::FetchModrinthFailed { error } => {
                format!("批量取 Modrinth 版本失败：{error}")
            }
            LogText::FatalInitFailed => String::from("M²L 启动器 - 初始化失败"),
            LogText::FatalRunFailed => String::from("M²L 启动器 - 运行错误"),
            LogText::LangFileSaveFailed { error } => format!("lang.txt 写入失败: {error}"),
            LogText::MusicReadFailed { source, error } => {
                format!("读取自定义音乐失败 {source}：{error}")
            }
            LogText::ColormcMigrateStart {
                source,
                target,
                mode,
            } => format!("[colormc] 迁移开始：{source} -> {target}（{mode}）"),
            LogText::ColormcMoveFailed { name, error } => {
                format!("[colormc] 移动 {name} 失败：{error}")
            }
            LogText::ColormcRemoveSourceFailed { error } => {
                format!("[colormc] 删除源目录失败：{error}")
            }
            LogText::ColormcSourceKept => String::from("[colormc] 源目录非空，保留不删"),
            LogText::ColormcCopyFailed { name, error } => {
                format!("[colormc] 复制 {name} 失败：{error}")
            }
            LogText::ColormcMigrateDone {
                ok,
                failed,
                instances,
                total,
            } => {
                format!("[colormc] 迁移完成：ok={ok} failed={failed} instances={instances}/{total}")
            }
            LogText::ColormcCopySkip { path, error } => {
                format!("[colormc] 跳过复制失败的文件 {path}：{error}")
            }
            LogText::ColormcInstancesFailed { error } => {
                format!("[colormc] 处理实例目录失败：{error}")
            }
            LogText::ColormcInstancePatchFailed { name, error } => {
                format!("[colormc] 写回实例配置 {name} 失败：{error}")
            }
            LogText::ColormcGroupMergeFailed { error } => {
                format!("[colormc] 合并分组失败：{error}")
            }
            LogText::ColormcAccountsImported { count } => {
                format!("[colormc] 合并账户 {count} 个")
            }
            LogText::ColormcAccountsFailed { error } => {
                format!("[colormc] 合并账户失败：{error}")
            }
            LogText::ColormcDetected { dir, from } => {
                format!("[colormc] 探测到旧版数据：{dir}（from={from}）")
            }
            LogText::ColormcMigrateSkipped => String::from("[colormc] 用户跳过迁移"),
            LogText::ColormcMarkerWriteFailed { error } => {
                format!("[colormc] 迁移标记落盘失败：{error}")
            }
            LogText::BgResizeJoinFailed { error } => format!("[bg] 缩放等待失败：{error}"),
            LogText::BgResizeFailed { error } => format!("[bg] 缩放失败：{error}"),
            LogText::BgConfigDirMissing => String::from("[bg] 设置背景失败：配置目录未初始化"),
            LogText::BgWriteFailed { error } => format!("[bg] 背景图落盘失败：{error}"),
            LogText::BgLoadFailed { error } => format!("[bg] 读取背景配置失败：{error}"),
            LogText::BgResizeLoadFailed { error } => format!("[bg] 缩放时读取背景失败：{error}"),
            LogText::BgReadFailed { error } => format!("[bg] 取背景图读取失败：{error}"),
            LogText::BgSet { percent, source } => {
                format!("[bg] 设置背景：缩放 {percent}%，来源 {source}")
            }
            LogText::BgResize { percent, source } => {
                format!("[bg] 调整背景大小：缩放 {percent}%，来源 {source}")
            }
            LogText::RestartLauncherFailed { error } => {
                format!("[window] 拉起新进程失败：{error}")
            }
            LogText::RestartLauncherError { error } => {
                format!("[window] 重启启动器失败：{error}")
            }
            LogText::ProgramIconSet { path } => {
                format!("[window] 已换用自定义程序图标：{path}")
            }
            LogText::NotificationFailed { error } => {
                format!("[notify] 系统通知发送失败：{error}")
            }
            LogText::NotificationPermissionFailed { error } => {
                format!("[notify] 申请系统通知权限失败：{error}")
            }
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
            LogText::LocalFileClearFailed { path, error } => {
                format!("[settings] failed to clear a local file {path}: {error}")
            }
            LogText::TrayInitFailed { error } => format!(
                "[tray] failed to create the tray icon, there will be no tray in the background: {error}"
            ),
            LogText::TrayVisibleFailed { visible, error } => {
                let action = if *visible { "show" } else { "hide" };
                format!("[tray] failed to {action} the tray icon: {error}")
            }
            LogText::TraySummonFailed { error } => {
                format!("[tray] failed to summon the main window from the tray: {error}")
            }
            LogText::QuitAfterLastGame => String::from(
                "[tray] the last instance exited and the main window is closed, quitting the launcher",
            ),
            LogText::IpcStartFailed { error } => {
                format!(
                    "[ipc] failed to start the IPC server, single-instance and summon are disabled: {error}"
                )
            }
            LogText::IpcPortSaveFailed { path, error } => {
                format!("[ipc] failed to write the port file {path}: {error}")
            }
            LogText::FilePathBridgeFailed { error } => format!(
                "[drop] failed to set up the file path bridge, \
                 dropped files will not carry paths: {error}"
            ),
            LogText::FilePathBridgeNoPath { label } => {
                format!("[drop] the path bridge returned no path (window {label})")
            }
            LogText::WebviewDataDirFailed { dir, error } => format!(
                "[window] failed to create the webview data directory, \
                 falling back to the default: {dir} ({error})"
            ),
            LogText::WindowEffectFailed { label, mode, error } => format!(
                "[window] failed to apply the system backdrop, the window falls back to an opaque background: mode={mode} window={label} ({error})"
            ),
            LogText::DecorationActivateFailed { label, error } => format!(
                "[window] decoration activation failed, the window falls back to a borderless frame with app-drawn buttons: window={label} ({error})"
            ),
            LogText::TrafficLightsFailed { error } => format!(
                "[window] failed to position the native window buttons, they stay at the default place: {error}"
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
            LogText::LaunchRuntimeFailed { error } => {
                format!("failed to create the game launch runtime: {error}")
            }
            LogText::AutoMarkReadFailed { path, error } => {
                format!("auto mark: failed to read {path}: {error}")
            }
            LogText::AutoMarkQueryFailed { error } => {
                format!("auto mark: Modrinth query failed: {error}")
            }
            LogText::ModUpdateCheckFailed { failed, error } => {
                format!("mod update check: {failed} queries failed, first: {error}")
            }
            LogText::ModUpdateAborted { reason } => {
                format!("mod update ended abnormally: {reason}")
            }
            LogText::ModUpdateStarted { total } => {
                format!("mod update started: {total} entries")
            }
            LogText::ModUpdateModsDirFailed { error } => {
                format!("mod update failed: could not get the mods directory: {error}")
            }
            LogText::ModUpdateItemFailed { project_id, error } => {
                format!("mod update failed for {project_id}: {error}")
            }
            LogText::ModUpdatePrepareFailed { error } => {
                format!("mod update preparation failed: {error}")
            }
            LogText::ModUpdateDownloadable { count, total } => {
                format!("mod update: {count}/{total} entries are downloadable")
            }
            LogText::ModUpdateDownloadStarted { count } => {
                format!("mod update: starting to download {count} files")
            }
            LogText::ModUpdateDownloadFinished { outcome } => {
                format!("mod update: download finished {outcome}")
            }
            LogText::ModUpdateHashMismatch { project_id } => format!(
                "mod update failed for {project_id}: \
                 the downloaded file did not pass the hash check"
            ),
            LogText::ModUpdateFinished { ok, failed } => {
                format!("mod update: wrapping up done ok={ok} failed={failed}")
            }
            LogText::LogRuntimeLib { path } => format!("runtime library: {path}"),
            LogText::LogJavaRedirect => String::from("Java output has been redirected"),
            LogText::LogJavaLocalRedirect => String::from("Java fell back to a local lookup"),
            LogText::LogLoginTime { secs } => format!("login took {secs:.2}s"),
            LogText::LogServerPackCheckTime { secs } => {
                format!("server pack check took {secs:.2}s")
            }
            LogText::LogCheckGameFileTime { secs } => {
                format!("checking game files took {secs:.2}s")
            }
            LogText::LogDownloadFileTime { secs } => format!("file download took {secs:.2}s"),
            LogText::LogLaunchTime { secs } => format!("launch took {secs:.2}s"),
            LogText::LogCmdPreTime { secs } => format!("pre-launch command took {secs:.2}s"),
            LogText::LogCmdPostTime { secs } => format!("post-launch command took {secs:.2}s"),
            LogText::ExportTime {
                level,
                prepare,
                pack,
                total,
                file,
            } => format!(
                "[export] export took {} in total: preparing {}, packing {} (compression {level}, file {file})",
                time_text(*total),
                time_text(*prepare),
                time_text(*pack)
            ),
            LogText::ExportPackDetail {
                open,
                write,
                finish,
                commit,
                entries,
            } => {
                let mut text = format!(
                    "[export] packing detail: opening {}, writing {}, finishing {}",
                    time_text(*open),
                    time_text(*write),
                    time_text(*finish)
                );
                if let Some(commit) = commit {
                    text.push_str(&format!(", renaming {}", time_text(*commit)));
                }
                text.push_str(&format!(" ({entries} entries)"));
                text
            }
            LogText::LogJavaPath { path } => format!("Java path: {path}"),
            LogText::LaunchStarting => String::from("launching the game…"),
            LogText::LaunchProcessStarted => String::from("the game process has started"),
            LogText::LaunchPanicked { reason } => {
                format!("the launch thread panicked: {reason}")
            }
            LogText::LaunchNoJava { java } => {
                format!("no Java {java} found, set the Java path in the instance settings")
            }
            LogText::ArchiveIndexMissing { name } => {
                format!("[custom_home] the archive has no {name}")
            }
            LogText::ArchiveOpenFailed { error } => {
                format!("[custom_home] failed to open the archive: {error}")
            }
            LogText::CustomUiArchiveMissing { name } => {
                format!("[custom_ui] the archive has no {name}")
            }
            LogText::CustomUiArchiveOpenFailed { error } => {
                format!("[custom_ui] failed to open the archive: {error}")
            }
            LogText::ThemeEntryCaseMismatch { name } => {
                format!("[theme] entry name differs in case, matched case-insensitively: {name}")
            }
            LogText::UnknownPanic => String::from("unknown panic"),
            LogText::FetchCurseForgeFailed { error } => {
                format!("batch CurseForge file fetch failed: {error}")
            }
            LogText::FetchModrinthFailed { error } => {
                format!("batch Modrinth version fetch failed: {error}")
            }
            LogText::FatalInitFailed => String::from("M²L Launcher - initialization failed"),
            LogText::FatalRunFailed => String::from("M²L Launcher - runtime error"),
            LogText::LangFileSaveFailed { error } => {
                format!("failed to write lang.txt: {error}")
            }
            LogText::MusicReadFailed { source, error } => {
                format!("failed to read the custom music {source}: {error}")
            }
            LogText::ColormcMigrateStart {
                source,
                target,
                mode,
            } => format!("[colormc] migrate start: {source} -> {target} ({mode})"),
            LogText::ColormcMoveFailed { name, error } => {
                format!("[colormc] failed to move {name}: {error}")
            }
            LogText::ColormcRemoveSourceFailed { error } => {
                format!("[colormc] failed to remove the source directory: {error}")
            }
            LogText::ColormcSourceKept => {
                String::from("[colormc] the source directory is not empty, keeping it")
            }
            LogText::ColormcCopyFailed { name, error } => {
                format!("[colormc] failed to copy {name}: {error}")
            }
            LogText::ColormcMigrateDone {
                ok,
                failed,
                instances,
                total,
            } => format!(
                "[colormc] migrate done: ok={ok} failed={failed} instances={instances}/{total}"
            ),
            LogText::ColormcCopySkip { path, error } => {
                format!("[colormc] skipping a file that failed to copy {path}: {error}")
            }
            LogText::ColormcInstancesFailed { error } => {
                format!("[colormc] failed to handle the instances directory: {error}")
            }
            LogText::ColormcInstancePatchFailed { name, error } => {
                format!("[colormc] failed to write back the instance config {name}: {error}")
            }
            LogText::ColormcGroupMergeFailed { error } => {
                format!("[colormc] failed to merge groups: {error}")
            }
            LogText::ColormcAccountsImported { count } => {
                format!("[colormc] imported {count} account(s)")
            }
            LogText::ColormcAccountsFailed { error } => {
                format!("[colormc] failed to import accounts: {error}")
            }
            LogText::ColormcDetected { dir, from } => {
                format!("[colormc] found legacy data: {dir} (from={from})")
            }
            LogText::ColormcMigrateSkipped => String::from("[colormc] migrate skipped by the user"),
            LogText::ColormcMarkerWriteFailed { error } => {
                format!("[colormc] failed to write the migrate marker: {error}")
            }
            LogText::BgResizeJoinFailed { error } => format!("[bg] resize wait failed: {error}"),
            LogText::BgResizeFailed { error } => format!("[bg] resize failed: {error}"),
            LogText::BgConfigDirMissing => {
                String::from("[bg] failed to set the background: the config directory is not ready")
            }
            LogText::BgWriteFailed { error } => {
                format!("[bg] failed to write the background image: {error}")
            }
            LogText::BgLoadFailed { error } => {
                format!("[bg] failed to load the background config: {error}")
            }
            LogText::BgResizeLoadFailed { error } => {
                format!("[bg] failed to read the background while resizing: {error}")
            }
            LogText::BgReadFailed { error } => {
                format!("[bg] failed to read the background image: {error}")
            }
            LogText::BgSet { percent, source } => {
                format!("[bg] setting the background: scale {percent}%, source {source}")
            }
            LogText::BgResize { percent, source } => {
                format!("[bg] resizing the background: scale {percent}%, source {source}")
            }
            LogText::RestartLauncherFailed { error } => {
                format!("[window] failed to spawn the new process: {error}")
            }
            LogText::RestartLauncherError { error } => {
                format!("[window] failed to restart the launcher: {error}")
            }
            LogText::ProgramIconSet { path } => {
                format!("[window] switched to the custom program icon: {path}")
            }
            LogText::NotificationFailed { error } => {
                format!("[notify] failed to show a system notification: {error}")
            }
            LogText::NotificationPermissionFailed { error } => {
                format!("[notify] failed to request the notification permission: {error}")
            }
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

        // 致命弹窗标题也要两边都不一样
        assert_ne!(
            LogText::FatalInitFailed.text_in(&Lang::zh_cn),
            LogText::FatalInitFailed.text_in(&Lang::en_us)
        );
        assert!(
            LogText::FatalInitFailed
                .text_in(&Lang::en_us)
                .contains("M²L")
        );

        // 日志窗口那几条：时间要落进文案
        let login = LogText::LogLoginTime { secs: 1.25 };
        assert_eq!(login.text_in(&Lang::zh_cn), String::from("登录用时：1.25s"));
        assert_eq!(
            login.text_in(&Lang::en_us),
            String::from("login took 1.25s")
        );

        let java = LogText::LaunchNoJava { java: 17 };
        assert!(java.text_in(&Lang::zh_cn).contains("Java 17"));
        assert!(java.text_in(&Lang::en_us).contains("Java 17"));

        // 带数量与项目号的条目，两个语言都要把参数落进去
        let miss = LogText::ModUpdateItemFailed {
            project_id: String::from("sodium"),
            error: String::from("timeout"),
        };
        assert_eq!(
            miss.text_in(&Lang::zh_cn),
            String::from("模组更新失败 sodium：timeout")
        );
        assert_eq!(
            miss.text_in(&Lang::en_us),
            String::from("mod update failed for sodium: timeout")
        );

        let done = LogText::ModUpdateFinished { ok: 3, failed: 1 };
        assert!(done.text_in(&Lang::zh_cn).contains("ok=3 failed=1"));
        assert!(done.text_in(&Lang::en_us).contains("ok=3 failed=1"));

        // 带缩放与来源的条目：参数要落进两种文案
        let set = LogText::BgSet {
            percent: 50,
            source: String::from("a.png"),
        };
        assert!(set.text_in(&Lang::zh_cn).contains("缩放 50%"));
        assert!(set.text_in(&Lang::zh_cn).contains("a.png"));
        assert!(set.text_in(&Lang::en_us).contains("50%"));
        assert!(set.text_in(&Lang::en_us).contains("a.png"));

        // 耗时条目：一秒以内用毫秒；没有改名那一段时不要留空壳
        let detail = LogText::ExportPackDetail {
            open: 0.012,
            write: 2.5,
            finish: 0.3,
            commit: None,
            entries: 7,
        };
        assert!(detail.text_in(&Lang::zh_cn).contains("12 ms"));
        assert!(detail.text_in(&Lang::zh_cn).contains("2.50 s"));
        assert!(detail.text_in(&Lang::zh_cn).contains("共 7 个条目"));
        assert!(!detail.text_in(&Lang::zh_cn).contains("改名"));
        assert!(detail.text_in(&Lang::en_us).contains("12 ms"));
        assert!(detail.text_in(&Lang::en_us).contains("(7 entries)"));

        let overview = LogText::ExportTime {
            level: String::from("fast"),
            prepare: 0.05,
            pack: 12.34,
            total: 12.5,
            file: String::from("a.zip"),
        };
        assert!(overview.text_in(&Lang::zh_cn).contains("50 ms"));
        assert!(overview.text_in(&Lang::zh_cn).contains("12.34 s"));
        assert!(overview.text_in(&Lang::zh_cn).contains("a.zip"));
        assert!(overview.text_in(&Lang::en_us).contains("fast"));

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

/// 系统通知的文案：标题与正文分开给，都跟界面语言走
///
/// 只在启动器不在前台时用，见 [`crate::notify`]。
pub enum NotifyText {
    /// 下载队列已清空
    DownloadDone {
        /// 这一轮从空队列开始算起的任务数
        count: usize,
    },
    /// 整合包安装完成
    ModpackDone {
        /// 整合包显示名
        name: String,
    },
    /// 整合包安装失败
    ModpackFailed {
        /// 整合包显示名
        name: String,
    },
}

impl NotifyText {
    /// 按当前界面语言渲染
    ///
    /// # 返回值
    ///
    /// `(标题, 正文)`，都是当前界面语言的成品字符串
    pub fn parts(&self) -> (String, String) {
        match gui_config::locale() {
            Lang::zh_cn => self.zh(),
            Lang::en_us => self.en(),
        }
    }

    /// 中文标题与正文
    fn zh(&self) -> (String, String) {
        match self {
            NotifyText::DownloadDone { count } => (
                String::from("下载完成"),
                format!("{count} 个下载任务已结束，打开下载窗口查看结果"),
            ),
            NotifyText::ModpackDone { name } => {
                (String::from("安装完成"), format!("{name} 已安装完成"))
            }
            // 失败原因是一串前端文案键，界面侧才翻得出来，通知里只报是哪一包
            NotifyText::ModpackFailed { name } => (
                String::from("安装失败"),
                format!("{name} 安装失败，打开下载窗口查看原因"),
            ),
        }
    }

    /// 英文标题与正文
    fn en(&self) -> (String, String) {
        match self {
            NotifyText::DownloadDone { count } => (
                String::from("Download finished"),
                format!("{count} download tasks finished, open the download window for details"),
            ),
            NotifyText::ModpackDone { name } => (
                String::from("Installation finished"),
                format!("{name} has been installed"),
            ),
            NotifyText::ModpackFailed { name } => (
                String::from("Installation failed"),
                format!("failed to install {name}, open the download window for details"),
            ),
        }
    }
}

/// 耗时文本：1 秒以内用毫秒，超过用秒
///
/// - `secs`: 秒数
///
/// # 返回值
///
/// 形如 `12 ms` 或 `3.45 s`
fn time_text(secs: f64) -> String {
    if secs < 1.0 {
        format!("{:.0} ms", secs * 1000.0)
    } else {
        format!("{:.2} s", secs)
    }
}
