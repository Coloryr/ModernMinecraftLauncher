<script setup lang="ts" generic="T extends string">
// 通用图标选项卡：图标 + 渐变激活态，按数量等分填满整行
//
// 原「添加实例」窗口的 ModeTabs，导出窗口要用同一套观感，所以挪到这里共用。
// 图标统一走 GlyphIcon：配置里只写名字，认不出的回落到地球，不再在这里各画一份 path。
import { t } from "../../lib/i18n";
import GlyphIcon from "./GlyphIcon.vue";
import { isGlyphName, type GlyphName } from "./glyphs";

defineProps<{
  /** 页签清单，顺序即显示顺序 */
  tabs: Array<{
    /** 页签 id，也就是 `v-model` 的值 */
    id: T;
    /** 文案的 i18n key */
    labelKey: string;
    /** 图标名，见 glyphs.ts；认不出时画地球兜底 */
    icon: string;
    /** 跟在标签后的数量一类小尾巴；不传则不画 */
    suffix?: string;
  }>;
  /** 当前页签 */
  modelValue: T;
  /** 禁止切换，比如表单正在提交 */
  disabled?: boolean;
}>();

const emit = defineEmits<{
  /** 切换页签 */
  (e: "update:modelValue", v: T): void;
}>();

/**
 * 把页签配置里的图标名收窄成 GlyphName
 *
 * - `icon`: 配置里的字符串
 *
 * # 返回值
 *
 * 已注册的图标名；认不出时回落成 `globe`
 */
function glyphOf(icon: string): GlyphName {
  return isGlyphName(icon) ? icon : "globe";
}
</script>

<template>
  <div class="tabs" role="tablist">
    <button v-for="tab in tabs" :key="tab.id" type="button" role="tab" class="tab-btn"
      :class="{ active: modelValue === tab.id }" :aria-selected="modelValue === tab.id" :disabled="disabled"
      @click="emit('update:modelValue', tab.id)">
      <GlyphIcon :name="glyphOf(tab.icon)" :size="17" :weight="1.8" />
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
  color: var(--on-accent);
  font-weight: 600;
  box-shadow: 0 6px 16px var(--accent-soft);
}

/* 激活态不再上浮：避免与背景渐变一起「跳」一下 */
.tab-btn.active:hover {
  transform: none;
}
</style>
