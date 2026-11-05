<script setup lang="ts">
import { inject } from "vue";
import VirtualList from "../../../components/ui/VirtualList.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import { MOD_ROW_H } from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { packs, pickedPacks, pickAllPacks, toggled, busy, packPidOf, packFidOf, packUrlOf, setPackId, setPackUrl, lockedProjectIds, changedPaths, baseName } = ctx;
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
  </div>
  <VirtualList class="si-mod-table" :count="packs.length" :row-h="MOD_ROW_H" :row-gap="0">
    <template #header>
      <div class="si-mod-head">
        <span>{{ t("serverInstance.colSync") }}</span>
        <span>{{ t("serverInstance.colFile") }}</span>
        <span>{{ t("serverInstance.colProject") }}</span>
        <span>{{ t("serverInstance.colFileId") }}</span>
        <span>{{ t("serverInstance.colUrl") }}</span>
        <span>{{ t("serverInstance.colSha1") }}</span>
      </div>
    </template>
    <template #default="{ start, end }">
      <p v-if="!packs.length" class="empty-tip">{{ t("serverInstance.noPacks") }}</p>
      <div v-for="row in packs.slice(start, end)" :key="row.path" class="si-mod-row"
        :class="{ 'is-changed': changedPaths.has(row.path) }">
        <span class="si-cell-check">
          <input type="checkbox" class="si-box" :checked="pickedPacks.has(row.path)" :disabled="busy"
            @change="pickedPacks = toggled(pickedPacks, row.path)" />
        </span>
        <span class="si-cell-name" :title="row.path">{{ baseName(row.path) }}</span>
        <div class="si-cell-id">
          <input class="field-input si-cell-control si-id-input" :value="packPidOf(row.path)"
            :disabled="busy || (row.customId && lockedProjectIds.has(row.path))"
            :title="row.customId && lockedProjectIds.has(row.path)
              ? t('serverInstance.projectIdLocked') : undefined"
            spellcheck="false" @input="setPackId(row.path, 'pid', $event)" />
          <em v-if="row.customId" class="si-id-custom">{{ t("serverInstance.customId") }}</em>
        </div>
        <div class="si-cell-id">
          <input class="field-input si-cell-control si-id-input" :value="packFidOf(row.path)"
            :disabled="busy" spellcheck="false"
            @input="setPackId(row.path, 'fid', $event)" />
          <em v-if="row.customId" class="si-id-custom">{{ t("serverInstance.customId") }}</em>
        </div>
        <input class="field-input si-cell-control" :value="packUrlOf(row.path)" :disabled="busy"
          spellcheck="false" :placeholder="t('serverInstance.cellUrl')"
          @input="setPackUrl(row.path, $event)" />
        <span class="si-cell-hash" :title="row.sha1">{{ row.sha1 || "—" }}</span>
      </div>
    </template>
  </VirtualList>
</section>
</template>
