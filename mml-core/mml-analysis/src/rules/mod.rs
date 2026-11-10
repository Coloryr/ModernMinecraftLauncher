//! 崩溃原因规则
//!
//! 规则按"越具体越靠前"的顺序匹配，第一条即主因。所有规则只读解析结果与原文，
//! 命中后给出稳定 id、证据行与机器可读细节，不生成面向用户的句子。
//!
//! 异常类相关的规则一律以顶层异常与 Caused by 链为准，不拿整份原文做子串匹配：
//! 崩溃报告里常夹着完整启动日志，按原文匹配会把日志里的 LWJGLException、
//! ZipException 这类类名当成崩溃原因。

use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Confidence, CrashAnalysis, CrashReport, Finding, FindingId, Severity};

mod class;
mod deps;
mod graphics;
mod jvm;
mod mixin;
mod runtime;
mod suspect;
mod world;

use self::class::*;
use self::deps::*;
use self::graphics::*;
use self::jvm::*;
use self::mixin::*;
use self::suspect::*;
use self::world::*;

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
static RE_WRONG_LOADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(fabric|forge|neoforge|quilt) mod and cannot be loaded").unwrap()
});

/// 报错里被点名的 jar 文件名，如 mods\indium-1.0.35+mc1.21.jar
static RE_JAR_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([A-Za-z0-9_.+]+-[0-9][A-Za-z0-9_.+-]*)\.jar").unwrap());

/// JVM 致命错误暂不分析；规则保留，后续接上时改为 true
const ANALYZE_JVM_FATAL: bool = false;

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
    // 加载器点名"这些模组出错"比按异常形态泛泛归类有用，排在通用异常规则之前
    check_errored_mods(report, &mut findings);
    runtime::check(report, &scan, &mut findings);
    check_missing_class(report, &scan, &mut findings);
    check_missing_method(report, &scan, &mut findings);
    check_entrypoint(&scan, &mut findings);
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

    runtime::enrich(report, &mut findings);
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

fn chain_has(report: &CrashReport, class: &str) -> bool {
    report
        .exception
        .iter()
        .chain(report.causes.iter())
        .any(|frame| frame.class.ends_with(class))
}

/// 取顶层异常与 Caused by 链
fn frames(report: &CrashReport) -> impl DoubleEndedIterator<Item = &crate::model::ExceptionFrame> {
    report.exception.iter().chain(report.causes.iter())
}

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
