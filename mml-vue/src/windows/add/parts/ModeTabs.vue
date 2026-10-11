<script setup lang="ts">
// 添加实例 · 顶部模式切换：图标 + 渐变激活态，按数量等分填满整行
import { t } from "../../../lib/i18n";
import type { AddMode, AddModeTab } from "../types";

defineProps<{
  /** 模式页签清单，顺序即显示顺序 */
  tabs: AddModeTab[];
  /** 当前模式 */
  modelValue: AddMode;
  /** 创建中禁止切换模式（避免表单在提交过程中换掉） */
  disabled?: boolean;
}>();

const emit = defineEmits<{
  /** 切换模式 */
  (e: "update:modelValue", v: AddMode): void;
}>();
</script>

<template>
  <div class="add-modes" role="tablist">
    <button v-for="tab in tabs" :key="tab.id" type="button" role="tab" class="add-mode-btn"
      :class="{ active: modelValue === tab.id }" :aria-selected="modelValue === tab.id" :disabled="disabled"
      @click="emit('update:modelValue', tab.id)">
      <svg v-if="tab.icon === 'cube'" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor"
        stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3 4.5 7.5v9L12 21l7.5-4.5v-9L12 3z" />
        <path d="M4.5 7.5 12 12l7.5-4.5" />
        <path d="M12 12v9" />
      </svg>
      <svg v-else-if="tab.icon === 'box'" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor"
        stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 8v13H3V8" />
        <path d="M1 3h22v5H1z" />
        <path d="M10 12h4" />
      </svg>
      <svg v-else-if="tab.icon === 'folder'" viewBox="0 0 24 24" width="17" height="17" fill="none"
        stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
      </svg>
      <svg v-else viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18" />
      </svg>
      <span>{{ t(tab.labelKey) }}</span>
    </button>
  </div>
</template>

<style scoped>
.add-modes {
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: 1fr;
  gap: 8px;
  margin-bottom: 14px;
}

.add-mode-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-width: 0;
  padding: 11px 6px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  color: var(--text-dim);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

/* 窗口很窄时标签省略，不撑破按钮 */
.add-mode-btn>span {
  overflow: hidden;
  text-overflow: ellipsis;
}

.add-mode-btn:hover:not(:disabled) {
  border-color: var(--accent-border);
  color: var(--text);
  transform: translateY(-1px);
}

.add-mode-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.add-mode-btn:disabled {
  cursor: default;
  opacity: 0.6;
}

.add-mode-btn.active {
  border-color: transparent;
  background: var(--accent-grad);
  color: #fff;
  font-weight: 600;
  box-shadow: 0 6px 16px var(--accent-soft);
}

/* 激活态不再上浮：避免与背景渐变一起「跳」一下 */
.add-mode-btn.active:hover {
  transform: none;
}
</style>
