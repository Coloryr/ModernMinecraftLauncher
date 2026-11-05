<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { baseEntries, pickedBase, pickAllBase, toggleBase, format, busy, sizeText } = ctx;
</script>

<template>
<section class="si-section stretch">
  <div class="si-section-head">
    <span class="si-counter">{{ t("serverInstance.baseCount", { n: pickedBase.size }) }}</span>
    <BaseButton size="sm" :disabled="busy || !baseEntries.length" @click="pickAllBase(true)">
      {{ t("serverInstance.pickAll") }}
    </BaseButton>
    <BaseButton size="sm" :disabled="busy || !baseEntries.length" @click="pickAllBase(false)">
      {{ t("serverInstance.pickNone") }}
    </BaseButton>
  </div>
  <p v-if="!baseEntries.length" class="empty-tip">{{ t("serverInstance.baseEmpty") }}</p>
  <div v-else class="si-list">
    <label v-for="entry in baseEntries" :key="entry.path" class="si-row-item">
      <input type="checkbox" class="si-box" :checked="pickedBase.has(entry.path)"
        :disabled="busy || format === 'hmcl'" @change="toggleBase(entry.path)" />
      <span class="si-name" :title="entry.path">{{ entry.path }}{{ entry.dir ? "/" : "" }}</span>
      <span class="si-size">{{ entry.dir ? "—" : sizeText(entry.size) }}</span>
    </label>
  </div>
  <p class="field-hint">{{ t("serverInstance.baseHint") }}</p>
  <p class="field-hint">{{ t("serverInstance.baseIconHint") }}</p>
</section>
</template>
