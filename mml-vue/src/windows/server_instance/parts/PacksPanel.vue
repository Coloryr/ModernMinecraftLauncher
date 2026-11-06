<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import VirtualList from "../../../components/ui/VirtualList.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import { MOD_ROW_H } from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const {
  packs,
  pickedPacks,
  pickAllPacks,
  toggled,
  busy,
  loading,
  refreshPacks,
  packPidOf,
  packFidOf,
  packUrlOf,
  setPackId,
  setPackUrl,
  packIdTypeOf,
  isPackCustomType,
  setPackIdType,
  lockedProjectIds,
  changedPaths,
  baseName,
  packChanges,
  addedPacks,
  removedPacks,
  modifiedPacks,
} = ctx;
</script>

<template>
<section class="si-section stretch">
  <div class="si-section-head">
    <span class="si-counter">{{ pickedPacks.size }} / {{ packs.length }}</span>
    <BaseButton size="sm" :disabled="busy" @click="pickAllPacks(true)">
      {{ t("serverInstance.pickAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy" @click="pickAllPacks(false)">
      {{ t("serverInstance.pickNone") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy || loading" @click="refreshPacks">
      <GlyphIcon name="refresh" :size="13" :weight="2" />
      {{ t("serverInstance.refresh") }}
    </BaseButton>
  </div>
  <div v-if="packChanges" class="si-changes">
    <span class="si-change add" :title="packChanges.added.join('\n')">+{{ addedPacks.size }}</span>
    <span class="si-change del" :title="packChanges.removed.join('\n')">-{{ removedPacks.size }}</span>
    <span class="si-change mod" :title="packChanges.modified.join('\n')">·{{ modifiedPacks.size }}</span>
  </div>
  <div v-if="packChanges && packChanges.removed.length" class="si-changes si-changes-list">
    <span v-for="path in packChanges.removed" :key="path" class="si-change del">- {{ baseName(path) }}</span>
  </div>
  <div class="si-table-card">
    <VirtualList class="si-mod-table" :count="packs.length" :row-h="MOD_ROW_H" :row-gap="0">
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
        <p v-if="!packs.length" class="empty-tip">{{ t("serverInstance.noPacks") }}</p>
        <div v-for="row in packs.slice(start, end)" :key="row.path" class="si-mod-row"
          :class="{
            'is-changed': changedPaths.has(row.path),
            'is-added': addedPacks.has(row.path),
            'is-modified': modifiedPacks.has(row.path),
          }">
          <span class="si-cell-check">
            <input type="checkbox" class="si-box" :checked="pickedPacks.has(row.path)" :disabled="busy"
              @change="pickedPacks = toggled(pickedPacks, row.path)" />
          </span>
          <span class="si-cell-name" :title="row.path">
            <em v-if="addedPacks.has(row.path)" class="si-mark add">+</em>
            <em v-else-if="modifiedPacks.has(row.path)" class="si-mark mod">·</em>
            {{ baseName(row.path) }}
          </span>
          <select class="field-select si-cell-control si-id-type" :value="packIdTypeOf(row.path, row.customId)"
            :disabled="busy" @change="setPackIdType(row.path, $event)">
            <option value="online">{{ t("serverInstance.idType.online") }}</option>
            <option value="custom">{{ t("serverInstance.idType.custom") }}</option>
          </select>
          <div class="si-cell-id">
            <input class="field-input si-cell-control si-id-input" :value="packPidOf(row.path)"
              :disabled="busy || (isPackCustomType(row.path, row.customId) && lockedProjectIds.has(row.path))"
              :title="isPackCustomType(row.path, row.customId) && lockedProjectIds.has(row.path)
                ? t('serverInstance.projectIdLocked') : undefined"
              spellcheck="false" @input="setPackId(row.path, 'pid', $event)" />
          </div>
          <div class="si-cell-id">
            <input class="field-input si-cell-control si-id-input" :value="packFidOf(row.path)"
              :disabled="busy" spellcheck="false"
              @input="setPackId(row.path, 'fid', $event)" />
          </div>
          <input class="field-input si-cell-control" :value="packUrlOf(row.path)" :disabled="busy"
            spellcheck="false" :placeholder="t('serverInstance.cellUrl')"
            @input="setPackUrl(row.path, $event)" />
          <span class="si-cell-hash" :title="row.sha1">{{ row.sha1 || "—" }}</span>
        </div>
      </template>
    </VirtualList>
  </div>
</section>
</template>
