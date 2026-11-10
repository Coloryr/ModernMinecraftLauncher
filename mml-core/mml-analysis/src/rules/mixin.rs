//! Mixin 规则
//!
//! 从 Mixin 报错里认出配置、类与被注入的模组。

use std::collections::HashMap;

use crate::model::{Confidence, CrashReport, Finding, FindingId, Severity};

use super::*;

/// Mixin 注入失败
pub(super) fn check_mixin(report: &CrashReport, scan: &Scan, out: &mut Vec<Finding>) {
    let needles = [
        "mixin apply failed",
        "mixin apply for mod",
        "mixintransformererror",
        "invalidmixinexception",
        "mixin transformation of",
        "mixin.injection.throwables",
        "critical injection failure",
    ];
    // "mixin config" 本身在正常启动日志里也可能出现，必须配上错误措辞才算
    let config_error = scan.detail_lower.contains("mixin config")
        && (scan.detail_lower.contains("does not specify")
            || scan.detail_lower.contains("could not be")
            || scan.detail_lower.contains("failed to")
            || scan.detail_lower.contains("is missing")
            || scan.detail_lower.contains("invalid"));
    if !needles
        .iter()
        .any(|needle| scan.detail_lower.contains(needle))
        && !config_error
    {
        return;
    }

    let suspects = mixin_suspects(report, scan.raw);
    let strong = RE_FROM_MOD.is_match(scan.raw)
        || RE_MIXIN_APPLY_MOD.is_match(scan.raw)
        || RE_MIXIN_CONFIG_FROM.is_match(scan.raw);

    // 配置名只在异常与栈帧里找：整份日志里每个模组的 mixin 配置都会出现，
    // 全收进来会把 details 撑到几百条
    let mut details: Vec<String> = Vec::new();
    for name in mixin_configs(scan, 8) {
        push_detail(&mut details, format!("mixin_config={name}"));
    }
    if let Some(class) = mixin_class(&report.detail) {
        push_detail(&mut details, format!("mixin_class={class}"));
    }

    out.push(Finding {
        id: FindingId::MixinFailure,
        severity: Severity::Error,
        confidence: if strong || !suspects.is_empty() {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence: scan.evidence(&needles, 8),
        mods: suspects,
        details,
    });
}

/// 从 Mixin 报错里挑出最可能出错的模组
///
/// 依据依次是显式的 from mod、Mixin 配置名、栈帧包名与报错里出现的模组 id。
///
/// - report: 解析结果，模组列表用来把配置名 / 包名对回模组 id
/// - raw: 原文
///
/// # 返回值
///
/// 按可疑程度排序的模组 id，第一个最可能是元凶
pub(super) fn mixin_suspects(report: &CrashReport, raw: &str) -> Vec<String> {
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
        } else if !candidate.contains('.') {
            // 对不上模组列表时只认像 id 的名字：mixins.angelica.early.j21.json
            // 推出来的是包名，当模组报出去只会误导
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
            // NeoForge 的栈帧写成 TRANSFORMER/modid@版本/包名，版本要剥掉
            let segment = segment.split('@').next().unwrap_or(segment);
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

/// 从 Mixin 报错里取配置名，最多 limit 个
///
/// 只认报错上下文里的配置名：正常启动日志里每个模组都会列一遍自己的 Mixin 配置，
/// 全收进来会把 details 撑到几百条。
pub(super) fn mixin_configs(scan: &Scan, limit: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for (line, low) in scan.raw.lines().zip(scan.lower.lines()) {
        if !low.contains("mixin") {
            continue;
        }
        let context = [
            "apply",
            "failed",
            "failure",
            "invalid",
            "does not specify",
            "critical",
            "transformer",
            "injection",
        ]
        .iter()
        .any(|needle| low.contains(needle));
        if !context {
            continue;
        }

        for captures in RE_JSON_NAME.captures_iter(line) {
            let name = captures[1].to_string();
            if !name.to_lowercase().contains("mixin") || out.contains(&name) {
                continue;
            }
            out.push(name);
            if out.len() >= limit {
                return out;
            }
        }
    }

    // 兜底：异常与栈帧里的 config.json:Class
    for captures in RE_JSON_NAME.captures_iter(&scan.detail_lower) {
        let name = captures[1].to_string();
        if name.to_lowercase().contains("mixin") && !out.contains(&name) {
            out.push(name);
            if out.len() >= limit {
                break;
            }
        }
    }

    out
}

/// 从 Mixin 配置名推出模组 id
///
/// 支持 modid.mixins.json、modid.client.mixins.json、mixins.modid.json
/// 与 modid.mixin.json 这几种命名。
///
/// - name: 配置文件名
///
/// # 返回值
///
/// 推不出时返回 None
pub(super) fn mod_from_mixin_config(name: &str) -> Option<String> {
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
pub(super) fn mixin_class(text: &str) -> Option<String> {
    RE_MIXIN_CLASS
        .captures_iter(text)
        .find(|captures| captures[1].to_lowercase().contains("mixin"))
        .map(|captures| captures[2].to_string())
}
