//! 存档与权限规则
//!
//! 存档损坏、维度缺失、访问被拒与安全类拦截。

use crate::model::{Confidence, CrashReport, Finding, FindingId, Severity};

use super::*;

/// 文件访问被拒绝
pub(super) fn check_access_denied(scan: &Scan, out: &mut Vec<Finding>) {
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
pub(super) fn check_world(scan: &Scan, out: &mut Vec<Finding>) {
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

/// 压缩包结构损坏，不用类校验或字节码格式异常推断磁盘文件损坏。
pub(super) fn check_corrupt_file(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "invalid loc header",
        "invalid cen header",
        "invalid entry crc",
        "crc mismatch",
        "invalid jar",
        "corrupt jar",
    ];
    if !chain_has(report, "ZipException")
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
                "invalid loc header",
                "invalid cen header",
                "invalid entry crc",
                "crc mismatch",
                "corrupt jar",
            ],
            6,
        ),
        mods: Vec::new(),
        details: Vec::new(),
    });
}

/// 维度注册缺失
pub(super) fn check_dimension(scan: &Scan, out: &mut Vec<Finding>) {
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

/// 报告把模组标成 Errored
///
/// Forge 的模组列表用状态字母记每个模组的进度，末尾带 `E` 表示加载器在它身上捕获到异常。
/// 这是"哪个模组出的问题"最直接的证据，比按栈帧包名推断可靠。
pub(super) fn check_errored_mods(report: &CrashReport, out: &mut Vec<Finding>) {
    let errored: Vec<_> = report
        .environment
        .mods
        .iter()
        .filter(|entry| entry.errored())
        .collect();
    if errored.is_empty() {
        return;
    }

    out.push(Finding {
        id: FindingId::ModErrored,
        severity: Severity::Error,
        confidence: Confidence::High,
        evidence: errored
            .iter()
            .map(|entry| clip(&entry.line, 240))
            .filter(|line| !line.is_empty())
            .take(6)
            .collect(),
        mods: errored
            .iter()
            .take(8)
            .map(|entry| entry.id.clone())
            .collect(),
        details: Vec::new(),
    });
}

/// 模组初始化失败
pub(super) fn check_mod_init(scan: &Scan, has_finding: bool, out: &mut Vec<Finding>) {
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
pub(super) fn check_security(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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
