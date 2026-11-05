<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import VirtualList from "../../../components/ui/VirtualList.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import { MOD_ROW_H } from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const {
  mods,
  pickedMods,
  pickAllMods,
  toggled,
  busy,
  modPidOf,
  modFidOf,
  setModId,
  modIdTypeOf,
  isCustomType,
  setModIdType,
  urlOf,
  setModUrl,
  lockedProjectIds,
  changedPaths,
  baseName,
  modChanges,
  addedMods,
  removedMods,
  modifiedMods,
} = ctx;
</script>

<template>
<section class="si-section stretch">
  <div class="si-section-head">
    <span class="si-counter">{{ pickedMods.size }} / {{ mods.length }}</span>
    <BaseButton size="sm" :disabled="busy" @click="pickAllMods(true)">
      {{ t("serverInstance.pickAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy" @click="pickAllMods(false)">
      {{ t("serverInstance.pickNone") }}
    </BaseButton>
  </div>
  <div v-if="modChanges" class="si-changes">
    <span class="si-change add" :title="modChanges.added.join('\n')">+{{ addedMods.size }}</span>
    <span class="si-change del" :title="modChanges.removed.join('\n')">-{{ removedMods.size }}</span>
    <span class="si-change mod" :title="modChanges.modified.join('\n')">·{{ modifiedMods.size }}</span>
  </div>
  <div v-if="modChanges && modChanges.removed.length" class="si-changes si-changes-list">
    <span v-for="path in modChanges.removed" :key="path" class="si-change del">- {{ baseName(path) }}</span>
  </div>
  <!-- 一张表：勾选 / 文件名 / 编号类型 / 项目编号 / 文件编号 / 下载地址 / 哈希值
       几百个模组逐个渲染太重，行虚拟化只画视口附近那些 -->
  <VirtualList class="si-mod-table" :count="mods.length" :row-h="MOD_ROW_H" :row-gap="0">
    <template #header>
      <div class="si-mod-head">
        <span>{{ t("serverInstance.colSync") }}</span>
        <span>{{ t("serverInstance.colFile") }}</span>
        <span>{{ t("serverInstance.colIdType") }}</span>
        <span>{{ t("serverInstance.colProject") }}</span>
        <span>{{ t("serverInstance.colFileId") }}</span>
        <span>{{ t("serverInstance.colUrl") }}</span>
        <span>{{ t("serverInstance.colSha1") }}</span>
      </div>
    </template>
    <template #default="{ start, end }">
      <p v-if="!mods.length" class="empty-tip">{{ t("serverInstance.noMods") }}</p>
      <div v-for="row in mods.slice(start, end)" :key="row.path" class="si-mod-row"
        :class="{
          'is-changed': changedPaths.has(row.path),
          'is-added': addedMods.has(row.path),
          'is-modified': modifiedMods.has(row.path),
        }">
        <span class="si-cell-check">
          <input type="checkbox" class="si-box" :checked="pickedMods.has(row.path)" :disabled="busy"
            @change="pickedMods = toggled(pickedMods, row.path)" />
        </span>
        <span class="si-cell-name" :title="row.path">
          <em v-if="addedMods.has(row.path)" class="si-mark add">+</em>
          <em v-else-if="modifiedMods.has(row.path)" class="si-mark mod">·</em>
          {{ baseName(row.path) }}
        </span>
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
          <em v-if="isCustomType(row.path, row.customId)" class="si-id-custom">{{ t("serverInstance.customId") }}</em>
        </div>
        <div class="si-cell-id">
          <input class="field-input si-cell-control si-id-input" :value="modFidOf(row.path)"
            :disabled="busy" spellcheck="false"
            @input="setModId(row.path, 'fid', $event)" />
          <em v-if="isCustomType(row.path, row.customId)" class="si-id-custom">{{ t("serverInstance.customId") }}</em>
        </div>
        <input class="field-input si-cell-control" :value="urlOf(row.path)" :disabled="busy"
          spellcheck="false" :placeholder="t('serverInstance.cellUrl')"
          @input="setModUrl(row.path, $event)" />
        <span class="si-cell-hash" :title="row.sha1">{{ row.sha1 || "—" }}</span>
      </div>
    </template>
  </VirtualList>
</section>
</template>
