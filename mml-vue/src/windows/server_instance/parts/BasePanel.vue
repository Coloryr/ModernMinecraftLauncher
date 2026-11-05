<script setup lang="ts">
import { computed, inject } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import { t } from "../../../lib/i18n";
import { ServerInstanceKey } from "../context";
import BaseFileTree from "./BaseFileTree.vue";
const ctx = inject(ServerInstanceKey);
if (!ctx) throw new Error("server instance context missing");
const { baseEntries, pickedBase, pickAllBase, toggleBase, format, busy, currentUuid } = ctx;

/** 根条目全勾上就等于整个实例进包 */
const allPicked = computed(
  () =>
    baseEntries.value.length > 0 &&
    baseEntries.value.every((entry) => pickedBase.value.has(entry.path)),
);

/** 勾 / 取消整个实例：根条目全选或全不选 */
function toggleAll() {
  pickAllBase(!allPicked.value);
}
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
    <!-- 根行：整份实例进包；下面才是实例目录里的条目，目录可以逐层展开 -->
    <label class="si-row-item si-root">
      <span class="si-caret"></span>
      <input type="checkbox" class="si-box" :checked="allPicked" :disabled="busy || format === 'hmcl'"
        @change="toggleAll" />
      <span class="si-name">{{ t("serverInstance.baseAll") }}</span>
    </label>
    <BaseFileTree :uuid="currentUuid" :root="baseEntries" :picked="pickedBase"
      :disabled="busy || format === 'hmcl'" @toggle="toggleBase" />
  </div>
  <p class="field-hint">{{ t("serverInstance.baseHint") }}</p>
  <p class="field-hint">{{ t("serverInstance.baseIconHint") }}</p>
</section>
</template>

<style scoped>
/* 根行没有箭头，但要占住与树行相同的宽度，勾选框才对得齐 */
.si-root .si-caret {
  width: 16px;
  flex-shrink: 0;
}

.si-root {
  cursor: default;
}
</style>
