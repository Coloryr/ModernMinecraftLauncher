<script setup lang="ts">
// 拖动时的跟手浮起副本：统一外观（底色 + 强调色描边 + 大阴影 + 轻微放大），内容由插槽给
//
// 位置是**指针坐标**，所以整体往左上挪一截，别被光标盖住；`pointer-events: none`
// 保证它不参与拖动过程中的命中判定。
defineProps<{
  /** 指针横坐标，视口坐标 */
  x: number;
  /** 指针纵坐标，视口坐标 */
  y: number;
}>();
</script>

<template>
  <Teleport to="body">
    <!-- 遮罩：拖动期间挡住底下的界面，按钮不会跟着高亮、也不会被点到 -->
    <div class="drag-shield" />
    <div class="drag-ghost" :style="{ left: x + 'px', top: y + 'px' }">
      <slot />
    </div>
  </Teleport>
</template>

<style scoped>
/* 铺满窗口接住指针事件；透明，只负责"吃掉"交互。命中判定见 useDragGhost 的
   elementBeneathShield —— 它要跳过这一层 */
.drag-shield {
  position: fixed;
  inset: 0;
  z-index: 499;
  cursor: grabbing;
}

.drag-ghost {
  position: fixed;
  /* 在弹窗（100）之上、轻提示（600）之下 */
  z-index: 500;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 320px;
  padding: 8px 12px;
  border: 1px solid var(--accent-border);
  border-radius: 10px;
  background: var(--bg-modal);
  color: var(--text);
  font-size: 13px;
  box-shadow: var(--shadow-lg);
  transform: translate(-40%, -50%) scale(1.04);
  pointer-events: none;
}
</style>
