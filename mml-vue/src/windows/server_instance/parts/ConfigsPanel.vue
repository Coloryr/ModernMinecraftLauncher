<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import type { ConfigMode } from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const {
  busy,
  changedPaths,
  configDirs,
  configRows,
  configCount,
  configModeOf,
  configAncestorLock,
  configDeleteLocked,
  configFileState,
  setConfigFile,
  toggleConfigFile,
  setAllConfigs,
  setConfigMode,
  toggleConfigExpand,
  isConfigOpen,
  isConfigLoading,
} = ctx;

/** 下拉里的同步方式，顺序与产品口径一致 */
const MODES: ConfigMode[] = ["none", "archiveAll", "archiveSelf", "filesAll", "filesSelf", "delete"];

/** 文件复选框不可用时的说明 */
function fileTitle(path: string): string | undefined {
  const state = configFileState(path);
  if (state === "none") return t("serverInstance.configFileNoOwner");
  if (state === "locked") return t("serverInstance.configFileLocked");
  return undefined;
}

/** 文件或文件夹大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${(size / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
</script>

<template>
<section class="si-section stretch">
  <div class="si-section-head">
    <span class="si-counter">{{ t("serverInstance.configCount", { n: configCount }) }}</span>
    <BaseButton size="sm" :disabled="busy || !configDirs.length" @click="setAllConfigs(true)">
      {{ t("serverInstance.configSyncAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy || !configDirs.length" @click="setAllConfigs(false)">
      {{ t("serverInstance.configSyncNone") }}
    </BaseButton>
  </div>

  <!-- 三列对齐：名字（按层级缩进）/ 大小 / 同步方式 -->
  <div class="si-table-card">
    <div class="si-config-tree">
      <div class="si-config-head">
        <span>{{ t("serverInstance.configName") }}</span>
        <span class="si-config-size">{{ t("serverInstance.configSize") }}</span>
        <span>{{ t("serverInstance.configMode") }}</span>
      </div>
      <p v-if="!configDirs.length" class="empty-tip">{{ t("serverInstance.configEmpty") }}</p>
      <div v-for="row in configRows" :key="row.path" class="si-config-row"
        :class="{ 'is-changed': changedPaths.has(row.path) || (row.depth === 0 && changedPaths.has(row.root)) }">
        <div class="si-config-name" :style="{ paddingLeft: `${row.depth * 16}px` }">
          <span class="si-caret" :class="{ open: isConfigOpen(row.path) }"
            @click="row.dir && !configDeleteLocked(row.path) && toggleConfigExpand(row.path)">
            <GlyphIcon v-if="row.dir && !configDeleteLocked(row.path)" name="chevron-down" :size="12" :weight="2.4" />
          </span>
          <span class="si-check">
            <input v-if="!row.dir" type="checkbox" class="si-box"
              :checked="configFileState(row.path) === 'on'"
              :disabled="busy || configFileState(row.path) === 'none' || configFileState(row.path) === 'locked'"
              :title="fileTitle(row.path)"
              @change="setConfigFile(row.path, ($event.target as HTMLInputElement).checked)" />
          </span>
          <span class="si-name" :class="{ dir: row.dir }" :title="row.path"
            @click="row.dir
              ? (!configDeleteLocked(row.path) && toggleConfigExpand(row.path))
              : toggleConfigFile(row.path)">
            {{ row.name }}{{ row.dir ? "/" : "" }}
          </span>
          <span v-if="isConfigLoading(row.path)" class="si-config-loading">
            {{ t("serverInstance.configLoading") }}
          </span>
        </div>
        <span class="si-config-size">{{ sizeText(row.size) }}</span>
        <div class="si-config-mode-cell">
          <template v-if="row.dir">
            <span v-if="configAncestorLock(row.path) === 'recursive'"
              class="si-config-follow" :title="t('serverInstance.configFollowHint')">
              {{ t("serverInstance.configFollow") }}
            </span>
            <span v-else-if="configAncestorLock(row.path) === 'delete'"
              class="si-config-follow" :title="t('serverInstance.configFollowHint')">
              {{ t("serverInstance.configFollow") }}
            </span>
            <select v-else-if="configAncestorLock(row.path) === 'none'"
              class="field-select si-cell-control si-config-mode" value="none" disabled
              :title="t('serverInstance.configNoneLock')" :aria-label="t('serverInstance.configMode')">
              <option value="none">{{ t("serverInstance.sync.none") }}</option>
            </select>
            <select v-else class="field-select si-cell-control si-config-mode" :value="configModeOf(row.path)"
              :disabled="busy" :aria-label="t('serverInstance.configMode')"
              @change="setConfigMode(row.path, $event)">
              <option v-for="mode in MODES" :key="mode" :value="mode">
                {{ t(`serverInstance.sync.${mode}`) }}
              </option>
            </select>
          </template>
        </div>
      </div>
    </div>
  </div>
  <p class="field-hint">{{ t("serverInstance.configHint") }}</p>
  <p class="field-hint">{{ t("serverInstance.configSelfHint") }}</p>
</section>
</template>
