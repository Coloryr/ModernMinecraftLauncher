<script setup lang="ts">
// 渲染带 `§` 格式码的文本，如材质包 / 数据包描述、存档名
//
// 只影响显示：原始字符串由调用方原样持有，这里不修改也不回写。
// 无格式码时退化成一段无样式文字，所以对普通文本也能直接用。
import { computed } from "vue";
import { parseFormatting, segmentStyle } from "../../lib/formatting";

const props = defineProps<{
  /** 原始文本，可含 `§` 颜色码 */
  text: string;
}>();

const segments = computed(() => parseFormatting(props.text));
</script>

<template>
  <!-- 外层 span 是为了不破坏父级的布局（inline 容器，逐段铺开） -->
  <span class="fmt-text">
    <span v-for="(seg, i) in segments" :key="i" :style="segmentStyle(seg)">{{ seg.text }}</span>
  </span>
</template>

<style scoped>
.fmt-text {
  /* 让内部的 span 跟着父级排版走，自己不加任何盒模型特性 */
  display: inline;
}
</style>
