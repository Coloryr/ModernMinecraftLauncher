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
  configs,
  newDir,
  newMode,
  dirChoices,
  newChildDirs,
  childAddParent,
  busy,
  changedPaths,
  addConfig,
  addChildConfigs,
  setConfigMode,
  setConfigDeleteOld,
  removeConfig,
} = ctx;
</script>

<template>
<section class="si-section stretch">
  <!-- 与模组页同一个表头行：左边计数，右边控件；下面的表也是同一副外观 -->
  <div class="si-section-head">
    <span class="si-counter">{{ t("serverInstance.configCount", { n: configs.length }) }}</span>
    <select v-model="newDir" class="field-select si-add-dir" :title="t('serverInstance.configSelect')"
      :disabled="busy || !dirChoices.length">
      <option value="" disabled>
        {{ dirChoices.length ? t("serverInstance.configPick") : t("serverInstance.configNoMore") }}
      </option>
      <option v-for="dir in dirChoices" :key="dir.path" :value="dir.path">
        {{ dir.path }}/
      </option>
    </select>
    <select v-model="newMode" class="field-select si-add-mode" :disabled="busy">
      <option value="archive">{{ t("serverInstance.mode.archive") }}</option>
      <option value="files">{{ t("serverInstance.mode.files") }}</option>
    </select>
    <BaseButton size="sm" :disabled="busy || !newDir" @click="addConfig">
      {{ t("serverInstance.configAdd") }}
    </BaseButton>
    <!-- 一级子目录几十上百个时逐个加太慢：按当前选中目录的父目录一次全加进来 -->
    <BaseButton size="sm" :disabled="busy || !newChildDirs.length"
      :title="newChildDirs.length ? t('serverInstance.configAddChildrenHint', { dir: childAddParent + '/', n: newChildDirs.length }) : ''"
      @click="addChildConfigs">
      {{ t("serverInstance.configAddChildren") }}
    </BaseButton>
  </div>

  <!-- 与模组页同一套表格外观：表头吸顶、行内直接改同步方式 -->
  <div class="si-table-card">
    <div class="si-table">
      <div class="si-table-head">
        <span>{{ t("serverInstance.configName") }}</span>
        <span>{{ t("serverInstance.configMode") }}</span>
        <span>{{ t("serverInstance.configDeleteOld") }}</span>
        <span></span>
      </div>
      <p v-if="!configs.length" class="empty-tip">{{ t("serverInstance.configEmpty") }}</p>
      <div v-for="item in configs" :key="item.dir" class="si-table-row"
        :class="{ 'is-changed': changedPaths.has(item.dir) }">
        <span class="si-name" :title="item.dir">{{ item.dir }}/</span>
        <select class="field-select si-cell-control" :value="item.mode" :disabled="busy"
          @change="setConfigMode(item.dir, ($event.target as HTMLSelectElement).value as ConfigMode)">
          <option value="archive">{{ t("serverInstance.mode.archive") }}</option>
          <option value="files">{{ t("serverInstance.mode.files") }}</option>
        </select>
        <label class="si-cell-check">
          <input type="checkbox" class="si-box" :checked="item.deleteOld"
            :disabled="busy || item.mode !== 'archive'"
            @change="setConfigDeleteOld(item.dir, $event)" />
        </label>
        <button class="si-remove" :disabled="busy" :title="t('serverInstance.configRemove')"
          @click="removeConfig(item.dir)">
          <GlyphIcon name="trash" :size="13" />
        </button>
      </div>
    </div>
  </div>
</section>
</template>
