<script setup lang="ts">
// 统一分段切换控件，配合 `v-model` 使用，值为 string；选项可带图标。
//
// 图标走 GlyphIcon（几何在 glyphs.ts），不再在这里维护一份 v-html 的 path 表 ——
// 同一枚 folder / grid / list 在别处也画，两套定义迟早会漂移。
import { computed } from "vue";
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
</script>

<template>
  <div class="seg-tabs">
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
  display: inline-flex;
  gap: 4px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: calc(var(--ctl-radius) + 2px);
  padding: 2px;
}

.seg-btn {
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
  transition: all 0.15s;
  font-family: inherit;
  white-space: nowrap;
}

.seg-btn:hover {
  color: var(--text);
}

.seg-btn.active {
  background: var(--accent);
  color: var(--on-accent);
  font-weight: 600;
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
