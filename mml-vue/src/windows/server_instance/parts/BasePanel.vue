<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import BaseFileTree from "./BaseFileTree.vue";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { baseEntries, pickedBase, pickAllBase, toggleBase, refreshBase, busy, loading, currentUuid } = ctx;
</script>

<template>
<section class="si-section stretch flex min-h-0 flex-1 flex-col gap-2 overflow-hidden pr-[26px]">
  <div class="si-section-head flex items-center gap-2">
    <span class="si-counter shrink-0 text-meta text-dim [font-variant-numeric:tabular-nums]">{{ t("serverInstance.baseCount", { n: pickedBase.size }) }}</span>
    <BaseButton size="sm" :disabled="busy || !baseEntries.length" @click="pickAllBase(true)">
      {{ t("serverInstance.pickAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy || !baseEntries.length" @click="pickAllBase(false)">
      {{ t("serverInstance.pickNone") }}
    </BaseButton>
    <button class="ui-btn-icon" :disabled="busy || loading"
      v-tip="t('serverInstance.refresh')" :aria-label="t('serverInstance.refresh')"
      @click="refreshBase">
      <GlyphIcon name="refresh" :size="14" :weight="2" />
    </button>
  </div>
  <div v-if="!baseEntries.length" class="empty-tip">{{ t("serverInstance.baseEmpty") }}</div>
  <div v-else class="si-list min-h-0 flex-1 overflow-y-scroll rounded-card border border-border bg-card p-1 [scrollbar-gutter:stable]">
    <BaseFileTree :uuid="currentUuid" :root="baseEntries" :picked="pickedBase"
      :disabled="busy" @toggle="toggleBase" />
  </div>
</section>
</template>
