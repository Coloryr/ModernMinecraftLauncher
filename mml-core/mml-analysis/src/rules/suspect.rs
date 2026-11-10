//! 可疑模组汇总
//!
//! 按证据里出现的模组 id 排序，给出最可能的肇事者。

use std::collections::HashMap;

use crate::model::{CrashReport, Finding};

/// 汇总可疑模组
///
/// 权重从高到低：异常消息里直接出现的模组 id、结论点名的模组（按顺序递减）、
/// 栈帧包名与正文里出现的模组 id。栈帧命中每个模组最多算三次，
/// 免得一个到处出现的库把真正被点名的模组挤下去。
pub(super) fn collect_suspected(
    report: &CrashReport,
    findings: &[Finding],
    detail_lower: &str,
) -> Vec<String> {
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

    // 报告标成 Errored 的模组在加载阶段就抛过异常，比栈帧猜出来的包名可靠
    for entry in &report.environment.mods {
        if entry.errored() {
            *score.entry(entry.id.clone()).or_default() += 5;
        }
    }

    // 包名与模组 id 用下划线 / 连字符分隔时（world_preview 对 world.preview），
    // 精确分段对不上，归一化后再比一次
    let dotted: Vec<(&str, String)> = report
        .environment
        .mods
        .iter()
        .filter(|entry| entry.id.len() >= 4 && (entry.id.contains('_') || entry.id.contains('-')))
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
pub(super) fn is_loader_mod(id: &str) -> bool {
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
