//! 运行时异常的细分，以及本次故障的阶段、调用位置和排查线索。

use super::{Scan, clip, frames, push_detail};
use crate::model::{Confidence, CrashReport, Finding, FindingId, Severity};
use std::collections::HashMap;

/// 先识别具体异常，再保留入口点失败等外层现象。
pub(super) fn check(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    for frame in frames(report).rev() {
        let class = frame.class.rsplit('.').next().unwrap_or(&frame.class);
        let message = &frame.message;
        let id = match class {
            "VerifyError" => FindingId::BytecodeVerification,
            "ClassFormatError" => FindingId::ClassFormat,
            "ClassCastException" => FindingId::ClassCast,
            "NullPointerException" => FindingId::NullPointer,
            "ArithmeticException" => FindingId::Arithmetic,
            "IndexOutOfBoundsException"
            | "ArrayIndexOutOfBoundsException"
            | "StringIndexOutOfBoundsException" => FindingId::IndexOutOfBounds,
            "NoClassDefFoundError" if message.starts_with("Could not initialize class ") => {
                FindingId::ClassInitialization
            }
            "NoClassDefFoundError" | "ClassNotFoundException"
                if report.environment.dedicated_server && is_client_class(report, message) =>
            {
                FindingId::ClientOnlyClass
            }
            "IllegalStateException"
                if message.contains("Trying to return to in-game GUI during disconnection") =>
            {
                FindingId::DisconnectedScreen
            }
            // 有些 JVM 只把 VerifyError 放在 InternalError 的消息里，不另列 Caused by。
            "InternalError" if message.starts_with("java.lang.VerifyError:") => {
                FindingId::BytecodeVerification
            }
            // 更深层的链接错误由专门规则处理，外层运行时异常不能遮住它。
            "NoSuchMethodError"
            | "NoSuchFieldError"
            | "AbstractMethodError"
            | "NoClassDefFoundError"
            | "ClassNotFoundException" => break,
            _ => continue,
        };
        if out.iter().any(|finding| finding.id == id) {
            continue;
        }
        let mut evidence = vec![clip(&frame.line(), 480)];
        let mut details = Vec::new();
        if id == FindingId::BytecodeVerification {
            evidence.extend(scan.evidence(&["is not assignable", "bad type on operand stack"], 3));
            let mut lines = scan.raw.lines().map(str::trim);
            if lines.any(|line| line == "Location:")
                && let Some(location) = lines.find(|line| !line.is_empty())
            {
                details.push(format!("bytecode_location={}", clip(location, 360)));
            }
            if scan.raw.contains("org/lwjgl/PointerBuffer")
                && scan.raw.contains("org/lwjgl/system/CustomBuffer")
                && scan.raw.contains("is not assignable")
            {
                details.push("hint=lwjgl_type_conflict".to_string());
            }
        }
        out.push(Finding {
            id,
            severity: Severity::Error,
            confidence: Confidence::High,
            evidence,
            mods: Vec::new(),
            details,
        });
    }
    if frames(report)
        .next_back()
        .is_some_and(|frame| frame.class.ends_with("ExceptionInInitializerError"))
        && !out
            .iter()
            .any(|finding| finding.id == FindingId::ClassInitialization)
    {
        out.push(Finding {
            id: FindingId::ClassInitialization,
            severity: Severity::Error,
            confidence: Confidence::High,
            evidence: scan.evidence(&["exceptionininitializererror"], 3),
            mods: Vec::new(),
            details: Vec::new(),
        });
    }
}

/// 只对已核对的混淆版本提供客户端类映射，其他版本仍按原始类名判断。
fn is_client_class(report: &CrashReport, message: &str) -> bool {
    let class = message.replace('/', ".");
    class.starts_with("net.minecraft.client.")
        || (report.environment.minecraft_version.as_deref() == Some("1.21.1")
            && class == "net.minecraft.class_638")
}

/// 给主结论补充阶段与最内层栈帧，并为明确的运行场景添加排查线索。
pub(super) fn enrich(report: &CrashReport, findings: &mut [Finding]) {
    let Some(primary) = findings.first_mut() else {
        return;
    };
    if let Some(stage) = &report.description {
        push_detail(&mut primary.details, format!("stage={}", clip(stage, 160)));
    }
    let root_stack = report
        .detail
        .rsplit_once("Caused by:")
        .map(|(_, root)| root)
        .unwrap_or(&report.detail);
    let stack: Vec<&str> = root_stack
        .lines()
        .filter(|line| line.starts_with("at "))
        .collect();
    let relevant = stack
        .iter()
        .find(|line| {
            let line = line.split('/').next_back().unwrap_or(line);
            !["java.", "jdk.", "sun."]
                .iter()
                .any(|prefix| line.trim_start_matches("at ").starts_with(prefix))
        })
        .copied()
        .or_else(|| stack.first().copied());
    if let Some(location) = relevant {
        push_detail(
            &mut primary.details,
            format!("location={}", clip(location, 300)),
        );
    }
    for line in stack.iter().take(3) {
        push_detail(&mut primary.evidence, clip(line, 300));
    }
    let related = stack
        .iter()
        .copied()
        .chain(report.detail.lines())
        .find(|line| {
            let Some(call) = line.strip_prefix("at ") else {
                return false;
            };
            let call = call.split('(').next().unwrap_or(call).to_lowercase();
            report.environment.mods.iter().any(|entry| {
                if super::is_loader_mod(&entry.id) {
                    return false;
                }
                let id = entry.id.to_lowercase();
                call.split(['.', '/', '$'])
                    .any(|part| part.split('@').next() == Some(id.as_str()))
            })
        });
    if let Some(related) = related.filter(|line| Some(*line) != relevant) {
        push_detail(
            &mut primary.details,
            format!("related_location={}", clip(related, 300)),
        );
        push_detail(&mut primary.evidence, clip(related, 300));
    }
    match primary.id {
        FindingId::StackOverflow => {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for line in &stack {
                *counts.entry(line).or_default() += 1;
            }
            let mut repeated: Vec<_> = counts
                .into_iter()
                .filter(|(_, count)| *count >= 3)
                .collect();
            repeated.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            for (line, count) in repeated.into_iter().take(3) {
                push_detail(
                    &mut primary.details,
                    format!("repeat_frame={} × {count}", clip(line, 280)),
                );
            }
        }
        FindingId::WatchdogTimeout => {
            let top = stack
                .iter()
                .take(40)
                .copied()
                .collect::<Vec<_>>()
                .join("\n");
            if top.contains("org.apache.logging.log4j.core.appender.OutputStreamManager")
                || top.contains("java.io.FileOutputStream.writeBytes")
            {
                push_detail(&mut primary.details, "hint=watchdog_log_output".to_string());
            }
            if top.contains("net.minecraft.server.command.SpreadPlayersCommand")
                || (report.environment.minecraft_version.as_deref() == Some("1.21.1")
                    && top.contains("net.minecraft.class_3131."))
                || (report.environment.minecraft_version.as_deref() == Some("1.21.1")
                    && top.contains("net.minecraft.class_3131$"))
            {
                push_detail(
                    &mut primary.details,
                    "hint=watchdog_spreadplayers".to_string(),
                );
            }
        }
        FindingId::DimensionMissing => {
            if let Some(value) = frames(report)
                .find_map(|frame| frame.message.split_once("dimension ").map(|(_, rest)| rest))
            {
                let id: String = value
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '-')
                    .collect();
                if !id.is_empty() {
                    push_detail(&mut primary.details, format!("dimension={id}"));
                }
            }
        }
        _ => {}
    }
}
