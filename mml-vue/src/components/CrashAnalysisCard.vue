<script setup lang="ts">
// 崩溃分析卡片：贴在日志正文上方，给出主因、可疑模组、细节与证据
//
// 结论的文案按后端给的稳定 id 取，规则新增 id 时只补 i18n，不动这里。
import { computed, ref } from "vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import type { CrashAnalysisDto, CrashFindingDto } from "../lib/bindings";
import { t } from "../lib/i18n";

const props = defineProps<{
  /** 后端 log_analyze 的结果 */
  analysis: CrashAnalysisDto;
}>();

defineEmits<{ (e: "close"): void }>();

/** 主因，后端已按相关性排好序 */
const primary = computed(() => props.analysis.findings[0] ?? null);
/** 其余结论 */
const others = computed(() => props.analysis.findings.slice(1));
/** 可疑模组，最多列六个 */
const suspects = computed(() => props.analysis.suspectedMods.slice(0, 6));
/** 证据默认收起：日志行很长，展开会把正文挤下去 */
const showEvidence = ref(false);

/** 卡片配色跟着主因的严重程度走 */
const tone = computed(() => {
  const severity = primary.value?.severity ?? "error";
  if (severity === "fatal" || severity === "error") return "error";
  if (severity === "warning") return "warn";
  return "info";
});

/**
 * 结论 id 转文案
 *
 * - finding: 一条结论
 *
 * # 返回值
 *
 * 译文；不认识的 id 原样显示
 */
function findingTitle(finding: CrashFindingDto): string {
  const key = "analysis.finding." + finding.id;
  const text = t(key);
  return text === key ? finding.id : text;
}

/** 把握程度转文案 */
function confidenceLabel(finding: CrashFindingDto): string {
  return t("analysis.confidence." + finding.confidence);
}

/** 环境摘要，如 1.20.1 · forge 47.2.0 · Java 17 */
const envLine = computed(() => {
  const parts: string[] = [];
  const analysis = props.analysis;
  if (analysis.minecraftVersion) parts.push(analysis.minecraftVersion);
  if (analysis.loader) {
    parts.push(
      analysis.loaderVersion
        ? analysis.loader + " " + analysis.loaderVersion
        : analysis.loader,
    );
  }
  if (analysis.javaVersion) parts.push("Java " + analysis.javaVersion);
  if (analysis.memoryMaxMb) parts.push(analysis.memoryMaxMb + " MiB");
  return parts.join(" · ");
});

/**
 * 把后端的机器可读细节翻成人话
 *
 * - finding: 一条结论
 *
 * # 返回值
 *
 * 按原顺序排好的细节行；不认识的键原样保留
 */
function detailLines(finding: CrashFindingDto): string[] {
  return finding.details.map((line) => {
    const at = line.indexOf("=");
    if (at < 0) return line;
    const key = line.slice(0, at);
    const value = line.slice(at + 1);
    switch (key) {
      case "mixin_config":
        return t("analysis.detail.mixinConfig", { value });
      case "mixin_class":
        return t("analysis.detail.mixinClass", { value });
      case "requires":
        return t("analysis.detail.requires", { value });
      case "required_version":
        return t("analysis.detail.requiredVersion", { value });
      case "actual_version":
        return t("analysis.detail.actualVersion", { value });
      case "memory_max_mb":
        return t("analysis.detail.memory", { value });
      case "class_file_version":
        return t("analysis.detail.classFile", { value });
      case "required_java":
        return t("analysis.detail.requiredJava", { value });
      case "loader":
        return t("analysis.detail.loader", { value });
      default:
        return line;
    }
  });
}

/** 主因的细节行 */
const primaryDetails = computed(() => (primary.value ? detailLines(primary.value) : []));
</script>

<template>
  <section class="analysis-card" :class="'is-' + tone">
    <GlyphIcon class="analysis-icon" name="alert" :size="15" :weight="2" />
    <div class="analysis-body">
      <div class="analysis-head">
        <span class="analysis-title">
          {{ primary ? findingTitle(primary) : t("analysis.noFinding") }}
        </span>
        <span v-if="primary" class="analysis-badge">{{ confidenceLabel(primary) }}</span>
        <span v-if="envLine" class="analysis-env">{{ envLine }}</span>
      </div>

      <div v-if="suspects.length" class="analysis-suspects">
        <span class="analysis-label">{{ t("analysis.suspects") }}</span>
        <span class="analysis-suspect">{{ suspects.join(", ") }}</span>
      </div>

      <ul v-if="primaryDetails.length" class="analysis-details">
        <li v-for="line in primaryDetails" :key="line">{{ line }}</li>
      </ul>

      <div v-if="primary && primary.evidence.length" class="analysis-evidence">
        <button class="analysis-toggle" @click="showEvidence = !showEvidence">
          {{
            showEvidence
              ? t("analysis.hideEvidence")
              : t("analysis.showEvidence", { count: primary.evidence.length })
          }}
        </button>
        <pre v-if="showEvidence" class="analysis-pre">{{ primary.evidence.join("\n") }}</pre>
      </div>

      <div v-if="others.length" class="analysis-others">
        <div class="analysis-label">{{ t("analysis.others", { count: others.length }) }}</div>
        <div v-for="(finding, index) in others" :key="index" class="analysis-other">
          <span class="analysis-other-title">{{ findingTitle(finding) }}</span>
          <span class="analysis-other-meta">{{ confidenceLabel(finding) }}</span>
          <span v-if="finding.mods.length" class="analysis-other-mods">
            {{ finding.mods.slice(0, 8).join(", ") }}{{ finding.mods.length > 8 ? " …" : "" }}
          </span>
        </div>
      </div>
    </div>
    <button
      class="analysis-close"
      :aria-label="t('analysis.close')"
      v-tip="t('analysis.close')"
      @click="$emit('close')"
    >
      ×
    </button>
  </section>
</template>

<style scoped>
/* 崩溃分析横幅：贴在日志正文上方。内容可能很长，限高后自己滚 */
.analysis-card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  flex: 0 1 auto;
  max-height: 40%;
  overflow-y: auto;
  padding: 8px 10px;
  border: 1px solid var(--red);
  border-radius: 10px;
  background: color-mix(in srgb, var(--red) 8%, var(--bg));
}

.analysis-card.is-warn {
  border-color: var(--yellow);
  background: color-mix(in srgb, var(--yellow) 8%, var(--bg));
}

.analysis-card.is-info {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.analysis-icon {
  flex: none;
  margin-top: 2px;
  color: var(--red);
}

.is-warn .analysis-icon {
  color: var(--yellow);
}

.is-info .analysis-icon {
  color: var(--accent);
}

.analysis-body {
  flex: 1;
  min-width: 0;
}

.analysis-head {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 6px;
}

.analysis-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
}

.analysis-badge {
  padding: 0 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--red) 18%, transparent);
  color: var(--red);
  font-size: 10.5px;
  line-height: 16px;
}

.is-warn .analysis-badge {
  background: color-mix(in srgb, var(--yellow) 18%, transparent);
  color: var(--yellow);
}

.is-info .analysis-badge {
  background: var(--accent-soft);
  color: var(--accent);
}

.analysis-env {
  color: var(--text-dim);
  font-size: 11.5px;
}

.analysis-label {
  color: var(--text-dim);
  font-size: 11.5px;
}

.analysis-suspects {
  margin-top: 4px;
  font-size: 12.5px;
}

.analysis-suspect {
  margin-left: 6px;
  color: var(--red);
  font-weight: 600;
  word-break: break-all;
}

.is-warn .analysis-suspect {
  color: var(--yellow);
}

.is-info .analysis-suspect {
  color: var(--accent);
}

.analysis-details {
  margin: 4px 0 0;
  padding-left: 16px;
  color: var(--text-dim);
  font-size: 11.5px;
  word-break: break-all;
}

.analysis-evidence {
  margin-top: 4px;
}

.analysis-toggle {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-size: 11.5px;
  cursor: pointer;
}

.analysis-toggle:hover {
  text-decoration: underline;
}

.analysis-pre {
  margin: 4px 0 0;
  padding: 6px 8px;
  max-height: 160px;
  overflow: auto;
  border-radius: 8px;
  background: var(--bg-modal);
  color: var(--text-dim);
  font-size: 11.5px;
  white-space: pre-wrap;
  word-break: break-all;
}

.analysis-others {
  margin-top: 6px;
}

.analysis-other {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 2px;
  font-size: 11.5px;
}

.analysis-other-title {
  color: var(--text);
}

.analysis-other-meta {
  color: var(--text-dim);
}

.analysis-other-mods {
  color: var(--text-dim);
  word-break: break-all;
}

.analysis-close {
  flex: none;
  padding: 0 2px;
  border: 0;
  background: transparent;
  color: var(--text-dim);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
}

.analysis-close:hover {
  color: var(--text);
}
</style>