//! JVM 层规则
//!
//! 内存不足、Java 版本、栈溢出与看门狗超时。

use crate::model::{Confidence, CrashReport, Finding, FindingId, Severity};

use super::*;

/// JVM 致命错误
pub(super) fn check_jvm_fatal(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_out_of_memory(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_java_version(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_stack_overflow(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_watchdog(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
        evidence: scan.evidence(
            &["watchdog", "single server tick took", "watching server"],
            6,
        ),
        mods: Vec::new(),
        details: Vec::new(),
    });
}
