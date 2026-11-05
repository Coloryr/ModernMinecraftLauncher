<script setup lang="ts">
import { inject } from "vue";
import VirtualList from "../../../components/ui/VirtualList.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import { MOD_ROW_H } from "../useServerInstance";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { mods, pickedMods, pickAllMods, toggled, busy, modPidOf, modFidOf, setModId, urlOf, setModUrl, lockedProjectIds, changedPaths, baseName } = ctx;
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
  <!-- 一张表：勾选 / 文件名 / 项目编号 / 文件编号 / 下载地址 / 哈希值
       编号是实例记录的，只读；只有下载地址能改。
       几百个模组逐个渲染太重，行虚拟化只画视口附近那些 -->
  <VirtualList class="si-mod-table" :count="mods.length" :row-h="MOD_ROW_H" :row-gap="0">
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
      <p v-if="!mods.length" class="empty-tip">{{ t("serverInstance.noMods") }}</p>
      <div v-for="row in mods.slice(start, end)" :key="row.path" class="si-mod-row"
        :class="{ 'is-changed': changedPaths.has(row.path) }">
        <span class="si-cell-check">
          <input type="checkbox" class="si-box" :checked="pickedMods.has(row.path)" :disabled="busy"
            @change="pickedMods = toggled(pickedMods, row.path)" />
        </span>
        <span class="si-cell-name" :title="row.path">{{ baseName(row.path) }}</span>
        <div class="si-cell-id">
          <input class="field-input si-cell-control si-id-input" :value="modPidOf(row.path)"
            :disabled="busy || (row.customId && lockedProjectIds.has(row.path))"
            :title="row.customId && lockedProjectIds.has(row.path)
              ? t('serverInstance.projectIdLocked') : undefined"
            spellcheck="false" @input="setModId(row.path, 'pid', $event)" />
          <em v-if="row.customId" class="si-id-custom">{{ t("serverInstance.customId") }}</em>
        </div>
        <div class="si-cell-id">
          <input class="field-input si-cell-control si-id-input" :value="modFidOf(row.path)"
            :disabled="busy" spellcheck="false"
            @input="setModId(row.path, 'fid', $event)" />
          <em v-if="row.customId" class="si-id-custom">{{ t("serverInstance.customId") }}</em>
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
