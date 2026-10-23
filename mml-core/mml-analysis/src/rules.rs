//! 崩溃原因规则
//!
//! 规则按"越具体越靠前"的顺序匹配，第一条即主因。所有规则只读解析结果与原文，
//! 命中后给出稳定 id、证据行与机器可读细节，不生成面向用户的句子。
//!
//! 异常类相关的规则一律以顶层异常与 Caused by 链为准，不拿整份原文做子串匹配：
//! 崩溃报告里常夹着完整启动日志，按原文匹配会把日志里的 LWJGLException、
//! ZipException 这类类名当成崩溃原因。

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Confidence, CrashAnalysis, CrashReport, Finding, FindingId, Severity};

/// 类文件版本，如 class file version 65.0
static RE_CLASS_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)class file version (\d+)").unwrap());

/// 任意 .json 文件名，用来从 Mixin 报错里挑配置名
static RE_JSON_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z0-9_.-]+\.json)").unwrap());

/// Forge 缺前置行里的 Mod ID: 'x'
static RE_MOD_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)mod id: '([^']+)'").unwrap());

/// Forge 缺前置行里的 Requested by: 'x'
static RE_REQUESTED_BY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)requested by: '([^']+)'").unwrap());

/// Fabric 报错里的 Mod 'x'
static RE_MOD_QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)mod '([^']+)'").unwrap());

/// Fabric 报错里的 of mod 'x'
static RE_OF_MOD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)of mod '([^']+)'").unwrap());

/// Fabric 入口点报错里的 provided by 'x'
static RE_PROVIDED_BY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)provided by '([^']+)'").unwrap());

/// Mixin 报错里的 from mod x
static RE_FROM_MOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)from mod ([A-Za-z0-9_.-]+)").unwrap());

/// Fabric 的 Mixin apply for mod x failed
static RE_MIXIN_APPLY_MOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)mixin apply for mod ([A-Za-z0-9_.-]+) failed").unwrap());

/// Mixin config x.json from mod y
static RE_MIXIN_CONFIG_FROM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)mixin config ([A-Za-z0-9_.-]+\.json) from mod ([A-Za-z0-9_.-]+)").unwrap()
});

/// config.json:ClassName
static RE_MIXIN_CLASS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z0-9_.-]+\.json):([A-Za-z0-9_$.]+)").unwrap());

/// NeoForge 模组加载问题段落的标题行
static RE_ISSUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^-- mod loading issue for: ([A-Za-z0-9_.-]+) --\s*$").unwrap()
});

/// NeoForge 的 Mod x requires y 1.2.3 or above
static RE_REQUIRES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)mod ([A-Za-z0-9_.-]+) requires ([A-Za-z0-9_.-]+)(?: ([0-9][^\s]*)(?: or above)?)?",
    )
    .unwrap()
});

/// NeoForge 报错里的 Currently, neoforge is 21.1.216
static RE_ACTUAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)currently, ([A-Za-z0-9_.-]+) is ([0-9][^\s]*)").unwrap());

/// File mods\x.jar is a Fabric mod and cannot be loaded
static RE_WRONG_LOADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(fabric|forge|neoforge|quilt) mod and cannot be loaded").unwrap());

/// 报错里被点名的 jar 文件名，如 mods\indium-1.0.35+mc1.21.jar
static RE_JAR_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([A-Za-z0-9_.+]+-[0-9][A-Za-z0-9_.+-]*)\.jar").unwrap()
});

/// JVM 致命错误暂不分析；规则保留，后续接上时改为 true
const ANALYZE_JVM_FATAL: bool = false;

/// 匹配用的三份文本
///
/// lower 与 raw 行序一一对应，由解析阶段算好，规则里不再逐条 to_lowercase。
struct Scan<'a> {
    /// 原文
    raw: &'a str,
    /// 原文的小写副本
    lower: &'a str,
    /// 异常与栈帧部分的小写文本
    detail_lower: String,
}

impl Scan<'_> {
    /// 取包含任一关键词的原始行
    fn evidence(&self, needles: &[&str], limit: usize) -> Vec<String> {
        evidence(self.raw, self.lower, needles, limit)
    }
}

/// 按规则分析一份解析结果
///
/// 只分析游戏日志与 crash-report；JVM 致命错误日志与认不出类型的文本直接返回空结论。
///
/// - report: 解析后的崩溃报告或日志
///
/// # 返回值
///
/// 见 CrashAnalysis
pub(crate) fn analyze(report: &CrashReport) -> CrashAnalysis {
    let scan = Scan {
        raw: &report.diagnostic,
        lower: &report.lower,
        detail_lower: report.detail.to_lowercase(),
    };
    let mut findings = Vec::new();

    let analyzable = matches!(
        report.kind,
        crate::model::LogKind::CrashReport | crate::model::LogKind::GameLog
    ) || (ANALYZE_JVM_FATAL && report.kind == crate::model::LogKind::JvmFatal);
    if !analyzable {
        return CrashAnalysis {
            kind: report.kind,
            description: report.description.clone(),
            exception: report.exception.clone(),
            causes: report.causes.clone(),
            environment: report.environment.clone(),
            findings,
            suspected_mods: Vec::new(),
        };
    }

    if ANALYZE_JVM_FATAL {
        check_jvm_fatal(report, &scan, &mut findings);
    }
    check_out_of_memory(report, &scan, &mut findings);
    check_java_version(report, &scan, &mut findings);
    // 栈溢出与看门狗是"异常类型即结论"的强信号，排在泛化的类名规则之前
    check_stack_overflow(report, &scan, &mut findings);
    check_watchdog(report, &scan, &mut findings);
    check_missing_dependency(&scan, &mut findings);
    let dependency_issue = findings.iter().any(|finding| {
        matches!(
            finding.id,
            FindingId::MissingDependency | FindingId::DependencyVersion
        )
    });
    check_incompatible(&scan, dependency_issue, &mut findings);
    check_wrong_loader(&scan, &mut findings);
    check_duplicate(&scan, &mut findings);
    check_mixin(report, &scan, &mut findings);
    check_entrypoint(&scan, &mut findings);
    check_missing_class(report, &scan, &mut findings);
    check_missing_method(report, &scan, &mut findings);
    check_graphics(report, &scan, &mut findings);
    check_missing_native(report, &scan, &mut findings);
    check_access_denied(&scan, &mut findings);
    check_world(&scan, &mut findings);
    check_corrupt_file(report, &scan, &mut findings);
    check_render(&scan, &mut findings);
    check_dimension(&scan, &mut findings);
    let has_finding = !findings.is_empty();
    check_mod_init(&scan, has_finding, &mut findings);
    check_security(report, &scan, &mut findings);

    if findings.is_empty()
        && let Some(exception) = &report.exception
    {
        let mut evidence = vec![clip(&exception.line(), 240)];
        evidence.extend(
            report
                .detail
                .lines()
                .skip(1)
                .take(4)
                .map(|line| clip(line, 240)),
        );
        findings.push(Finding {
            id: FindingId::Unclassified,
            severity: Severity::Error,
            confidence: Confidence::Low,
            evidence,
            mods: Vec::new(),
            details: Vec::new(),
        });
    }

    let suspected_mods = collect_suspected(report, &findings, &scan.detail_lower);

    CrashAnalysis {
        kind: report.kind,
        description: report.description.clone(),
        exception: report.exception.clone(),
        causes: report.causes.clone(),
        environment: report.environment.clone(),
        findings,
        suspected_mods,
    }
}

/// 顶层异常或 Caused by 链里有没有这个类
fn chain_has(report: &CrashReport, class: &str) -> bool {
    report
        .exception
        .iter()
        .chain(report.causes.iter())
        .any(|frame| frame.class.ends_with(class))
}

/// 取顶层异常与 Caused by 链
fn frames(report: &CrashReport) -> impl Iterator<Item = &crate::model::ExceptionFrame> {
    report.exception.iter().chain(report.causes.iter())
}

/// JVM 致命错误
fn check_jvm_fatal(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let hit = report.kind == crate::model::LogKind::JvmFatal
        || scan.lower.contains("a fatal error has been detected")
        || scan.lower.contains("exception_access_violation");
    if !hit {
        return;
    }

    let details = scan
        .raw
        .lines()
        .find(|line| line.contains("Problematic frame:"))
        .map(|line| vec![clip(line.trim(), 240)])
        .unwrap_or_default();

    out.push(Finding {
        id: FindingId::JvmFatal,
        severity: Severity::Fatal,
        confidence: Confidence::High,
        evidence: scan.evidence(
            &[
                "a fatal error has been detected",
                "problematic frame",
                "exception_access_violation",
                "sigsegv",
            ],
            6,
        ),
        mods: Vec::new(),
        details,
    });
}

/// 内存不足，按错误消息细分
fn check_out_of_memory(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let Some(frame) = frames(report).find(|frame| frame.class.ends_with("OutOfMemoryError")) else {
        return;
    };
    let message = frame.message.to_lowercase();

    let rules = [
        (FindingId::OutOfMemoryHeap, "java heap space"),
        (FindingId::OutOfMemoryMetaspace, "metaspace"),
        (FindingId::OutOfMemoryMetaspace, "compressed class space"),
        (FindingId::OutOfMemoryDirect, "direct buffer memory"),
        (FindingId::OutOfMemoryGc, "gc overhead limit exceeded"),
        (
            FindingId::OutOfMemoryNativeThread,
            "unable to create native thread",
        ),
    ];

    let specific = rules
        .iter()
        .find(|(_, needle)| message.contains(needle))
        .map(|(id, _)| *id);
    let id = specific.unwrap_or(FindingId::OutOfMemoryHeap);

    let mut details = Vec::new();
    if let Some(mb) = report.environment.memory_max_mb {
        details.push(format!("memory_max_mb={mb}"));
    }
    if let Some(java) = &report.environment.java_version {
        details.push(format!("java_version={java}"));
    }

    out.push(Finding {
        id,
        severity: Severity::Fatal,
        confidence: if specific.is_some() {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence: scan.evidence(
            &[
                "outofmemoryerror",
                "java heap space",
                "metaspace",
                "direct buffer memory",
            ],
            6,
        ),
        mods: Vec::new(),
        details,
    });
}

/// 字节码版本高于当前 Java
fn check_java_version(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    if !chain_has(report, "UnsupportedClassVersionError") {
        return;
    }

    let mut details = Vec::new();
    if let Some(captures) = RE_CLASS_VERSION.captures(scan.raw)
        && let Some(major) = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok())
    {
        details.push(format!("class_file_version={major}"));
        if major >= 45 {
            details.push(format!("required_java={}", major - 44));
        }
    }

    out.push(Finding {
        id: FindingId::JavaVersionTooOld,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&["unsupportedclassversionerror", "class file version"], 4),
        mods: Vec::new(),
        details,
    });
}

/// 栈溢出
fn check_stack_overflow(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    if !chain_has(report, "StackOverflowError") {
        return;
    }

    out.push(Finding {
        id: FindingId::StackOverflow,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&["stackoverflowerror"], 4),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 服务端看门狗超时
fn check_watchdog(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let hit = frames(report).any(|frame| {
        frame.class == "java.lang.Error" && frame.message.eq_ignore_ascii_case("watchdog")
    }) || scan.detail_lower.contains("java.lang.error: watchdog")
        || (scan.lower.contains("watching server") && scan.detail_lower.contains("watchdog"));
    if !hit {
        return;
    }

    out.push(Finding {
        id: FindingId::WatchdogTimeout,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&["watchdog", "single server tick took", "watching server"], 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 模组缺少前置，或依赖的版本不满足
fn check_missing_dependency(scan: &Scan, out: &mut Vec<Finding>) {
    let legacy = [
        "missing or unsupported mandatory dependencies",
        "unmet dependency",
        "mod resolution encountered an error",
        "which is missing",
        "requires version",
        "requires any version",
        "requires mod",
        "is missing!",
    ]
    .iter()
    .any(|needle| scan.lower.contains(needle));

    // NeoForge 的写法是 "-- Mod loading issue for: x --" 加 "Failure message: Mod x requires y"；
    // 同一段落也用来报"不兼容""装错加载器"，那些没有 requires，不能算缺前置
    let neo_requires = RE_ISSUE.is_match(scan.lower) && scan.lower.contains("requires ");
    if !legacy && !neo_requires {
        return;
    }

    let mut mods = Vec::new();
    for captures in RE_ISSUE.captures_iter(scan.lower) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_MOD_ID.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_REQUESTED_BY.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_OF_MOD.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_MOD_QUOTED.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }

    let mut details = Vec::new();
    let mut version_issue = false;
    for captures in RE_REQUIRES.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
        push_mod(&mut mods, &captures[2]);
        push_detail(&mut details, format!("requires={}", &captures[2]));
        if let Some(version) = captures.get(3) {
            version_issue = true;
            push_detail(&mut details, format!("required_version={}", version.as_str()));
        }
    }
    for captures in RE_ACTUAL.captures_iter(scan.raw) {
        push_detail(
            &mut details,
            format!("actual_version={} {}", &captures[1], &captures[2]),
        );
    }

    out.push(Finding {
        id: if version_issue {
            FindingId::DependencyVersion
        } else {
            FindingId::MissingDependency
        },
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(
            &[
                "missing or unsupported mandatory dependencies",
                "unmet dependency",
                "which is missing",
                "requires",
                "mod loading issue for",
                "failure message",
            ],
            8,
        ),
        mods,
        details,
    });
}

/// 模组之间不兼容，缺少前置时不再重复报
fn check_incompatible(scan: &Scan, dependency_issue: bool, out: &mut Vec<Finding>) {
    if dependency_issue {
        return;
    }

    let needles = [
        "incompatible mod set",
        "incompatible with the game or each other",
        "mod resolution failed",
        "is incompatible with",
    ];
    if !needles.iter().any(|needle| scan.lower.contains(needle)) {
        return;
    }

    let mut mods = Vec::new();
    for captures in RE_MOD_QUOTED.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_ISSUE.captures_iter(scan.lower) {
        push_mod(&mut mods, &captures[1]);
    }

    out.push(Finding {
        id: FindingId::IncompatibleModSet,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&needles, 8),
        mods,
        details: Vec::new(),
    });
}

/// 把 Fabric 模组塞进了别的加载器
fn check_wrong_loader(scan: &Scan, out: &mut Vec<Finding>) {
    let Some(captures) = RE_WRONG_LOADER.captures(scan.raw) else {
        return;
    };

    let mut mods = Vec::new();
    for captures in RE_ISSUE.captures_iter(scan.lower) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_MOD_QUOTED.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }
    // 真正装错的往往是报错里那个 jar，问题段落标题只是被牵连的模组
    if let Some(file) = RE_JAR_FILE.captures(scan.raw).and_then(|item| item.get(1))
        && let Some(id) = jar_mod_id(file.as_str())
    {
        mods.retain(|item| !item.eq_ignore_ascii_case(&id));
        mods.insert(0, id);
    }

    out.push(Finding {
        id: FindingId::WrongLoader,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&["mod and cannot be loaded", "is a fabric mod"], 4),
        mods,
        details: vec![format!("loader={}", captures[1].to_lowercase())],
    });
}

/// 同一个模组装了多份
fn check_duplicate(scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "duplicate mods",
        "duplicate mod",
        "already present",
        "mods share id",
        "duplicate key",
        "duplicate entry",
    ];
    if !needles.iter().any(|needle| scan.lower.contains(needle)) {
        return;
    }

    let fatal = scan.lower.contains("duplicate mods found") || scan.lower.contains("mods share id");
    let mut mods = Vec::new();
    for captures in RE_MOD_QUOTED.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }

    out.push(Finding {
        id: FindingId::DuplicateMod,
        severity: if fatal {
            Severity::Error
        } else {
            Severity::Warning
        },
        confidence: Confidence::Medium,
        evidence: scan.evidence(&needles, 6),
        mods,
        details: Vec::new(),
    });
}

/// Mixin 注入失败
fn check_mixin(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "mixin apply failed",
        "mixin apply for mod",
        "mixintransformererror",
        "invalidmixinexception",
        "mixin transformation of",
        "mixin.injection.throwables",
        "critical injection failure",
    ];
    // "mixin config" 本身在正常启动日志里也可能出现，必须配上错误措辞才算
    let config_error = scan.detail_lower.contains("mixin config")
        && (scan.detail_lower.contains("does not specify")
            || scan.detail_lower.contains("could not be")
            || scan.detail_lower.contains("failed to")
            || scan.detail_lower.contains("is missing")
            || scan.detail_lower.contains("invalid"));
    if !needles
        .iter()
        .any(|needle| scan.detail_lower.contains(needle))
        && !config_error
    {
        return;
    }

    let suspects = mixin_suspects(report, scan.raw);
    let strong = RE_FROM_MOD.is_match(scan.raw)
        || RE_MIXIN_APPLY_MOD.is_match(scan.raw)
        || RE_MIXIN_CONFIG_FROM.is_match(scan.raw);

    // 配置名只在异常与栈帧里找：整份日志里每个模组的 mixin 配置都会出现，
    // 全收进来会把 details 撑到几百条
    let mut details: Vec<String> = Vec::new();
    for name in mixin_configs(scan, 8) {
        push_detail(&mut details, format!("mixin_config={name}"));
    }
    if let Some(class) = mixin_class(&report.detail) {
        push_detail(&mut details, format!("mixin_class={class}"));
    }

    out.push(Finding {
        id: FindingId::MixinFailure,
        severity: Severity::Error,
        confidence: if strong || !suspects.is_empty() {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence: scan.evidence(&needles, 8),
        mods: suspects,
        details,
    });
}

/// 从 Mixin 报错里挑出最可能出错的模组
///
/// 依据依次是显式的 from mod、Mixin 配置名、栈帧包名与报错里出现的模组 id。
///
/// - report: 解析结果，模组列表用来把配置名 / 包名对回模组 id
/// - raw: 原文
///
/// # 返回值
///
/// 按可疑程度排序的模组 id，第一个最可能是元凶
fn mixin_suspects(report: &CrashReport, raw: &str) -> Vec<String> {
    let mut scores: HashMap<String, u32> = HashMap::new();
    let lower_raw = raw.to_lowercase();

    let mut add = |id: &str, score: u32| {
        let id = id.trim().trim_end_matches([',', '.', ';', ')', ':']);
        if id.is_empty() || is_loader_mod(id) {
            return;
        }
        *scores.entry(id.to_string()).or_default() += score;
    };

    // 报错里直接点名的模组
    for captures in RE_FROM_MOD.captures_iter(raw) {
        add(&captures[1], 10);
    }
    for captures in RE_MIXIN_APPLY_MOD.captures_iter(raw) {
        add(&captures[1], 10);
    }
    for captures in RE_MIXIN_CONFIG_FROM.captures_iter(raw) {
        add(&captures[2], 10);
    }

    // Mixin 配置名 → 模组 id
    for captures in RE_JSON_NAME.captures_iter(raw) {
        let name = captures[1].to_string();
        if !name.to_lowercase().contains("mixin") {
            continue;
        }
        let Some(candidate) = mod_from_mixin_config(&name) else {
            continue;
        };

        if let Some(entry) = report
            .environment
            .mods
            .iter()
            .find(|entry| entry.id.eq_ignore_ascii_case(&candidate))
        {
            add(&entry.id, 8);
        } else if !candidate.contains('.') {
            // 对不上模组列表时只认像 id 的名字：mixins.angelica.early.j21.json
            // 推出来的是包名，当模组报出去只会误导
            add(&candidate, 6);
        }

        // 配置名带包路径时，用包含关系对回真实模组 id
        let candidate = candidate.to_lowercase();
        for entry in &report.environment.mods {
            let id = entry.id.to_lowercase();
            if id.len() >= 3
                && (candidate == id
                    || candidate.ends_with(&format!(".{id}"))
                    || candidate.starts_with(&format!("{id}.")))
            {
                add(&entry.id, 6);
            }
        }
    }

    // 栈帧包名：com.examplemod.mixin.FooMixin
    for line in report.detail.lines() {
        let Some(rest) = line.trim_start().strip_prefix("at ") else {
            continue;
        };
        let class = rest.split('(').next().unwrap_or(rest).to_lowercase();
        if !class.contains("mixin") {
            continue;
        }
        for segment in class.split(['.', '/', '$']) {
            // NeoForge 的栈帧写成 TRANSFORMER/modid@版本/包名，版本要剥掉
            let segment = segment.split('@').next().unwrap_or(segment);
            if let Some(entry) = report
                .environment
                .mods
                .iter()
                .find(|entry| entry.id.eq_ignore_ascii_case(segment))
            {
                add(&entry.id, 5);
            }
        }
    }

    // 报错里的 mod 'x'，以及正文里出现的模组 id
    for captures in RE_MOD_QUOTED.captures_iter(raw) {
        add(&captures[1], 3);
    }
    for entry in &report.environment.mods {
        let id = entry.id.to_lowercase();
        if id.len() >= 4 && lower_raw.contains(&id) {
            add(&entry.id, 1);
        }
    }

    let mut list: Vec<(String, u32)> = scores.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list.into_iter().take(5).map(|(id, _)| id).collect()
}

/// 从 Mixin 报错里取配置名，最多 limit 个
///
/// 只认报错上下文里的配置名：正常启动日志里每个模组都会列一遍自己的 Mixin 配置，
/// 全收进来会把 details 撑到几百条。
fn mixin_configs(scan: &Scan, limit: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for (line, low) in scan.raw.lines().zip(scan.lower.lines()) {
        if !low.contains("mixin") {
            continue;
        }
        let context = [
            "apply",
            "failed",
            "failure",
            "invalid",
            "does not specify",
            "critical",
            "transformer",
            "injection",
        ]
        .iter()
        .any(|needle| low.contains(needle));
        if !context {
            continue;
        }

        for captures in RE_JSON_NAME.captures_iter(line) {
            let name = captures[1].to_string();
            if !name.to_lowercase().contains("mixin") || out.contains(&name) {
                continue;
            }
            out.push(name);
            if out.len() >= limit {
                return out;
            }
        }
    }

    // 兜底：异常与栈帧里的 config.json:Class
    for captures in RE_JSON_NAME.captures_iter(&scan.detail_lower) {
        let name = captures[1].to_string();
        if name.to_lowercase().contains("mixin") && !out.contains(&name) {
            out.push(name);
            if out.len() >= limit {
                break;
            }
        }
    }

    out
}

/// 从 Mixin 配置名推出模组 id
///
/// 支持 modid.mixins.json、modid.client.mixins.json、mixins.modid.json
/// 与 modid.mixin.json 这几种命名。
///
/// - name: 配置文件名
///
/// # 返回值
///
/// 推不出时返回 None
fn mod_from_mixin_config(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let base = lower.strip_suffix(".json")?;

    let prefix = if let Some(prefix) = base.strip_suffix(".mixins") {
        prefix
    } else if let Some(prefix) = base.strip_suffix(".mixin") {
        prefix
    } else if let Some(prefix) = base.strip_prefix("mixins.") {
        prefix
    } else {
        return None;
    };

    let prefix = prefix
        .strip_suffix(".client")
        .or_else(|| prefix.strip_suffix(".common"))
        .or_else(|| prefix.strip_suffix(".server"))
        .unwrap_or(prefix)
        .trim_matches('.');

    if prefix.is_empty() {
        None
    } else {
        Some(prefix.to_string())
    }
}

/// 取报错里的 Mixin 类名
fn mixin_class(text: &str) -> Option<String> {
    RE_MIXIN_CLASS
        .captures_iter(text)
        .find(|captures| captures[1].to_lowercase().contains("mixin"))
        .map(|captures| captures[2].to_string())
}

/// 模组入口点执行失败
fn check_entrypoint(scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "could not execute entrypoint stage",
        "entrypoint stage",
        "failed to execute entrypoint",
    ];
    if !needles
        .iter()
        .any(|needle| scan.detail_lower.contains(needle))
    {
        return;
    }

    let mut mods = Vec::new();
    for captures in RE_PROVIDED_BY.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_FROM_MOD.captures_iter(scan.raw) {
        push_mod(&mut mods, &captures[1]);
    }

    out.push(Finding {
        id: FindingId::ModEntrypointFailure,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&needles, 6),
        mods,
        details: Vec::new(),
    });
}

/// 找不到类
fn check_missing_class(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    if !chain_has(report, "NoClassDefFoundError") && !chain_has(report, "ClassNotFoundException") {
        return;
    }

    let mut details = Vec::new();
    for frame in frames(report).filter(|frame| {
        frame.class.ends_with("NoClassDefFoundError")
            || frame.class.ends_with("ClassNotFoundException")
    }) {
        if !frame.message.is_empty() {
            push_detail(&mut details, clip(&frame.message, 240));
        }
    }

    out.push(Finding {
        id: FindingId::MissingClass,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(&["noclassdeffounderror", "classnotfoundexception"], 6),
        mods: Vec::new(),
        details,
    });
}

/// 找不到方法或字段
fn check_missing_method(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let hit = frames(report).any(|frame| {
        frame.class.ends_with("NoSuchMethodError")
            || frame.class.ends_with("NoSuchFieldError")
            || frame.class.ends_with("NoSuchMethodException")
    });
    if !hit {
        return;
    }

    let mut details = Vec::new();
    for frame in frames(report).filter(|frame| {
        frame.class.ends_with("NoSuchMethodError")
            || frame.class.ends_with("NoSuchFieldError")
            || frame.class.ends_with("NoSuchMethodException")
    }) {
        if !frame.message.is_empty() {
            push_detail(&mut details, clip(&frame.message, 240));
        }
    }

    out.push(Finding {
        id: FindingId::MissingMethod,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(
            &[
                "nosuchmethoderror",
                "nosuchfielderror",
                "nosuchmethodexception",
            ],
            6,
        ),
        mods: Vec::new(),
        details,
    });
}

/// 图形驱动或 OpenGL 上下文
fn check_graphics(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let hard = [
        "pixel format not accelerated",
        "failed to create display",
        "failed to initialize glfw",
        "could not create opengl context",
    ];
    let soft = [
        "glfw error",
        "opengl error",
        "wglcreatecontext",
        "org.lwjgl.opengl.glexception",
    ];

    let high = hard.iter().any(|needle| scan.detail_lower.contains(needle));
    let hit = high
        || chain_has(report, "GLException")
        || chain_has(report, "LWJGLException")
        || soft.iter().any(|needle| scan.lower.contains(needle));
    if !hit {
        return;
    }

    let mut needles = hard.to_vec();
    needles.extend(soft);
    out.push(Finding {
        id: FindingId::GraphicsDriver,
        severity: Severity::Error,
        confidence: if high {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence: scan.evidence(&needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 本地库缺失或架构不匹配
fn check_missing_native(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "no lwjgl in java.library.path",
        "failed to load a library",
        "could not load native",
    ];
    if !chain_has(report, "UnsatisfiedLinkError")
        && !needles
            .iter()
            .any(|needle| scan.detail_lower.contains(needle))
    {
        return;
    }

    out.push(Finding {
        id: FindingId::MissingNative,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&["unsatisfiedlinkerror", "java.library.path"], 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 文件访问被拒绝
fn check_access_denied(scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "accessdeniedexception",
        "access is denied",
        "permission denied",
        "拒绝访问",
    ];
    if !needles.iter().any(|needle| scan.lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::AccessDenied,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(&needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 存档或区域文件损坏
fn check_world(scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "region file",
        "chunk file at",
        "corrupted chunk",
        "world corrupted",
        "failed to load chunk",
    ];
    if !needles.iter().any(|needle| scan.lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::CorruptWorld,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(&needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// jar 或类文件损坏
fn check_corrupt_file(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "invalid loc header",
        "invalid cen header",
        "crc32",
        "invalid jar",
        "corrupt jar",
    ];
    if !chain_has(report, "ZipException")
        && !chain_has(report, "ClassFormatError")
        && !chain_has(report, "VerifyError")
        && !needles
            .iter()
            .any(|needle| scan.detail_lower.contains(needle))
    {
        return;
    }

    out.push(Finding {
        id: FindingId::CorruptFile,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(
            &[
                "zipexception",
                "classformaterror",
                "verifyerror",
                "invalid loc header",
                "corrupt jar",
            ],
            6,
        ),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 渲染或区块构建崩溃
fn check_render(scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "illegalstateexception: not building!",
        "chunk build failed",
        "pose stack not empty",
        "exception while building chunk meshes",
    ];
    if !needles
        .iter()
        .any(|needle| scan.detail_lower.contains(needle))
    {
        return;
    }

    out.push(Finding {
        id: FindingId::RenderFailure,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(&needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 维度注册缺失
fn check_dimension(scan: &Scan, out: &mut Vec<Finding>) {
    if !scan
        .detail_lower
        .contains("could not get provider type for dimension")
    {
        return;
    }

    out.push(Finding {
        id: FindingId::DimensionMissing,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: scan.evidence(&["could not get provider type for dimension"], 4),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 模组初始化失败
fn check_mod_init(scan: &Scan, has_finding: bool, out: &mut Vec<Finding>) {
    // 已经命中更具体的原因时不再补一条泛泛的"模组加载失败"
    if has_finding {
        return;
    }

    let needles = [
        "exceptionininitializererror",
        "failed to load mods",
        "modloadingexception",
        "error during initialization",
        "error during pre-loading",
        "failed to create mod",
        "mod loading failed",
        "has failed to load correctly",
    ];
    if !needles.iter().any(|needle| scan.lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::ModInitFailure,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: scan.evidence(&needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 安全管理器或权限问题
fn check_security(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    if !chain_has(report, "SecurityException") {
        return;
    }

    out.push(Finding {
        id: FindingId::Security,
        severity: Severity::Warning,
        confidence: Confidence::Low,
        evidence: scan.evidence(&["securityexception"], 4),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 取包含任一关键词的原始行
///
/// lower 与 raw 行序一致，由解析阶段算好，这里只做逐行比较。
///
/// - raw: 原文
/// - lower: 原文的小写副本
/// - needles: 小写关键词
/// - limit: 最多保留几行
///
/// # 返回值
///
/// 去掉首尾空白并截断后的行
fn evidence(raw: &str, lower: &str, needles: &[&str], limit: usize) -> Vec<String> {
    let mut out = Vec::new();

    for (line, low) in raw.lines().zip(lower.lines()) {
        let trimmed = line.trim();
        if trimmed.is_empty() || low.contains("added class metadata for") {
            continue;
        }
        if needles.iter().any(|needle| low.contains(needle)) {
            out.push(clip(trimmed, 240));
            if out.len() >= limit {
                break;
            }
        }
    }

    out
}

/// 把文本截到指定字符数
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }

    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// 从 jar 文件名取模组 id，如 indium-1.0.35+mc1.21.jar → indium
fn jar_mod_id(file: &str) -> Option<String> {
    let bytes = file.as_bytes();
    for index in 1..bytes.len() {
        if bytes[index - 1] == b'-' && bytes[index].is_ascii_digit() {
            let id = &file[..index - 1];
            return if id.is_empty() {
                None
            } else {
                Some(id.to_string())
            };
        }
    }
    None
}

/// 追加模组 id，忽略空值与重复
fn push_mod(mods: &mut Vec<String>, id: &str) {
    let id = id.trim().trim_end_matches([',', '.', ';', ')', ':']);
    if id.is_empty() || mods.iter().any(|item| item.eq_ignore_ascii_case(id)) {
        return;
    }
    mods.push(id.to_string());
}

/// 追加细节，忽略重复
fn push_detail(details: &mut Vec<String>, line: String) {
    if !details.contains(&line) {
        details.push(line);
    }
}

/// 汇总可疑模组
///
/// 权重从高到低：异常消息里直接出现的模组 id、结论点名的模组（按顺序递减）、
/// 栈帧包名与正文里出现的模组 id。栈帧命中每个模组最多算三次，
/// 免得一个到处出现的库把真正被点名的模组挤下去。
fn collect_suspected(report: &CrashReport, findings: &[Finding], detail_lower: &str) -> Vec<String> {
    let mut score: HashMap<String, u32> = HashMap::new();

    // 依赖类结论可能一次列几十个模组，全给同样的权重会让排头变成字母序的偶然结果
    for finding in findings {
        for (index, id) in finding.mods.iter().enumerate() {
            let weight = match index {
                0 => 6,
                1 => 4,
                2 => 3,
                _ => 0,
            };
            if weight > 0 {
                *score.entry(id.clone()).or_default() += weight;
            }
        }
    }

    let known: HashMap<String, &str> = report
        .environment
        .mods
        .iter()
        .map(|entry| (entry.id.to_lowercase(), entry.id.as_str()))
        .collect();

    // 异常消息里直接点名的模组最可信
    let message = report
        .exception
        .iter()
        .chain(report.causes.iter())
        .map(|frame| frame.line().to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    for entry in &report.environment.mods {
        let id = entry.id.to_lowercase();
        if id.len() >= 4 && message.contains(&id) {
            *score.entry(entry.id.clone()).or_default() += 6;
        }
    }

    // 包名与模组 id 用下划线 / 连字符分隔时（world_preview 对 world.preview），
    // 精确分段对不上，归一化后再比一次
    let dotted: Vec<(&str, String)> = report
        .environment
        .mods
        .iter()
        .filter(|entry| {
            entry.id.len() >= 4 && (entry.id.contains('_') || entry.id.contains('-'))
        })
        .map(|entry| {
            (
                entry.id.as_str(),
                entry.id.to_lowercase().replace(['_', '-'], "."),
            )
        })
        .collect();

    let mut frame_score: HashMap<String, u32> = HashMap::new();
    for line in report.detail.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("at ") else {
            continue;
        };
        let class = rest.split('(').next().unwrap_or(rest).to_lowercase();
        for segment in class.split(['.', '/', '$']) {
            // NeoForge 的栈帧写成 TRANSFORMER/modid@版本/包名，版本要剥掉
            let segment = segment.split('@').next().unwrap_or(segment);
            if let Some(id) = known.get(segment) {
                let entry = frame_score.entry(id.to_string()).or_default();
                if *entry < 3 {
                    *entry += 1;
                }
            }
        }
        for (id, pattern) in &dotted {
            if class.contains(pattern) {
                let entry = frame_score.entry((*id).to_string()).or_default();
                if *entry < 3 {
                    *entry += 1;
                }
            }
        }
    }
    for (id, hits) in frame_score {
        *score.entry(id).or_default() += hits;
    }

    for entry in &report.environment.mods {
        let id = entry.id.to_lowercase();
        if id.len() >= 4 && detail_lower.contains(&id) {
            *score.entry(entry.id.clone()).or_default() += 1;
        }
    }

    let mut list: Vec<(String, u32)> = score
        .into_iter()
        .filter(|(id, _)| !is_loader_mod(id))
        .collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list.into_iter().take(8).map(|(id, _)| id).collect()
}

/// 加载器与游戏本身不算可疑模组
fn is_loader_mod(id: &str) -> bool {
    matches!(
        id.to_lowercase().as_str(),
        "minecraft"
            | "mcp"
            | "fml"
            | "forge"
            | "neoforge"
            | "fabricloader"
            | "quilt_loader"
            | "java"
            | "mixin"
    ) || id.eq_ignore_ascii_case("mixinextras")
}
