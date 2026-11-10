//! 依赖规则
//!
//! 缺前置、版本不符、加载器装错与重复安装。

use crate::model::{Confidence, Finding, FindingId, Severity};

use super::*;

/// 模组缺少前置，或依赖的版本不满足
pub(super) fn check_missing_dependency(scan: &Scan, out: &mut Vec<Finding>) {
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
            push_detail(
                &mut details,
                format!("required_version={}", version.as_str()),
            );
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
pub(super) fn check_incompatible(scan: &Scan, dependency_issue: bool, out: &mut Vec<Finding>) {
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
pub(super) fn check_wrong_loader(scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_duplicate(scan: &Scan, out: &mut Vec<Finding>) {
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
