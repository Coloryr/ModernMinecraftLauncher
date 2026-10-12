<script setup lang="ts">
// 标题栏上的当前实例（名字 + 版本），走 WindowFrame 的 head-right 槽渲染。
// 资源管理窗口与添加资源窗口共用：自带 scoped 样式，标题栏不在各窗口自己的布局容器里，
// 窗口级样式够不到它
/** 标题栏 chip 需要的实例信息，只取渲染用得到的两个字段 */
interface InstanceChipData {
  /** 实例名 */
  name: string;
  /** 游戏版本，如 1.20.1 */
  version: string;
}

defineProps<{
  /** 当前实例；没有实例时整个 chip 不渲染 */
  instance: InstanceChipData | null;
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
   名字长了省略，别把右边的按钮与窗口键挤走。
   宽度是"能显示多少个字"的旋钮：名字 260px 约 21 个汉字，版本名再占一点点 */
.inst-chip {
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  max-width: 340px;
  font-size: 12px;
  color: var(--text-dim);
}

.inst-chip-name {
  max-width: 260px;
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
