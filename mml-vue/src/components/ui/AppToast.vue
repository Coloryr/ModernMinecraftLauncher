<script setup lang="ts">
// 全局轻提示组件：所有窗口统一使用（App.vue 挂载一次）
import { dismissToast, toasts } from "../../lib/toast";
</script>

<template>
  <Teleport to="body">
    <div class="toast-wrap">
      <TransitionGroup name="toast">
        <div v-for="t in toasts" :key="t.id" class="toast-item" @click="dismissToast(t.id)">
          {{ t.msg }}
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-wrap {
  position: fixed;
  bottom: 34px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 600;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  pointer-events: none;
}

.toast-item {
  pointer-events: auto;
  background: var(--bg-modal);
  border: 1px solid var(--accent-border);
  color: var(--text);
  font-size: 13px;
  padding: 11px 22px;
  border-radius: 10px;
  box-shadow: var(--shadow-lg);
  max-width: 80vw;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
