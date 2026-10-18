<script setup lang="ts">
// 设置窗口 · 左侧标签导航（图标 + 标题 + 一行说明）
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { t } from "../../../lib/i18n";
import type { SettingsTab, SettingsTabItem } from "../types";

defineProps<{
  /** 标签清单，顺序即左侧导航顺序 */
  tabs: SettingsTabItem[];
  /** 当前标签（模型值） */
  modelValue: SettingsTab;
}>();

const emit = defineEmits<{
  /** 切换标签，v-model 的回写 */
  (e: "update:modelValue", v: SettingsTab): void;
}>();
</script>

<template>
  <nav class="tab-rail" role="tablist" aria-orientation="vertical">
    <button v-for="item in tabs" :key="item.id" type="button" role="tab" class="tab-item"
      :class="{ active: modelValue === item.id }" :aria-selected="modelValue === item.id"
      @click="emit('update:modelValue', item.id)">
      <span class="tab-icon">
        <GlyphIcon :name="item.icon" :size="16" :weight="1.8" />
      </span>
      <span class="tab-text">
        <span class="tab-title">{{ t(`winSettings.tab.${item.id}`) }}</span>
        <span class="tab-desc">{{ t(`winSettings.tabDesc.${item.id}`) }}</span>
      </span>
    </button>
  </nav>
</template>

<style scoped>
.tab-rail {
  width: 176px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 10px 14px 16px;
  border-right: 1px solid var(--border);
  background: var(--bg-side);
}

.tab-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  text-align: left;
  transition: background 0.12s, color 0.12s;
}

.tab-item:hover {
  background: var(--bg-card);
  color: var(--text);
}

.tab-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.tab-item.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.tab-icon {
  flex-shrink: 0;
  display: flex;
}

.tab-text {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
  min-width: 0;
}

.tab-title {
  font-size: 13px;
  font-weight: 600;
}

.tab-desc {
  font-size: 10.5px;
  color: var(--text-dim);
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
