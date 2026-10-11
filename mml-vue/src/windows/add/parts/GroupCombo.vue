<script setup lang="ts">
// 添加实例 · 分组组合框：可输入、聚焦时列出已有分组、允许自定义
// 留空 = 默认分组（不进下拉）
//
// 输入框里始终是**组名**（用户手输的就是名字），提交时才由 api.resolveGroupId
// 转成分组 uuid；候选列表来自后端的 uuid + 名字。
//
// 展开态由父组件持有（`v-model:open`）：Esc 的优先级链里要先收下拉再谈关窗。
import { computed } from "vue";
import type { GroupDto } from "../../../lib/bindings";
import { t } from "../../../lib/i18n";

const props = defineProps<{
  /** 输入框里的组名，空 = 默认分组 */
  modelValue: string;
  /** 已有分组候选，下拉里按输入内容过滤 */
  groups: GroupDto[];
  /** 下拉是否展开，由父组件持有以配合 Esc 的优先级链 */
  open: boolean;
}>();

const emit = defineEmits<{
  /** 组名变化 */
  (e: "update:modelValue", v: string): void;
  /** 请求展开 / 收起下拉 */
  (e: "update:open", v: boolean): void;
}>();

/** 按输入内容过滤已有分组（大小写不敏感） */
const matched = computed(() => {
  const q = props.modelValue.trim().toLowerCase();
  return props.groups.filter((g) => g.name.toLowerCase().includes(q));
});

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLInputElement).value);
  emit("update:open", true);
}

function pick(name: string) {
  emit("update:modelValue", name);
  emit("update:open", false);
}
</script>

<template>
  <div class="group-combo">
    <input :value="modelValue" class="field-input" :placeholder="t('add.groupPlaceholder')" spellcheck="false"
      autocomplete="off" role="combobox" aria-autocomplete="list" :aria-expanded="open" @input="onInput"
      @focus="emit('update:open', true)" @blur="emit('update:open', false)" />
    <div v-if="open" class="group-drop" role="listbox">
      <button v-for="g in matched" :key="g.uuid" type="button" class="group-opt" role="option"
        :aria-selected="modelValue === g.name" @mousedown.prevent @click="pick(g.name)">
        {{ g.name }}
      </button>
      <div v-if="!matched.length" class="empty-tip">{{ t("add.groupNone") }}</div>
    </div>
  </div>
</template>

<style scoped>
.group-combo {
  position: relative;
}

.group-opt[aria-selected="true"] {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}
</style>
<style scoped src="../../../styles/parts/group-combo.css"></style>
