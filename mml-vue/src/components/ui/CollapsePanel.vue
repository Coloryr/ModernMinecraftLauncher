<script setup lang="ts">
// 展开/收起动画容器：外层 grid 的 grid-template-rows 在 0fr / 1fr 之间过渡，内层 overflow 负责裁切。
// 相比测 scrollHeight 的方案：不需要 JS、不需要 v-if 卸载内容（表单状态得以保留），收起后高度精确为 0。
defineProps<{
  /** 展开状态；切换即触发高度过渡 */
  open: boolean;
}>();
</script>

<template>
  <div class="collapse" :class="{ open }">
    <div class="collapse-inner">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.collapse {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 0.22s var(--ease-out);
}

.collapse.open {
  grid-template-rows: 1fr;
}

/* min-height: 0 是 grid 子项能被压到 0 高度的必要条件 */
.collapse-inner {
  overflow: hidden;
  min-height: 0;
}
</style>
