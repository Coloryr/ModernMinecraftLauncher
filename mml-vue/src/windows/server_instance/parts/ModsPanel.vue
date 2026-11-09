<script setup lang="ts">
import { computed, inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import SearchBox from "../../../components/ui/SearchBox.vue";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import VirtualList from "../../../components/ui/VirtualList.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import { MOD_ROW_H } from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const {
  mods,
  modsShown,
  modQuery,
  modView,
  modGroups,
  pickedMods,
  pickAllMods,
  toggled,
  busy,
  loading,
  refreshMods,
  modPidOf,
  modFidOf,
  setModId,
  modIdTypeOf,
  isCustomType,
  setModIdType,
  urlOf,
  setModUrl,
  modTextOf,
  setModText,
  optionalOf,
  toggleModOptional,
  lockedProjectIds,
  changedPaths,
  baseName,
  modChanges,
  addedMods,
  removedMods,
  modifiedMods,
} = ctx;

/** 视图切换：普通表格 / 分组按变化 */
const viewOptions = computed(() => [
  { value: "normal", label: t("serverInstance.viewNormal"), icon: "list" },
  { value: "group", label: t("serverInstance.viewGroup"), icon: "grid" },
]);
</script>

<template>
<section class="si-section stretch">
  <div class="si-section-head">
    <span class="si-counter">{{ pickedMods.size }} / {{ mods.length }}</span>
    <span class="si-head-tools">
      <SearchBox v-model="modQuery" :placeholder="t('serverInstance.modSearch')" width="200px" />
      <SegmentedTabs v-model="modView" :options="viewOptions" />
    </span>
    <!-- 勾选与刷新统一放在工具条最右端 -->
    <BaseButton size="sm" :disabled="busy" @click="pickAllMods(true)">
      {{ t("serverInstance.pickAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy" @click="pickAllMods(false)">
      {{ t("serverInstance.pickNone") }}
    </BaseButton>
    <button class="ui-btn-icon" :disabled="busy || loading"
      v-tip="t('serverInstance.refresh')" :aria-label="t('serverInstance.refresh')"
      @click="refreshMods">
      <GlyphIcon name="refresh" :size="14" :weight="2" />
    </button>
  </div>
  <div v-if="modChanges" class="si-changes">
    <span class="si-change add" :title="modChanges.added.join('\n')">+{{ addedMods.size }}</span>
    <span class="si-change del" :title="modChanges.removed.join('\n')">-{{ removedMods.size }}</span>
    <span class="si-change mod" :title="modChanges.modified.join('\n')">·{{ modifiedMods.size }}</span>
  </div>
  <div v-if="modChanges && modChanges.removed.length" class="si-changes si-changes-list">
    <span v-for="path in modChanges.removed" :key="path" class="si-change del">- {{ baseName(path) }}</span>
  </div>
  <!-- 一张表：勾选 / 可选 / 文件名 / 编号类型 / 项目编号 / 文件编号 / 下载地址 / 哈希值
       几百个模组逐个渲染太重，行虚拟化只画视口附近那些 -->
  <!-- 分组：按 新增 / 修改 / 删除 回顾，只读 -->
  <div v-if="modView === 'group'" class="si-table-card si-group-card">
    <p v-if="!modGroups.some((group) => group.rows.length)" class="empty-tip">
      {{ t("serverInstance.configNoChange") }}
    </p>
    <template v-for="group in modGroups" :key="group.key">
      <div v-if="group.rows.length">
        <div class="si-group-head" :class="`is-${group.key}`">
          {{ t(`serverInstance.group.${group.key}`) }}
          <span class="si-group-count">{{ group.rows.length }}</span>
        </div>
        <div v-for="row in group.rows" :key="row.path" class="si-group-row">
          <span>{{ row.name }}</span>
          <span class="si-group-path">{{ row.path }}</span>
        </div>
      </div>
    </template>
  </div>
  <div v-else class="si-table-card">
    <VirtualList class="si-mod-table" :count="modsShown.length" :row-h="MOD_ROW_H" :row-gap="0">
      <template #header>
        <div class="si-mod-head">
          <span>{{ t("serverInstance.colSync") }}</span>
          <span>{{ t("serverInstance.colOptional") }}</span>
          <span>{{ t("serverInstance.colFile") }}</span>
          <span>{{ t("serverInstance.colText") }}</span>
          <span>{{ t("serverInstance.colIdType") }}</span>
          <span>{{ t("serverInstance.colProject") }}</span>
          <span>{{ t("serverInstance.colFileId") }}</span>
          <span>{{ t("serverInstance.colUrl") }}</span>
          <span>{{ t("serverInstance.colSha1") }}</span>
        </div>
      </template>
      <template #default="{ start, end }">
        <p v-if="!modsShown.length" class="empty-tip">
          {{ mods.length ? t("serverInstance.noModMatch") : t("serverInstance.noMods") }}
        </p>
        <div v-for="row in modsShown.slice(start, end)" :key="row.path" class="si-mod-row"
          :class="{
            'is-changed': changedPaths.has(row.path),
            'is-added': addedMods.has(row.path),
            'is-modified': modifiedMods.has(row.path),
          }">
          <span class="si-cell-check">
            <input type="checkbox" class="si-box" :checked="pickedMods.has(row.path)" :disabled="busy"
              @change="pickedMods = toggled(pickedMods, row.path)" />
          </span>
          <span class="si-cell-check">
            <input type="checkbox" class="si-box" :checked="optionalOf(row.path)" :disabled="busy"
              :title="t('serverInstance.optionalHint')" @change="toggleModOptional(row.path)" />
          </span>
          <span class="si-cell-name" :title="row.path">
            <em v-if="addedMods.has(row.path)" class="si-mark add">+</em>
            <em v-else-if="modifiedMods.has(row.path)" class="si-mark mod">·</em>
            {{ baseName(row.path) }}
          </span>
          <input class="field-input si-cell-control" :value="modTextOf(row.path)" :disabled="busy"
            spellcheck="false" :placeholder="t('serverInstance.cellText')"
            :title="modTextOf(row.path)" @input="setModText(row.path, $event)" />
          <select class="field-select si-cell-control si-id-type" :value="modIdTypeOf(row.path, row.customId)"
            :disabled="busy" @change="setModIdType(row.path, $event)">
            <option value="online">{{ t("serverInstance.idType.online") }}</option>
            <option value="custom">{{ t("serverInstance.idType.custom") }}</option>
          </select>
          <div class="si-cell-id">
            <input class="field-input si-cell-control si-id-input" :value="modPidOf(row.path)"
              :disabled="busy || (isCustomType(row.path, row.customId) && lockedProjectIds.has(row.path))"
              :title="isCustomType(row.path, row.customId) && lockedProjectIds.has(row.path)
                ? t('serverInstance.projectIdLocked') : undefined"
              spellcheck="false" @input="setModId(row.path, 'pid', $event)" />
          </div>
          <div class="si-cell-id">
            <input class="field-input si-cell-control si-id-input" :value="modFidOf(row.path)"
              :disabled="busy" spellcheck="false"
              @input="setModId(row.path, 'fid', $event)" />
          </div>
          <input class="field-input si-cell-control" :value="urlOf(row.path)" :disabled="busy"
            spellcheck="false" :placeholder="t('serverInstance.cellUrl')"
            @input="setModUrl(row.path, $event)" />
          <span class="si-cell-hash" :title="row.sha1">{{ row.sha1 || "—" }}</span>
        </div>
      </template>
    </VirtualList>
  </div>
</section>
</template>
