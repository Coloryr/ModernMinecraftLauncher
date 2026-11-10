//! 图形规则
//!
//! 显卡驱动、OpenGL 与渲染相关报错。

use crate::model::{Confidence, CrashReport, Finding, FindingId, Severity};

use super::*;

/// 图形驱动或 OpenGL 上下文
pub(super) fn check_graphics(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
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

/// 渲染或区块构建崩溃
pub(super) fn check_render(scan: &Scan, out: &mut Vec<Finding>) {
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
