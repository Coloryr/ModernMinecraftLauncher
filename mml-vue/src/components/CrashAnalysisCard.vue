<script setup lang="ts">
// 崩溃分析卡片：先展示解释和操作建议，再展开原始证据与完整异常链。
import { computed, ref, watch } from "vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import BaseButton from "./ui/BaseButton.vue";
import CollapsePanel from "./ui/CollapsePanel.vue";
import type { CrashAnalysisDto } from "../lib/bindings";
import { presentFinding } from "../lib/crashAnalysisPresentation";
import { t } from "../lib/i18n";

const props = defineProps<{
  /** 后端 log_analyze 的结果 */
  analysis: CrashAnalysisDto;
}>();

defineEmits<{
  /** 用户关闭本次分析卡片。 */
  (e: "close"): void;
}>();

const primary = computed(() => props.analysis.findings[0] ?? null);
const presentation = computed(() => primary.value ? presentFinding(primary.value, t) : null);
const others = computed(() => props.analysis.findings.slice(1).map((finding) => ({
  finding,
  presentation: presentFinding(finding, t),
})));
const suspects = computed(() => props.analysis.suspectedMods.slice(0, 6));
const showEvidence = ref(false);
const showOthers = ref(false);

// 切换日志后收起旧展开状态，避免长异常链直接挤占新日志正文。
watch(() => props.analysis, () => {
  showEvidence.value = false;
  showOthers.value = false;
});

const tone = computed(() => {
  const severity = primary.value?.severity ?? "error";
  if (severity === "fatal" || severity === "error") return "error";
  if (severity === "warning") return "warn";
  return "info";
});

const envLine = computed(() => {
  const parts: string[] = [];
  const analysis = props.analysis;
  if (analysis.minecraftVersion) parts.push(analysis.minecraftVersion);
  if (analysis.loader) {
    parts.push(analysis.loaderVersion ? analysis.loader + " " + analysis.loaderVersion : analysis.loader);
  }
  if (analysis.javaVersion) parts.push("Java " + analysis.javaVersion);
  if (analysis.memoryMaxMb) parts.push(analysis.memoryMaxMb + " MiB");
  return parts.join(" · ");
});

const exceptionChain = computed(() => {
  const analysis = props.analysis;
  const lines = analysis.exception ? [t("analysis.outerException", { value: analysis.exception })] : [];
  for (const value of analysis.causes) lines.push(t("analysis.causedBy", { value }));
  return lines.join("\n");
});
</script>

<template>
  <section class="analysis-card" :class="'is-' + tone">
    <GlyphIcon class="analysis-icon" name="alert" :size="15" :weight="2" />
    <div class="analysis-body">
      <div class="analysis-head">
        <span class="analysis-title">{{ presentation?.title ?? t("analysis.noFinding") }}</span>
        <span v-if="primary" class="analysis-badge" :title="t('analysis.confidenceHelp')">
          {{ t("analysis.confidence." + primary.confidence) }}
        </span>
        <span v-if="envLine" class="analysis-env">{{ envLine }}</span>
      </div>

      <div v-if="presentation" class="analysis-explanation">
        <p><strong>{{ t("analysis.reason") }}：</strong>{{ presentation.reason }}</p>
        <p><strong>{{ t("analysis.advice") }}：</strong>{{ presentation.advice }}</p>
        <p v-for="hint in presentation.hints" :key="hint">{{ hint }}</p>
        <ul v-if="presentation.details.length" class="analysis-details">
          <li v-for="(line, index) in presentation.details" :key="index">{{ line }}</li>
        </ul>
      </div>

      <div v-if="suspects.length" class="analysis-suspects">
        <span class="analysis-label">{{ t("analysis.suspects") }}</span>
        <span class="analysis-suspect">{{ suspects.join(", ") }}</span>
        <p class="analysis-help">{{ t("analysis.suspectsHelp") }}</p>
      </div>

      <div class="analysis-actions">
        <BaseButton
          v-if="primary?.evidence.length || exceptionChain"
          variant="ghost" size="sm" :aria-expanded="showEvidence"
          @click="showEvidence = !showEvidence"
        >
          {{ showEvidence ? t("analysis.hideEvidence") : t("analysis.showEvidence", { count: primary?.evidence.length ?? 0 }) }}
        </BaseButton>
        <BaseButton v-if="others.length" variant="ghost" size="sm" :aria-expanded="showOthers" @click="showOthers = !showOthers">
          {{ showOthers ? t("analysis.hideOthers") : t("analysis.showOthers") }}
        </BaseButton>
      </div>
      <CollapsePanel :open="showEvidence">
        <div v-if="showEvidence" class="analysis-evidence">
          <pre v-if="primary?.evidence.length" class="analysis-pre">{{ primary.evidence.join("\n") }}</pre>
          <template v-if="exceptionChain">
            <p class="analysis-label">{{ t("analysis.exceptionChain") }}</p>
            <pre class="analysis-pre">{{ exceptionChain }}</pre>
          </template>
        </div>
      </CollapsePanel>
      <CollapsePanel :open="showOthers">
        <div v-if="showOthers" class="analysis-others">
          <div class="analysis-label">{{ t("analysis.others", { count: others.length }) }}</div>
          <div v-for="item in others" :key="item.finding.id" class="analysis-other">
            <span class="analysis-other-title">{{ item.presentation.title }}</span>
            <span class="analysis-other-meta">{{ t("analysis.confidence." + item.finding.confidence) }}</span>
            <p>{{ item.presentation.reason }}</p>
            <p><strong>{{ t("analysis.advice") }}：</strong>{{ item.presentation.advice }}</p>
            <p v-for="hint in item.presentation.hints" :key="hint">{{ hint }}</p>
            <p v-if="item.finding.mods.length">{{ t("analysis.suspects") }}：{{ item.finding.mods.join(", ") }}</p>
            <ul v-if="item.presentation.details.length" class="analysis-details">
              <li v-for="(line, index) in item.presentation.details" :key="index">{{ line }}</li>
            </ul>
            <pre v-if="item.finding.evidence.length" class="analysis-pre">{{ item.finding.evidence.join("\n") }}</pre>
          </div>
        </div>
      </CollapsePanel>
    </div>
    <BaseButton class="analysis-close" variant="ghost" size="sm" :aria-label="t('analysis.close')" v-tip="t('analysis.close')" @click="$emit('close')">
      <GlyphIcon name="close" :size="15" />
    </BaseButton>
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

.analysis-evidence,
.analysis-explanation {
  margin-top: 4px;
}

.analysis-explanation {
  font-size: 12.5px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.analysis-explanation p {
  margin: 4px 0;
}

.analysis-help {
  margin: 2px 0;
  color: var(--text-dim);
  font-size: 11.5px;
}

.analysis-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 4px;
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
  margin-top: 8px;
  font-size: 11.5px;
  overflow-wrap: anywhere;
}

.analysis-other p {
  margin: 4px 0;
}

.analysis-other-title {
  color: var(--text);
  font-weight: 600;
}

.analysis-other-meta {
  margin-left: 6px;
  color: var(--text-dim);
}

.analysis-close {
  flex: none;
}
</style>
