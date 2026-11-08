<script setup lang="ts">
import { inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import BaseFileTree from "./BaseFileTree.vue";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { baseEntries, pickedBase, pickAllBase, toggleBase, busy, currentUuid } = ctx;
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
  <div v-if="!baseEntries.length" class="empty-tip">{{ t("serverInstance.baseEmpty") }}</div>
  <div v-else class="si-list">
    <BaseFileTree :uuid="currentUuid" :root="baseEntries" :picked="pickedBase"
      :disabled="busy" @toggle="toggleBase" />
  </div>
  <p class="field-hint">{{ t("serverInstance.baseHint") }}</p>
  <p class="field-hint">{{ t("serverInstance.baseIconHint") }}</p>
</section>
</template>
