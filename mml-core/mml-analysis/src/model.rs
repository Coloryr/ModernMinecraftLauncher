//! 崩溃分析的数据结构
//!
//! 解析结果与分析结论都只带稳定 id 与机器可读数据，面向用户的文案由调用方按 id 取 i18n。

/// 输入文本的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// 带 `---- Minecraft Crash Report ----` 头的崩溃报告
    CrashReport,
    /// JVM 致命错误日志，通常是 `hs_err_pid*.log`
    JvmFatal,
    /// 游戏运行日志，`latest.log` 这一类
    GameLog,
    /// 认不出类型
    Unknown,
}

impl LogKind {
    /// 稳定 id，供前端取文案
    pub fn as_str(&self) -> &'static str {
        match self {
            LogKind::CrashReport => "crash_report",
            LogKind::JvmFatal => "jvm_fatal",
            LogKind::GameLog => "game_log",
            LogKind::Unknown => "unknown",
        }
    }
}

/// 模组加载器
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    /// Forge
    Forge,
    /// NeoForge
    NeoForge,
    /// Fabric
    Fabric,
    /// Quilt
    Quilt,
}

impl LoaderKind {
    /// 稳定 id，与启动器里的加载器 id 一致
    pub fn as_str(&self) -> &'static str {
        match self {
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
        }
    }
}

/// 从崩溃报告里读到的运行环境
#[derive(Debug, Clone, Default)]
pub struct Environment {
    /// Minecraft 版本，如 `1.20.1`
    pub minecraft_version: Option<String>,
    /// 模组加载器；原版实例或认不出时为空
    pub loader: Option<LoaderKind>,
    /// 加载器版本
    pub loader_version: Option<String>,
    /// Java 版本，如 `17.0.8`
    pub java_version: Option<String>,
    /// Java 发行方，如 `Microsoft`
    pub java_vendor: Option<String>,
    /// 操作系统描述
    pub os: Option<String>,
    /// 最大堆内存，单位 MiB
    pub memory_max_mb: Option<u64>,
    /// 崩溃报告里列出的模组
    pub mods: Vec<ModEntry>,
}

impl Environment {
    /// 按 id 找模组
    ///
    /// - `id`: 模组 id，大小写不敏感
    ///
    /// # 返回值
    ///
    /// 找到的条目；没有时返回 `None`
    pub fn mod_of(&self, id: &str) -> Option<&ModEntry> {
        self.mods.iter().find(|m| m.id.eq_ignore_ascii_case(id))
    }
}

/// 模组列表里的一条
#[derive(Debug, Clone)]
pub struct ModEntry {
    /// 模组 id
    pub id: String,
    /// 显示名；取不到时与 id 相同
    pub name: String,
    /// 版本；取不到时为空串
    pub version: String,
}

/// 异常链里的一层
#[derive(Debug, Clone)]
pub struct ExceptionFrame {
    /// 异常类全名，如 `java.lang.NullPointerException`
    pub class: String,
    /// 异常消息；没有时为空串
    pub message: String,
}

impl ExceptionFrame {
    /// 类名加消息的一行文本
    pub fn line(&self) -> String {
        if self.message.is_empty() {
            self.class.clone()
        } else {
            format!("{}: {}", self.class, self.message)
        }
    }
}

/// 解析后的崩溃报告或日志
#[derive(Debug, Clone)]
pub struct CrashReport {
    /// 输入文本的类型
    pub kind: LogKind,
    /// 崩溃报告里的 `Description:` 行
    pub description: Option<String>,
    /// 崩溃报告里的 `Time:` 行
    pub time: Option<String>,
    /// 最外层异常
    pub exception: Option<ExceptionFrame>,
    /// `Caused by:` 链，按出现顺序
    pub causes: Vec<ExceptionFrame>,
    /// 运行环境
    pub environment: Environment,
    /// 原始文本
    pub(crate) raw: String,
    /// 本次故障的诊断正文，不含系统信息、线程转储和附带日志
    pub(crate) diagnostic: String,
    /// 诊断正文的小写副本，与 `diagnostic` 的行一一对应
    pub(crate) lower: String,
    /// 异常与栈帧所在的行，规则优先在这段里匹配
    pub(crate) detail: String,
}

impl CrashReport {
    /// 解析一份崩溃报告或日志
    ///
    /// - `text`: 日志全文，UTF-8
    ///
    /// # 返回值
    ///
    /// 解析结果；认不出的文本也能解析，只是字段大多为空
    pub fn parse(text: &str) -> Self {
        crate::parse::parse(text)
    }

    /// 原始文本
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// 按规则分析
    ///
    /// # 返回值
    ///
    /// 见 [`CrashAnalysis`]
    pub fn analyze(&self) -> CrashAnalysis {
        crate::rules::analyze(self)
    }
}

/// 问题的严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 只是提示
    Info,
    /// 可能影响运行
    Warning,
    /// 本次崩溃的直接原因
    Error,
    /// 进程级崩溃，如 JVM 致命错误
    Fatal,
}

impl Severity {
    /// 稳定 id
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Fatal => "fatal",
        }
    }
}

/// 规则命中的把握
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// 只是线索，需要用户自己核对
    Low,
    /// 证据比较明确
    Medium,
    /// 异常类型与关键消息都对上了
    High,
}

impl Confidence {
    /// 稳定 id
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }
}

/// 命中的问题类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingId {
    /// JVM 致命错误，如访问越界、SIGSEGV
    JvmFatal,
    /// 堆内存不足
    OutOfMemoryHeap,
    /// 元空间不足
    OutOfMemoryMetaspace,
    /// 直接内存不足
    OutOfMemoryDirect,
    /// GC 开销超限
    OutOfMemoryGc,
    /// 线程创建失败
    OutOfMemoryNativeThread,
    /// 类的字节码版本高于当前 Java
    JavaVersionTooOld,
    /// 模组缺少前置
    MissingDependency,
    /// 依赖的版本不满足，如模组要求更高的加载器版本
    DependencyVersion,
    /// 模组之间不兼容
    IncompatibleModSet,
    /// 模组装错了加载器，如把 Fabric 模组放进 NeoForge
    WrongLoader,
    /// 同一个模组装了多份
    DuplicateMod,
    /// Mixin 注入失败
    MixinFailure,
    /// 模组入口点执行失败
    ModEntrypointFailure,
    /// 找不到类
    MissingClass,
    /// 找不到方法或字段
    MissingMethod,
    /// 图形驱动或 OpenGL 上下文问题
    GraphicsDriver,
    /// 本地库缺失或架构不匹配
    MissingNative,
    /// 文件访问被拒绝
    AccessDenied,
    /// 文件损坏，如 zip 头错误
    CorruptFile,
    /// 存档或区域文件损坏
    CorruptWorld,
    /// 栈溢出
    StackOverflow,
    /// 渲染或区块构建崩溃
    RenderFailure,
    /// 服务端主线程卡死，被看门狗强制关服
    WatchdogTimeout,
    /// 维度注册缺失，通常是维度模组与存档不匹配
    DimensionMissing,
    /// 模组初始化失败
    ModInitFailure,
    /// 安全管理器或权限问题
    Security,
    /// 没有规则命中，只能给出异常本身
    Unclassified,
}

impl FindingId {
    /// 稳定 id，供前端取文案
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingId::JvmFatal => "jvm_fatal",
            FindingId::OutOfMemoryHeap => "out_of_memory_heap",
            FindingId::OutOfMemoryMetaspace => "out_of_memory_metaspace",
            FindingId::OutOfMemoryDirect => "out_of_memory_direct",
            FindingId::OutOfMemoryGc => "out_of_memory_gc",
            FindingId::OutOfMemoryNativeThread => "out_of_memory_native_thread",
            FindingId::JavaVersionTooOld => "java_version_too_old",
            FindingId::MissingDependency => "missing_dependency",
            FindingId::DependencyVersion => "dependency_version",
            FindingId::IncompatibleModSet => "incompatible_mod_set",
            FindingId::WrongLoader => "wrong_loader",
            FindingId::DuplicateMod => "duplicate_mod",
            FindingId::MixinFailure => "mixin_failure",
            FindingId::ModEntrypointFailure => "mod_entrypoint_failure",
            FindingId::MissingClass => "missing_class",
            FindingId::MissingMethod => "missing_method",
            FindingId::GraphicsDriver => "graphics_driver",
            FindingId::MissingNative => "missing_native",
            FindingId::AccessDenied => "access_denied",
            FindingId::CorruptFile => "corrupt_file",
            FindingId::CorruptWorld => "corrupt_world",
            FindingId::StackOverflow => "stack_overflow",
            FindingId::RenderFailure => "render_failure",
            FindingId::WatchdogTimeout => "watchdog_timeout",
            FindingId::DimensionMissing => "dimension_missing",
            FindingId::ModInitFailure => "mod_init_failure",
            FindingId::Security => "security",
            FindingId::Unclassified => "unclassified",
        }
    }
}

/// 一条分析结论
#[derive(Debug, Clone)]
pub struct Finding {
    /// 问题类型
    pub id: FindingId,
    /// 严重程度
    pub severity: Severity,
    /// 命中把握
    pub confidence: Confidence,
    /// 命中的原始行，已去空白并截断
    pub evidence: Vec<String>,
    /// 涉及的模组 id
    pub mods: Vec<String>,
    /// 机器可读的细节，如缺少的类名、要求的 Java 版本
    pub details: Vec<String>,
}

/// 整份分析结果
#[derive(Debug, Clone)]
pub struct CrashAnalysis {
    /// 输入文本的类型
    pub kind: LogKind,
    /// 崩溃报告里的描述
    pub description: Option<String>,
    /// 最外层异常
    pub exception: Option<ExceptionFrame>,
    /// `Caused by:` 链
    pub causes: Vec<ExceptionFrame>,
    /// 运行环境
    pub environment: Environment,
    /// 按相关性排列的结论，第一条是主因
    pub findings: Vec<Finding>,
    /// 栈帧与结论里出现过的模组，按可疑程度排序
    pub suspected_mods: Vec<String>,
}

impl CrashAnalysis {
    /// 主因，即第一条结论
    ///
    /// # 返回值
    ///
    /// 没有命中任何规则时返回 `None`
    pub fn primary(&self) -> Option<&Finding> {
        self.findings.first()
    }
}
