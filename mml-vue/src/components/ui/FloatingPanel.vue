<script setup lang="ts">
// 无遮罩的浮动面板：可同时开几个的工具窗外壳（标题 + ✕ + 内容）
//
// 与 BaseModal 的分工：那个是**模态**弹窗，整屏遮罩 + 居中，同一时刻只该有一个；
// 这个不画遮罩、不挡页面，尺寸固定，供"边跑边看、可以同时开几个"的工具窗用。
// 叠放时的错位与前后顺序由调用方负责，见 windows/log/LogPopups.vue。
import GlyphIcon from "./GlyphIcon.vue";

withDefaults(
  defineProps<{
    /** 标题 */
    title?: string;
    /** 面板宽度，单位 px；视口太窄时按 max-width 收 */
    width?: number;
    /** 面板高度，CSS 长度；视口太矮时按 max-height 收 */
    height?: string;
  }>(),
  {
    title: "",
    width: 760,
    height: "560px",
  },
);

const emit = defineEmits<{
  /** 点右上角 ✕ */
  (e: "close"): void;
}>();
</script>

<template>
  <div class="float-panel" :style="{ width: width + 'px', height: height }">
    <div class="fp-head">
      <h3>{{ title }}</h3>
      <!-- 标题栏右侧扩展区，与 WindowFrame 同名同义：调用方一套控件两个外壳都能用 -->
      <slot name="head-right" />
      <button class="fp-x" @click="emit('close')">
        <GlyphIcon name="close" :size="14" :weight="2.2" />
      </button>
    </div>
    <div class="fp-body">
      <slot />
    </div>
  </div>
</template>

<style scoped>
/* 卡片外观与 BaseModal 同一套（底色 / 描边 / 圆角 / 阴影），只是不画遮罩 */
.float-panel {
  display: flex;
  flex-direction: column;
  max-width: calc(100vw - 80px);
  max-height: calc(100vh - var(--titlebar-h) - 60px);
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 14px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.fp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
  padding: 10px 10px 10px 16px;
  border-bottom: 1px solid var(--border);
}

.fp-head h3 {
  font-size: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  /* flex 项要允许收缩，标题太长时先省略号，不去挤右边的控件 */
  min-width: 0;
}

.fp-x {
  border: none;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  padding: 4px 6px;
  border-radius: 6px;
  line-height: 1;
  flex-shrink: 0;
}

.fp-x:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.fp-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 左右与 .fp-head 的 16 对齐：标题与内容同一条左边缘 */
  padding: 12px 16px;
}
</style>
