<script setup lang="ts">
// 标题栏上的当前实例（名字 + 版本）
//
// 原先它在左侧分类栏顶部（占了左栏最显眼的位置）；用户要求挪到标题栏，于是走
// WindowFrame 的 head-right 槽渲染。自带 scoped 样式：标题栏不在 `.resource-layout`
// 里，窗口级 resource.css 的那套前缀规则够不到它。
import type { ResourceInstance } from "../composables/useResourceData";

defineProps<{
  /** 当前实例；没有实例时整个 chip 不渲染 */
  instance: ResourceInstance | null;
}>();
</script>

<template>
  <span v-if="instance" class="inst-chip" :title="`${instance.name} · ${instance.version}`">
    <span class="inst-chip-name">{{ instance.name }}</span>
    <span class="inst-chip-ver">{{ instance.version }}</span>
  </span>
</template>

<style scoped>
/* 标题栏里的一小段上下文：**不套框**（用户要求），就是一行文字；
   名字长了省略，别把右边的按钮与窗口键挤走 */
.inst-chip {
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  max-width: 220px;
  font-size: 12px;
  color: var(--text-dim);
}

.inst-chip-name {
  max-width: 140px;
  color: var(--text);
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.inst-chip-ver {
  flex-shrink: 0;
}
</style>
