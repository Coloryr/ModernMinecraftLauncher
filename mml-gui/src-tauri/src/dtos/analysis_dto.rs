//! 崩溃分析 DTO —— `log_analyze` 的返回结构
//!
//! 字段直接对应 `mml-analysis` 的结论；枚举一律给稳定 id，前端按 id 取文案。

use mml_analysis::{CrashAnalysis, Finding};
use serde::Serialize;

/// 一份日志的分析结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashAnalysisDto {
    /// 日志类型：crash_report / game_log / jvm_fatal / unknown
    pub kind: String,
    /// 崩溃报告里的 Description
    pub description: Option<String>,
    /// 最外层异常的一行文本
    pub exception: Option<String>,
    /// 从外到内的 Caused by 异常链，不含其他线程或清理阶段的异常
    pub causes: Vec<String>,
    /// 命中的结论，第一条是主因
    pub findings: Vec<CrashFindingDto>,
    /// 在故障链中出现的模组，仅作为排查线索
    pub suspected_mods: Vec<String>,
    /// Minecraft 版本
    pub minecraft_version: Option<String>,
    /// 加载器 id：forge / neoforge / fabric / quilt
    pub loader: Option<String>,
    /// 加载器版本
    pub loader_version: Option<String>,
    /// Java 版本
    pub java_version: Option<String>,
    /// 最大堆内存，单位 MiB
    pub memory_max_mb: Option<u64>,
}

/// 一条分析结论
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashFindingDto {
    /// 问题类型，如 mixin_failure / missing_dependency
    pub id: String,
    /// 严重程度：info / warning / error / fatal
    pub severity: String,
    /// 把握：low / medium / high
    pub confidence: String,
    /// 命中的原始行
    pub evidence: Vec<String>,
    /// 涉及的模组 id，出现不代表该模组有缺陷
    pub mods: Vec<String>,
    /// 机器可读细节，如 mixin_config=xxx.mixins.json
    pub details: Vec<String>,
}

impl From<CrashAnalysis> for CrashAnalysisDto {
    fn from(value: CrashAnalysis) -> Self {
        Self {
            kind: value.kind.as_str().to_string(),
            description: value.description,
            exception: value.exception.map(|frame| frame.line()),
            causes: value.causes.into_iter().map(|frame| frame.line()).collect(),
            findings: value.findings.into_iter().map(Into::into).collect(),
            suspected_mods: value.suspected_mods,
            minecraft_version: value.environment.minecraft_version,
            loader: value
                .environment
                .loader
                .map(|loader| loader.as_str().to_string()),
            loader_version: value.environment.loader_version,
            java_version: value.environment.java_version,
            memory_max_mb: value.environment.memory_max_mb,
        }
    }
}

impl From<Finding> for CrashFindingDto {
    fn from(value: Finding) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            severity: value.severity.as_str().to_string(),
            confidence: value.confidence.as_str().to_string(),
            evidence: value.evidence,
            mods: value.mods,
            details: value.details,
        }
    }
}
