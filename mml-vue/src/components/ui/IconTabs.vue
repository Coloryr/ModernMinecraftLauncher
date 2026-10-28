<script setup lang="ts" generic="T extends string">
// 通用图标选项卡：图标 + 渐变激活态，按数量等分填满整行
//
// 原「添加实例」窗口的 ModeTabs，导出窗口要用同一套观感，所以挪到这里共用。
// 图标名固定五个：cube / box / folder / globe / download，其它值回落成地球。
import { t } from "../../lib/i18n";

defineProps<{
  /** 页签清单，顺序即显示顺序 */
  tabs: Array<{ id: T; labelKey: string; icon: string; suffix?: string }>;
  /** 当前页签 */
  modelValue: T;
  /** 禁止切换，比如表单正在提交 */
  disabled?: boolean;
}>();

const emit = defineEmits<{
  /** 切换页签 */
  (e: "update:modelValue", v: T): void;
}>();
</script>

<template>
  <div class="tabs" role="tablist">
    <button v-for="tab in tabs" :key="tab.id" type="button" role="tab" class="tab-btn"
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
      <svg v-else-if="tab.icon === 'download'" viewBox="0 0 24 24" width="17" height="17" fill="none"
        stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3v11" />
        <path d="m7 10 5 5 5-5" />
        <path d="M5 20h14" />
      </svg>
      <svg v-else viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18" />
      </svg>
      <span class="tab-label">{{ t(tab.labelKey) }}</span>
      <span v-if="tab.suffix" class="tab-suffix">{{ tab.suffix }}</span>
    </button>
  </div>
</template>

<style scoped>
.tabs {
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: 1fr;
  gap: 8px;
  margin-bottom: 14px;
}

.tab-btn {
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
.tab-label {
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 数量一类的小尾巴，激活态下也压得住 */
.tab-suffix {
  flex-shrink: 0;
  font-size: 11.5px;
  opacity: 0.75;
  font-variant-numeric: tabular-nums;
}

.tab-btn:hover:not(:disabled) {
  border-color: var(--accent-border);
  color: var(--text);
  transform: translateY(-1px);
}

.tab-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.tab-btn:disabled {
  cursor: default;
  opacity: 0.6;
}

.tab-btn.active {
  border-color: transparent;
  background: var(--accent-grad);
  color: #fff;
  font-weight: 600;
  box-shadow: 0 6px 16px var(--accent-soft);
}

/* 激活态不再上浮：避免与背景渐变一起「跳」一下 */
.tab-btn.active:hover {
  transform: none;
}
</style>
