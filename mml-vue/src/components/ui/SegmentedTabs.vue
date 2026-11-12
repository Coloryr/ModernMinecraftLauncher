<script setup lang="ts">
// 统一分段切换控件，配合 `v-model` 使用，值为 string；选项可带图标。
//
// 图标走 GlyphIcon（几何在 glyphs.ts），不再在这里维护一份 v-html 的 path 表 ——
// 同一枚 folder / grid / list 在别处也画，两套定义迟早会漂移。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import GlyphIcon from "./GlyphIcon.vue";
import { isGlyphName } from "./glyphs";

interface SegOption {
  /** 选中值，也就是 `v-model` 的值 */
  value: string;
  /** 显示文案，调用方已 `t()` 好的成品 */
  label: string;
  /** 图标名，取 glyphs.ts 的键；不传或认不出则只显示文案 */
  icon?: string;
}

const props = defineProps<{
  /** 可选项；`value` 为选中值，`icon` 是 glyphs.ts 里的图标名 */
  options: SegOption[];
  /** 当前选中值，配合 `v-model` 使用 */
  modelValue: string;
  /** 禁用整组；加载期间锁住筛选控件用 */
  disabled?: boolean;
  /** 只显示图标，文字只作为提示与读屏名 */
  iconOnly?: boolean;
}>();

const emit = defineEmits<{
  /** `v-model` 更新：新的选中值 */
  (e: "update:modelValue", value: string): void;
}>();

/** 选项 + 解析好的图标名；icon 认不出时为 null，模板据此不画图标 */
const items = computed(() =>
  props.options.map((opt) => ({
    ...opt,
    glyph: opt.icon && isGlyphName(opt.icon) ? opt.icon : null,
  })),
);

/** 轨道容器，用来量当前选中按钮的位置与宽度 */
const rootEl = ref<HTMLElement | null>(null);
/** 蓝条的内联位置；宽度与位移都按真实按钮算，文案长短不同也能对齐 */
const thumbStyle = ref<Record<string, string>>({ width: "0px", transform: "translateX(0px)" });
/** 首次量完再显示蓝条，避免从 0 宽滑到选中项那一下 */
const thumbReady = ref(false);
let resizeObserver: ResizeObserver | null = null;

/** 把蓝条移到当前选中项下面 */
function updateThumb() {
  const root = rootEl.value;
  if (!root) return;
  const index = props.options.findIndex((opt) => opt.value === props.modelValue);
  const button = root.querySelectorAll<HTMLElement>(".seg-btn")[index];
  if (!button) {
    thumbReady.value = false;
    return;
  }
  thumbStyle.value = {
    width: `${button.offsetWidth}px`,
    transform: `translateX(${button.offsetLeft}px)`,
  };
  if (!thumbReady.value) {
    // 先让带 transition:none 的初始帧落进 DOM，下一帧再开启过渡
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
  () => props.options,
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
  <div ref="rootEl" class="seg-tabs" :class="{ ready: thumbReady, disabled: props.disabled }">
    <span class="seg-thumb" :style="thumbStyle" aria-hidden="true"></span>
    <button v-for="opt in items" :key="opt.value" class="seg-btn"
      :class="{ active: modelValue === opt.value, 'icon-only': props.iconOnly }" :disabled="props.disabled"
      :aria-label="opt.label" v-tip="opt.label" @click="emit('update:modelValue', opt.value)">
      <GlyphIcon v-if="opt.glyph" class="seg-icon" :name="opt.glyph ?? 'globe'" :size="14" :weight="2" />
      <span v-if="!props.iconOnly">{{ opt.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.seg-tabs {
  /* inline-flex：轨道背景只包住选项内容，不撑满父容器宽度 */
  position: relative;
  display: inline-flex;
  gap: 4px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: calc(var(--ctl-radius) + 2px);
  padding: 2px;
}

/* 蓝色选中条：位置和宽度由脚本按当前按钮量出来，切换时左右滑过去 */
.seg-thumb {
  position: absolute;
  top: 2px;
  left: 0;
  z-index: 0;
  height: calc(var(--ctl-h) - 6px);
  border-radius: var(--ctl-radius);
  background: var(--accent);
  opacity: 0;
  pointer-events: none;
  transition: none;
}

.seg-tabs.ready .seg-thumb {
  opacity: 1;
  transition:
    transform 0.18s var(--ease-out),
    width 0.18s var(--ease-out);
}

.seg-tabs.disabled .seg-thumb {
  opacity: 0.55;
}

.seg-btn {
  position: relative;
  z-index: 1;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  color: var(--text-dim);
  font-size: var(--ctl-font);
  /* 内胆 = 控件高 − 上下 padding 2×2 − 边框 1×2，整轨就是 --ctl-h */
  height: calc(var(--ctl-h) - 6px);
  padding: 0 12px;
  border-radius: var(--ctl-radius);
  cursor: pointer;
  transition: color 0.12s, background 0.12s;
  font-family: inherit;
  white-space: nowrap;
}

.seg-btn:hover {
  color: var(--text);
}

.seg-btn.active {
  color: var(--on-accent);
  font-weight: 600;
}

/* 第一次量完之前没有蓝条，给选中项一个背景兜底 */
.seg-tabs:not(.ready) .seg-btn.active {
  background: var(--accent);
}

.seg-icon {
  flex-shrink: 0;
}

.seg-btn.icon-only {
  justify-content: center;
  width: calc(var(--ctl-h) - 6px);
  padding: 0;
  gap: 0;
}

.seg-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
</style>
