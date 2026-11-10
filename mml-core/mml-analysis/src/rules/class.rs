//! 类与方法规则
//!
//! 入口点缺失、类找不到、方法签名不符与原生库缺失。

use crate::model::{Confidence, CrashReport, Finding, FindingId, Severity};

use super::*;

/// 模组入口点执行失败
pub(super) fn check_entrypoint(scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_missing_class(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    if out.iter().any(|finding| {
        matches!(
            finding.id,
            FindingId::ClassInitialization | FindingId::ClientOnlyClass
        )
    }) {
        return;
    }
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
pub(super) fn check_missing_method(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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

/// 本地库缺失或架构不匹配
pub(super) fn check_missing_native(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
