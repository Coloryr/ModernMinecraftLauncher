<script setup lang="ts">
// 拖入遮罩：四个落点共用。只画外观——是否显示由调用方 v-if、标题由调用方算好传入。
// 拖的是哪一档、该显示哪句，见 useFileDrop 的 dragKind 与各窗口的选键逻辑。
import GlyphIcon from "./GlyphIcon.vue";

defineProps<{
  /** 标题，调用方已 `t()` 好的成品 */
  title: string;
}>();
</script>

<template>
  <div class="drop-layer">
    <div class="drop-box">
      <GlyphIcon name="download" :size="42" :weight="1.6" />
      <h2>{{ title }}</h2>
    </div>
  </div>
</template>

<style scoped>
/* 事件穿透：拖放走窗口级 DOM 事件，遮罩不能挡住 dragover / drop */
.drop-layer {
  position: fixed;
  inset: 0;
  z-index: 300;
  background: var(--overlay);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.drop-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 44px 64px;
  border: 2px dashed var(--accent);
  border-radius: 20px;
  background: var(--bg-card);
  color: var(--accent);
  box-shadow: var(--shadow-lg);
}

.drop-box h2 {
  font-size: 19px;
  font-weight: 700;
  color: var(--text);
}
</style>
