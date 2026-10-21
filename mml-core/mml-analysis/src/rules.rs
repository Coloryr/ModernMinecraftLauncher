//! 崩溃原因规则
//!
//! 规则按"越具体越靠前"的顺序匹配，第一条即主因。所有规则只读解析结果与原文，
//! 命中后给出稳定 id、证据行与机器可读细节，不生成面向用户的句子。

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Confidence, CrashAnalysis, CrashReport, Finding, FindingId, Severity};

/// 类文件版本，如 `class file version 65.0`
static RE_CLASS_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)class file version (\d+)").unwrap());

/// 任意 `.json` 文件名，用来从 Mixin 报错里挑配置名
static RE_JSON_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z0-9_.-]+\.json)").unwrap());

/// Forge 缺前置行里的 `Mod ID: 'x'`
static RE_MOD_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)mod id: '([^']+)'").unwrap());

/// Forge 缺前置行里的 `Requested by: 'x'`
static RE_REQUESTED_BY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)requested by: '([^']+)'").unwrap());

/// Fabric 报错里的 `Mod 'x'`
static RE_MOD_QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)mod '([^']+)'").unwrap());

/// Fabric 报错里的 `of mod 'x'`
static RE_OF_MOD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)of mod '([^']+)'").unwrap());

/// Fabric 入口点报错里的 `provided by 'x'`
static RE_PROVIDED_BY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)provided by '([^']+)'").unwrap());

/// Mixin 报错里的 `from mod x`
static RE_FROM_MOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)from mod ([A-Za-z0-9_.-]+)").unwrap());

/// Fabric 的 `Mixin apply for mod x failed`
static RE_MIXIN_APPLY_MOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)mixin apply for mod ([A-Za-z0-9_.-]+) failed").unwrap());

/// `Mixin config x.json from mod y`
static RE_MIXIN_CONFIG_FROM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)mixin config ([A-Za-z0-9_.-]+\.json) from mod ([A-Za-z0-9_.-]+)").unwrap()
});

/// `config.json:ClassName`
static RE_MIXIN_CLASS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z0-9_.-]+\.json):([A-Za-z0-9_$.]+)").unwrap());

/// JVM 致命错误暂不分析；规则保留，后续接上时改为 `true`
const ANALYZE_JVM_FATAL: bool = false;

/// 按规则分析一份解析结果
///
/// 只分析游戏日志与 crash-report；JVM 致命错误日志与认不出类型的文本直接返回空结论。
///
/// - `report`: 解析后的崩溃报告或日志
///
/// # 返回值
///
/// 见 [`CrashAnalysis`]
pub(crate) fn analyze(report: &CrashReport) -> CrashAnalysis {
    let raw = report.raw();
    let lower = raw.to_lowercase();
    let detail_lower = report.detail.to_lowercase();
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
        check_jvm_fatal(report, &lower, raw, &mut findings);
    }
    check_out_of_memory(report, &detail_lower, raw, &mut findings);
    check_java_version(&detail_lower, raw, &mut findings);
    check_missing_dependency(&lower, raw, &mut findings);
    let missing_dependency = findings
        .iter()
        .any(|finding| finding.id == FindingId::MissingDependency);
    check_incompatible(&lower, raw, missing_dependency, &mut findings);
    check_duplicate(&lower, raw, &mut findings);
    check_mixin(&lower, raw, report, &mut findings);
    check_entrypoint(&lower, raw, &mut findings);
    check_missing_class(&detail_lower, report, &mut findings);
    check_missing_method(&detail_lower, report, &mut findings);
    check_graphics(&lower, raw, &mut findings);
    check_missing_native(&detail_lower, raw, &mut findings);
    check_access_denied(&lower, raw, &mut findings);
    check_world(&lower, raw, &mut findings);
    check_corrupt_file(&lower, raw, &mut findings);
    check_stack_overflow(&detail_lower, raw, &mut findings);
    let has_finding = !findings.is_empty();
    check_mod_init(&lower, raw, has_finding, &mut findings);
    check_security(&detail_lower, raw, &mut findings);

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

    let suspected_mods = collect_suspected(report, &findings, &detail_lower);

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

/// JVM 致命错误
fn check_jvm_fatal(report: &CrashReport, lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let hit = report.kind == crate::model::LogKind::JvmFatal
        || lower.contains("a fatal error has been detected")
        || lower.contains("exception_access_violation");
    if !hit {
        return;
    }

    let details = raw
        .lines()
        .find(|line| line.contains("Problematic frame:"))
        .map(|line| vec![clip(line.trim(), 240)])
        .unwrap_or_default();

    out.push(Finding {
        id: FindingId::JvmFatal,
        severity: Severity::Fatal,
        confidence: Confidence::High,
        evidence: evidence(
            raw,
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
fn check_out_of_memory(
    report: &CrashReport,
    detail_lower: &str,
    raw: &str,
    out: &mut Vec<Finding>,
) {
    let rules = [
        (
            FindingId::OutOfMemoryHeap,
            "outofmemoryerror: java heap space",
        ),
        (
            FindingId::OutOfMemoryMetaspace,
            "outofmemoryerror: metaspace",
        ),
        (
            FindingId::OutOfMemoryMetaspace,
            "outofmemoryerror: compressed class space",
        ),
        (
            FindingId::OutOfMemoryDirect,
            "outofmemoryerror: direct buffer memory",
        ),
        (
            FindingId::OutOfMemoryGc,
            "outofmemoryerror: gc overhead limit exceeded",
        ),
        (
            FindingId::OutOfMemoryNativeThread,
            "outofmemoryerror: unable to create native thread",
        ),
    ];

    let specific = rules
        .iter()
        .find(|(_, needle)| detail_lower.contains(needle))
        .map(|(id, _)| *id);
    let generic = detail_lower.contains("outofmemoryerror");

    let Some(id) = specific.or(if generic {
        Some(FindingId::OutOfMemoryHeap)
    } else {
        None
    }) else {
        return;
    };

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
        evidence: evidence(
            raw,
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
fn check_java_version(detail_lower: &str, raw: &str, out: &mut Vec<Finding>) {
    if !detail_lower.contains("unsupportedclassversionerror") {
        return;
    }

    let mut details = Vec::new();
    if let Some(captures) = RE_CLASS_VERSION.captures(raw)
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
        evidence: evidence(
            raw,
            &["unsupportedclassversionerror", "class file version"],
            4,
        ),
        mods: Vec::new(),
        details,
    });
}

/// 模组缺少前置
fn check_missing_dependency(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "missing or unsupported mandatory dependencies",
        "unmet dependency",
        "mod resolution encountered an error",
        "which is missing",
        "requires version",
        "requires any version",
        "requires mod",
        "is missing!",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    let mut mods = Vec::new();
    for captures in RE_MOD_ID.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_REQUESTED_BY.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_OF_MOD.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_MOD_QUOTED.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }

    let details = evidence(raw, &["mod id:", "requires", "missing"], 8);
    out.push(Finding {
        id: FindingId::MissingDependency,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: evidence(
            raw,
            &[
                "missing or unsupported mandatory dependencies",
                "unmet dependency",
                "which is missing",
                "requires version",
                "requires mod",
            ],
            8,
        ),
        mods,
        details,
    });
}

/// 模组之间不兼容，缺少前置时不再重复报
fn check_incompatible(lower: &str, raw: &str, missing_dependency: bool, out: &mut Vec<Finding>) {
    if missing_dependency {
        return;
    }

    let needles = [
        "incompatible mod set",
        "incompatible with the game or each other",
        "mod resolution failed",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    let mut mods = Vec::new();
    for captures in RE_MOD_QUOTED.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }

    out.push(Finding {
        id: FindingId::IncompatibleModSet,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: evidence(raw, &needles, 8),
        mods,
        details: Vec::new(),
    });
}

/// 同一个模组装了多份
fn check_duplicate(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "duplicate mods",
        "duplicate mod",
        "already present",
        "mods share id",
        "duplicate key",
        "duplicate entry",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    let fatal = lower.contains("duplicate mods found") || lower.contains("mods share id");
    let mut mods = Vec::new();
    for captures in RE_MOD_QUOTED.captures_iter(raw) {
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
        evidence: evidence(raw, &needles, 6),
        mods,
        details: Vec::new(),
    });
}

/// Mixin 注入失败
fn check_mixin(lower: &str, raw: &str, report: &CrashReport, out: &mut Vec<Finding>) {
    let needles = [
        "mixin apply failed",
        "mixin apply for mod",
        "mixintransformererror",
        "invalidmixinexception",
        "mixin transformation of",
        "org.spongepowered.asm.mixin.transformer",
        "mixin.injection.throwables",
        "critical injection failure",
    ];
    // "mixin config" 本身在正常启动日志里也可能出现，必须配上错误措辞才算
    let config_error = lower.contains("mixin config")
        && (lower.contains("does not specify")
            || lower.contains("could not be")
            || lower.contains("failed to")
            || lower.contains("is missing")
            || lower.contains("invalid"));
    if !needles.iter().any(|needle| lower.contains(needle)) && !config_error {
        return;
    }

    let suspects = mixin_suspects(report, raw);
    let strong = RE_FROM_MOD.is_match(raw)
        || RE_MIXIN_APPLY_MOD.is_match(raw)
        || RE_MIXIN_CONFIG_FROM.is_match(raw);

    let mut details: Vec<String> = Vec::new();
    for captures in RE_JSON_NAME.captures_iter(raw) {
        let name = captures[1].to_string();
        if name.to_lowercase().contains("mixin") {
            let line = format!("mixin_config={name}");
            if !details.contains(&line) {
                details.push(line);
            }
        }
    }
    if let Some(class) = mixin_class(raw) {
        details.push(format!("mixin_class={class}"));
    }
    for id in suspects.iter().take(3) {
        details.push(format!("suspect_mod={id}"));
    }

    out.push(Finding {
        id: FindingId::MixinFailure,
        severity: Severity::Error,
        confidence: if strong || !suspects.is_empty() {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence: evidence(raw, &needles, 8),
        mods: suspects,
        details,
    });
}

/// 从 Mixin 报错里挑出最可能出错的模组
///
/// 依据依次是显式的 from mod、Mixin 配置名、栈帧包名与报错里出现的模组 id。
///
/// - `report`: 解析结果，模组列表用来把配置名 / 包名对回模组 id
/// - `raw`: 原文
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
        } else {
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

/// 从 Mixin 配置名推出模组 id
///
/// 支持 modid.mixins.json、modid.client.mixins.json、mixins.modid.json
/// 与 modid.mixin.json 这几种命名。
///
/// - `name`: 配置文件名
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
fn mixin_class(raw: &str) -> Option<String> {
    RE_MIXIN_CLASS
        .captures_iter(raw)
        .find(|captures| captures[1].to_lowercase().contains("mixin"))
        .map(|captures| captures[2].to_string())
}

/// 模组入口点执行失败
fn check_entrypoint(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "could not execute entrypoint stage",
        "entrypoint stage",
        "failed to execute entrypoint",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    let mut mods = Vec::new();
    for captures in RE_PROVIDED_BY.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }
    for captures in RE_FROM_MOD.captures_iter(raw) {
        push_mod(&mut mods, &captures[1]);
    }

    out.push(Finding {
        id: FindingId::ModEntrypointFailure,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: evidence(raw, &needles, 6),
        mods,
        details: Vec::new(),
    });
}

/// 找不到类
fn check_missing_class(detail_lower: &str, report: &CrashReport, out: &mut Vec<Finding>) {
    if !detail_lower.contains("noclassdeffounderror")
        && !detail_lower.contains("classnotfoundexception")
    {
        return;
    }

    let mut details = Vec::new();
    for frame in report
        .exception
        .iter()
        .chain(report.causes.iter())
        .filter(|frame| {
            frame.class.ends_with("NoClassDefFoundError")
                || frame.class.ends_with("ClassNotFoundException")
        })
    {
        if !frame.message.is_empty() {
            details.push(clip(&frame.message, 240));
        }
    }

    out.push(Finding {
        id: FindingId::MissingClass,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(
            report.raw(),
            &["noclassdeffounderror", "classnotfoundexception"],
            6,
        ),
        mods: Vec::new(),
        details,
    });
}

/// 找不到方法或字段
fn check_missing_method(detail_lower: &str, report: &CrashReport, out: &mut Vec<Finding>) {
    let needles = [
        "nosuchmethoderror",
        "nosuchfielderror",
        "nosuchmethodexception",
    ];
    if !needles.iter().any(|needle| detail_lower.contains(needle)) {
        return;
    }

    let mut details = Vec::new();
    for frame in report
        .exception
        .iter()
        .chain(report.causes.iter())
        .filter(|frame| {
            frame.class.ends_with("NoSuchMethodError")
                || frame.class.ends_with("NoSuchFieldError")
                || frame.class.ends_with("NoSuchMethodException")
        })
    {
        if !frame.message.is_empty() {
            details.push(clip(&frame.message, 240));
        }
    }

    out.push(Finding {
        id: FindingId::MissingMethod,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(report.raw(), &needles, 6),
        mods: Vec::new(),
        details,
    });
}

/// 图形驱动或 OpenGL 上下文
fn check_graphics(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let hard = [
        "pixel format not accelerated",
        "failed to create display",
        "failed to initialize glfw",
        "could not create opengl context",
    ];
    let soft = [
        "glfw error",
        "opengl error",
        "glexception",
        "wglcreatecontext",
        "org.lwjgl.opengl.glexception",
    ];

    let high = hard.iter().any(|needle| lower.contains(needle));
    let hit = high || soft.iter().any(|needle| lower.contains(needle));
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
        evidence: evidence(raw, &needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 本地库缺失或架构不匹配
fn check_missing_native(detail_lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "unsatisfiedlinkerror",
        "no lwjgl in java.library.path",
        "failed to load a library",
        "could not load native",
    ];
    if !needles.iter().any(|needle| detail_lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::MissingNative,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: evidence(raw, &needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 文件访问被拒绝
fn check_access_denied(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "accessdeniedexception",
        "access is denied",
        "permission denied",
        "拒绝访问",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::AccessDenied,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(raw, &needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 存档或区域文件损坏
fn check_world(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "region file",
        "chunk file at",
        "corrupted chunk",
        "world corrupted",
        "failed to load chunk",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::CorruptWorld,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(raw, &needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// jar 或类文件损坏
fn check_corrupt_file(lower: &str, raw: &str, out: &mut Vec<Finding>) {
    let needles = [
        "zipexception",
        "invalid loc header",
        "invalid cen header",
        "classformaterror",
        "verifyerror",
        "crc32",
        "invalid jar",
        "corrupt jar",
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::CorruptFile,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(raw, &needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 栈溢出
fn check_stack_overflow(detail_lower: &str, raw: &str, out: &mut Vec<Finding>) {
    if !detail_lower.contains("stackoverflowerror") {
        return;
    }

    out.push(Finding {
        id: FindingId::StackOverflow,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(raw, &["stackoverflowerror"], 4),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 模组初始化失败
fn check_mod_init(lower: &str, raw: &str, has_finding: bool, out: &mut Vec<Finding>) {
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
    ];
    if !needles.iter().any(|needle| lower.contains(needle)) {
        return;
    }

    out.push(Finding {
        id: FindingId::ModInitFailure,
        severity: Severity::Error,
        confidence: Confidence::Medium,
        evidence: evidence(raw, &needles, 6),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 安全管理器或权限问题
fn check_security(detail_lower: &str, raw: &str, out: &mut Vec<Finding>) {
    if !detail_lower.contains("securityexception") {
        return;
    }

    out.push(Finding {
        id: FindingId::Security,
        severity: Severity::Warning,
        confidence: Confidence::Low,
        evidence: evidence(raw, &["securityexception"], 4),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 取包含任一关键词的原始行
///
/// - `raw`: 原文
/// - `needles`: 小写关键词
/// - `limit`: 最多保留几行
///
/// # 返回值
///
/// 去掉首尾空白并截断后的行
fn evidence(raw: &str, needles: &[&str], limit: usize) -> Vec<String> {
    let mut out = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if needles.iter().any(|needle| lower.contains(needle)) {
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

/// 追加模组 id，忽略空值与重复
fn push_mod(mods: &mut Vec<String>, id: &str) {
    let id = id.trim().trim_end_matches([',', '.', ';', ')', ':']);
    if id.is_empty() || mods.iter().any(|item| item.eq_ignore_ascii_case(id)) {
        return;
    }
    mods.push(id.to_string());
}

/// 汇总可疑模组
///
/// 结论里点名的模组权重最高，其次看栈帧包名与异常消息里出现的模组 id。
fn collect_suspected(
    report: &CrashReport,
    findings: &[Finding],
    detail_lower: &str,
) -> Vec<String> {
    let mut score: HashMap<String, u32> = HashMap::new();

    for finding in findings {
        for id in &finding.mods {
            *score.entry(id.clone()).or_default() += 4;
        }
    }

    let known: HashMap<String, &str> = report
        .environment
        .mods
        .iter()
        .map(|entry| (entry.id.to_lowercase(), entry.id.as_str()))
        .collect();

    for line in report.detail.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("at ") else {
            continue;
        };
        let class = rest.split('(').next().unwrap_or(rest);
        for segment in class.split(['.', '/', '$']) {
            if let Some(id) = known.get(&segment.to_lowercase()) {
                *score.entry(id.to_string()).or_default() += 1;
            }
        }
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
        "minecraft" | "forge" | "neoforge" | "fabricloader" | "quilt_loader" | "java" | "mixin"
    ) || id.eq_ignore_ascii_case("mixinextras")
}
