<script setup lang="ts">
// 动作下拉：键盘 / 鼠标 / 滚轮分组的选项
//
// 选项来自后端 gamepad_get_actions，保证每个名字都能被映射解析；
// 分组按名字前缀分，前端不另维护一份键表。
import { computed } from "vue";
import { t } from "../../../lib/i18n";

const props = defineProps<{
  /** 可选动作名，全部列出 */
  actions: string[];
  /** 当前选中的动作 */
  modelValue: string;
}>();

const emit = defineEmits<{
  /** 选中的动作名变化 */
  (e: "update:modelValue", value: string): void;
}>();

/** 按前缀分好的三组 */
const groups = computed(() => [
  {
    label: t("winGamepad.keys"),
    items: props.actions.filter(
      (item) => !item.startsWith("Mouse") && !item.startsWith("Scroll"),
    ),
  },
  {
    label: t("winGamepad.mouseActions"),
    items: props.actions.filter((item) => item.startsWith("Mouse")),
  },
  {
    label: t("winGamepad.scrollActions"),
    items: props.actions.filter((item) => item.startsWith("Scroll")),
  },
]);
</script>

<template>
  <select
    class="field-select"
    :value="modelValue"
    @change="emit('update:modelValue', ($event.target as HTMLSelectElement).value)"
  >
    <optgroup v-for="group in groups" :key="group.label" :label="group.label">
      <option v-for="item in group.items" :key="item" :value="item">{{ item }}</option>
    </optgroup>
  </select>
</template>
