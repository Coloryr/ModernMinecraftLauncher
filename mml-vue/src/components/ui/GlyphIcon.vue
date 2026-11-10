<script setup lang="ts">
// 小图标：原来用文字字形当图标的地方统一走这里，如 ＋ ✓ ✕ ‹ › ▾ − ×
//
// 为什么不留着字形：
// - 字形形状 / 粗细 / 位置随字体走：中文字体把 ＋ 画在 em 框偏上，盒子居中了笔画仍显偏上；
// - ▾ 这类三角按字号画得太小，粗细还随字号变形；
// - 字号一改图标就变形，而 SVG 的尺寸由 size 定死，线宽由 weight 调。
//
// 几何数据在 ./glyphs.ts：组件只负责套 <svg> 与描边属性，新增图标改数据表即可，
// IconTabs / SegmentedTabs 也直接复用同一份画法。
import { GLYPHS, type GlyphName } from "./glyphs";

withDefaults(
  defineProps<{
    /** 图标名，取 glyphs.ts 里的键 */
    name: GlyphName;
    /** 边长，单位 px */
    size?: number;
    /** 线宽，无单位，对应 SVG 的 stroke-width */
    weight?: number;
  }>(),
  {
    size: 14,
    weight: 2,
  },
);
</script>

<template>
  <!-- 数据表是静态常量，不含用户输入；v-html 只用来铺 SVG 子元素，省掉几十条 v-else-if -->
  <svg class="glyph" :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor"
    :stroke-width="weight" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
    v-html="GLYPHS[name]"></svg>
</template>

<style scoped>
.glyph {
  /* 与文字同行时（如"＋ 添加一行"）按文字中线对齐，别骑在基线上；
     当 flex 子项时 vertical-align 不参与，由父级的 align-items 居中 */
  display: inline-block;
  vertical-align: -0.14em;
  flex-shrink: 0;
}
</style>
