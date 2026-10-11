<script setup lang="ts">
// 设置窗口 · 左侧标签导航（图标 + 标题 + 一行说明）
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
        <!-- 调色板：界面 -->
        <svg v-if="item.icon === 'palette'" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor"
          stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="13.5" cy="6.5" r=".5" />
          <circle cx="17.5" cy="10.5" r=".5" />
          <circle cx="8.5" cy="7.5" r=".5" />
          <circle cx="6.5" cy="12.5" r=".5" />
          <path
            d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z" />
        </svg>
        <!-- 人像：皮肤与头像 -->
        <svg v-else-if="item.icon === 'user'" viewBox="0 0 24 24" width="16" height="16" fill="none"
          stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
          <circle cx="12" cy="7" r="4" />
        </svg>
        <!-- 下载箭头：网络与下载 -->
        <svg v-else-if="item.icon === 'download'" viewBox="0 0 24 24" width="16" height="16" fill="none"
          stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <path d="m7 10 5 5 5-5" />
          <path d="M12 15V3" />
        </svg>
        <!-- 播放：游戏启动 -->
        <svg v-else-if="item.icon === 'play'" viewBox="0 0 24 24" width="16" height="16" fill="none"
          stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="m6 4 14 8-14 8V4z" />
        </svg>
        <!-- 咖啡杯：Java -->
        <svg v-else-if="item.icon === 'coffee'" viewBox="0 0 24 24" width="16" height="16" fill="none"
          stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M17 8h1a4 4 0 1 1 0 8h-1" />
          <path d="M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4Z" />
          <line x1="6" x2="6" y1="2" y2="4" />
          <line x1="10" x2="10" y1="2" y2="4" />
          <line x1="14" x2="14" y1="2" y2="4" />
        </svg>
        <!-- 齿轮：客户端设置 -->
        <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <path
            d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
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
