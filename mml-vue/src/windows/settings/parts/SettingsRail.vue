<script setup lang="ts">
// 设置窗口 · 左侧标签导航（图标 + 标题 + 一行说明）
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { t } from "../../../lib/i18n";
import type { SettingsTab, SettingsTabItem } from "../types";

const props = defineProps<{
  /** 标签清单，顺序即左侧导航顺序 */
  tabs: SettingsTabItem[];
  /** 当前标签（模型值） */
  modelValue: SettingsTab;
}>();

const emit = defineEmits<{
  /** 切换标签，v-model 的回写 */
  (e: "update:modelValue", v: SettingsTab): void;
}>();

/** 导航容器，用来量当前选中项的尺寸与位置 */
const rootEl = ref<HTMLElement | null>(null);
/** 选中项背后的色块，按真实按钮的位置与高度摆放 */
const thumbStyle = ref<Record<string, string>>({
  width: "0px",
  height: "0px",
  transform: "translate(0, 0)",
});
/** 首次量完再显示，避免从左上角滑到选中项那一下 */
const thumbReady = ref(false);
let resizeObserver: ResizeObserver | null = null;

/** 把色块移到当前选中的标签下面 */
function updateThumb() {
  const root = rootEl.value;
  if (!root) return;
  const index = props.tabs.findIndex((item) => item.id === props.modelValue);
  const button = root.querySelectorAll<HTMLElement>(".tab-item")[index];
  if (!button) {
    thumbReady.value = false;
    return;
  }
  thumbStyle.value = {
    width: `${button.offsetWidth}px`,
    height: `${button.offsetHeight}px`,
    transform: `translate(${button.offsetLeft}px, ${button.offsetTop}px)`,
  };
  if (!thumbReady.value) {
    // 先让 transition:none 的初始帧落进 DOM，下一帧再开启过渡
    requestAnimationFrame(() => {
      thumbReady.value = true;
    });
  }
}

watch(
  () => props.modelValue,
  () => {
    void nextTick(updateThumb);
  },
);
watch(
  () => props.tabs,
  () => {
    void nextTick(updateThumb);
  },
  { deep: true },
);

onMounted(() => {
  void nextTick(updateThumb);
  if (typeof ResizeObserver !== "undefined" && rootEl.value) {
    resizeObserver = new ResizeObserver(() => updateThumb());
    resizeObserver.observe(rootEl.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});
</script>

<template>
  <nav ref="rootEl" class="tab-rail" :class="{ ready: thumbReady }" role="tablist" aria-orientation="vertical">
    <span class="tab-thumb" :style="thumbStyle" aria-hidden="true"></span>
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
  position: relative;
  width: 176px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 10px 14px 16px;
  border-right: 1px solid var(--border);
  background: var(--bg-side);
}

/* 选中色块：切换标签时在导航里上下滑动 */
.tab-thumb {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 0;
  border-radius: 10px;
  background: var(--accent-soft);
  opacity: 0;
  pointer-events: none;
  transition: none;
}

.tab-rail.ready .tab-thumb {
  opacity: 1;
  transition:
    transform 0.18s var(--ease-out),
    width 0.18s var(--ease-out),
    height 0.18s var(--ease-out);
}

.tab-item {
  position: relative;
  z-index: 1;
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

.tab-item:hover:not(.active) {
  background: var(--bg-card);
  color: var(--text);
}

.tab-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.tab-item.active {
  background: transparent;
  color: var(--accent);
}

/* 第一次量完之前没有色块，给选中项一个背景兜底 */
.tab-rail:not(.ready) .tab-item.active {
  background: var(--accent-soft);
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
