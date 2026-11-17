//! 可疑模组汇总
//!
//! 按证据里出现的模组 id 排序，给出最可能的肇事者。

use std::collections::HashMap;

use crate::model::{CrashReport, Finding};

/// 汇总可疑模组
///
/// 权重从高到低：异常消息里直接出现的模组 id、报告标成 Errored 的模组、
/// 结论点名的模组（按顺序递减）、栈帧归属的模组。栈帧命中每个模组最多算三次，
/// 免得一个到处出现的库把真正被点名的模组挤下去。
///
/// 栈帧只按 jar 文件名与包段归属，不在整段栈文本里搜模组 id：调用方的栈帧会把
/// 所有作用于该类的 mixin 配置名列出来，按子串算会把它们全变成嫌疑人。
pub(super) fn collect_suspected(report: &CrashReport, findings: &[Finding]) -> Vec<String> {
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

    let mut frame_score: HashMap<String, u32> = HashMap::new();
    for line in report.detail.lines() {
        let Some(id) = mod_of_frame(report, line) else {
            continue;
        };
        let entry = frame_score.entry(id.to_string()).or_default();
        if *entry < 3 {
            *entry += 1;
        }
    }
    for (id, hits) in frame_score {
        *score.entry(id).or_default() += hits;
    }

    let mut list: Vec<(String, u32)> = score
        .into_iter()
        .filter(|(id, _)| !is_loader_mod(id))
        .collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list.into_iter().take(8).map(|(id, _)| id).collect()
}

/// 栈帧属于哪个模组
///
/// 依次按栈帧末尾的 jar 文件名、类名的包段与 NeoForge 的 `modid@版本` 段匹配。
/// 模组 id 与包名经常对不上（`ae2` 的包是 `appeng`），jar 文件名才是能直接对上的那个。
///
/// - `report`: 解析结果，模组表在里面
/// - `line`: 栈帧原文，以 `at ` 开头
///
/// # 返回值
///
/// 模组 id；对不上或属于加载器时返回 `None`
pub(super) fn mod_of_frame<'a>(report: &'a CrashReport, line: &str) -> Option<&'a str> {
    let call = line.strip_prefix("at ")?;

    if let Some(jar) = jar_of_frame(line)
        && let Some(entry) = report
            .environment
            .mods
            .iter()
            .find(|entry| !entry.file.is_empty() && entry.file.eq_ignore_ascii_case(jar))
    {
        return (!is_loader_mod(&entry.id)).then_some(entry.id.as_str());
    }

    let class = call.split('(').next().unwrap_or(call).to_lowercase();
    for segment in class.split(['.', '/', '$']) {
        // NeoForge 的栈帧写成 TRANSFORMER/modid@版本/包名，版本要剥掉
        let segment = segment.split('@').next().unwrap_or(segment);
        if let Some(entry) = report.environment.mod_of(segment)
            && !is_loader_mod(&entry.id)
        {
            return Some(entry.id.as_str());
        }
    }

    // 包名与模组 id 用下划线 / 连字符分隔时（world_preview 对 world.preview），
    // 精确分段对不上，归一化后再比一次
    report
        .environment
        .mods
        .iter()
        .filter(|entry| entry.id.len() >= 4 && (entry.id.contains('_') || entry.id.contains('-')))
        .find(|entry| {
            !is_loader_mod(&entry.id)
                && class.contains(&entry.id.to_lowercase().replace(['_', '-'], "."))
        })
        .map(|entry| entry.id.as_str())
}

/// 栈帧末尾 `~[xxx.jar%23123!/:?]` 里的 jar 文件名
fn jar_of_frame(line: &str) -> Option<&str> {
    let start = line.find('[')?;
    let end = line[start..].find(']')? + start;
    let inner = &line[start + 1..end];
    // `%23` 是 `#` 的转义，后面跟着文件 id，不属于文件名
    let jar = inner.split('%').next()?;
    jar.ends_with(".jar").then_some(jar)
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
