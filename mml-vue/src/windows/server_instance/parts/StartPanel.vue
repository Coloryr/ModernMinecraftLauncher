<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const {
  current,
  pickedBase,
  pickedMods,
  mods,
  pickedPacks,
  packs,
  configCount,
  folder,
  busy,
  ready,
  pickedCount,
  generate,
  exportPack,
  packing,
  percent,
  progress,
  result,
  error,
  modChanges,
  packChanges,
} = ctx;
</script>

<template>
<section class="si-section">
  <div class="si-summary">
    <div class="si-line">
      {{ t("serverInstance.summaryInstance") }}{{ t("common.detailSep") }}
      <code>{{ current?.name }}</code>
    </div>
    <div class="si-line">
      {{ t("serverInstance.summaryBase") }}{{ t("common.detailSep") }}
      <code>{{ pickedBase.size }}</code>
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
      <code>{{ configCount }}</code>
    </div>
    <div v-if="modChanges" class="si-line">
      {{ t("serverInstance.modChanges") }}{{ t("common.detailSep") }}
      <span class="si-change add">+{{ modChanges.added.length }}</span>
      <span class="si-change del">-{{ modChanges.removed.length }}</span>
      <span class="si-change mod">·{{ modChanges.modified.length }}</span>
    </div>
    <div v-if="packChanges" class="si-line">
      {{ t("serverInstance.packChanges") }}{{ t("common.detailSep") }}
      <span class="si-change add">+{{ packChanges.added.length }}</span>
      <span class="si-change del">-{{ packChanges.removed.length }}</span>
      <span class="si-change mod">·{{ packChanges.modified.length }}</span>
    </div>
    <div class="si-line">
      {{ t("serverInstance.folder") }}{{ t("common.detailSep") }}
      <code>{{ folder || "—" }}</code>
    </div>
  </div>

  <!-- 三个按钮同一行：左侧导出基础包 / 完整包，右侧生成服务器实例 -->
  <div class="si-actions">
    <BaseButton size="md" :disabled="busy || packing || !ready" @click="exportPack(false)">
      {{ t("serverInstance.packThin") }}
    </BaseButton>
    <BaseButton size="md" :disabled="busy || packing || !ready" @click="exportPack(true)">
      {{ t("serverInstance.packFull") }}
    </BaseButton>
    <BaseButton variant="primary" size="md" class="si-actions-main"
      :disabled="busy || packing || !ready || !pickedCount" @click="generate">
      {{ busy ? t("serverInstance.running") : t("serverInstance.generate") }}
    </BaseButton>
  </div>
  <p class="field-hint">{{ t("serverInstance.packHint") }}</p>

  <!-- 导出服务器包：进度条 + 当前写入的文件 -->
  <template v-if="packing">
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
