<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { current, format, pickedBase, baseEntries, pickedMods, mods, pickedPacks, packs, configs, folder, busy, ready, pickedCount, generate, percent, progress, result, error } = ctx;
</script>

<template>
<section class="si-section">
  <div class="si-summary">
    <div class="si-line">
      {{ t("serverInstance.summaryInstance") }}{{ t("common.detailSep") }}
      <code>{{ current?.name }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.summaryFormat") }}{{ t("common.detailSep") }}
      <code>{{ format === "mml" ? "MML" : "HMCL" }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.summaryBase") }}{{ t("common.detailSep") }}
      <code>{{ pickedBase.size }} / {{ baseEntries.length }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.summaryMods") }}{{ t("common.detailSep") }}
      <code>{{ pickedMods.size }} / {{ mods.length }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.summaryPacks") }}{{ t("common.detailSep") }}
      <code>{{ pickedPacks.size }} / {{ packs.length }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.summaryConfigs") }}{{ t("common.detailSep") }}
      <code>{{ configs.length }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.folder") }}{{ t("common.detailSep") }}
      <code>{{ folder || "—" }}</code>
    </div>
  </div>

  <div class="si-actions">
    <BaseButton variant="primary" size="lg" :disabled="busy || !ready || !pickedCount" @click="generate">
      {{ busy ? t("serverInstance.running") : t("serverInstance.generate") }}
    </BaseButton>
  </div>

  <!-- 生成中：进度条 + 当前文件 -->
  <template v-if="busy">
    <div class="progress-track">
      <div class="progress-fill" :class="{ 'is-roll': percent < 0 }"
        :style="percent < 0 ? undefined : { width: percent + '%' }" />
    </div>
    <div class="run-info">
      <span v-if="progress && progress.total > 0">
        {{ progress.now }} / {{ progress.total }}
      </span>
      <span class="cur">{{ progress?.text }}</span>
    </div>
  </template>

  <!-- 结果 -->
  <template v-else-if="result">
    <div class="si-result">
      <div class="si-line">
        {{ t("serverInstance.manifest") }}{{ t("common.detailSep") }}<code>{{ result.manifest }}</code>
      </div>
      <div v-if="result.files.length" class="si-line">
        {{ t("serverInstance.files") }}{{ t("common.detailSep") }}
        <code v-for="item in result.files" :key="item">{{ item }}</code>
      </div>
      <div class="si-line">{{ t("serverInstance.count", { n: result.count }) }}</div>
    </div>
  </template>

  <p v-if="error" class="si-error">{{ error }}</p>
</section>
</template>
