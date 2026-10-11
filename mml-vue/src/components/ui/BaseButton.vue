<script setup lang="ts">
// 统一按钮，五种变体：primary 主操作、accent 强调、plain 次要、danger 危险、ghost 图标类
withDefaults(
  defineProps<{
    /** 外观变体；primary 主操作、accent 强调、plain 次要、danger 危险、ghost 图标类 */
    variant?: "primary" | "accent" | "plain" | "danger" | "ghost";
    /** 尺寸档；sm 定高 28px、md 35px，lg 由主操作按钮自行覆盖 */
    size?: "sm" | "md" | "lg";
    /** 是否禁用 */
    disabled?: boolean;
    /** 是否占满整行宽度 */
    block?: boolean;
  }>(),
  { variant: "plain", size: "md", disabled: false, block: false },
);
</script>

<template>
  <button class="ui-btn" :class="[`v-${variant}`, `s-${size}`, { block }]" :disabled="disabled">
    <slot />
  </button>
</template>

<style scoped>
.ui-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border: none;
  border-radius: 10px;
  font-family: inherit;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.ui-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  filter: none;
}

/* 尺寸：定高两档，28 = NumberStepper 档，35 = SegmentedTabs / 设置页 dl-grid 档；
   lg 留给主操作按钮自行覆盖 */
.s-sm {
  font-size: 12.5px;
  height: 28px;
  padding: 0 14px;
  border-radius: 8px;
}

.s-md {
  font-size: 14px;
  height: 35px;
  padding: 0 20px;
}

.s-lg {
  font-size: 15px;
  padding: 13px 30px;
}

/* 变体 */
.v-primary {
  background: var(--accent-grad);
  color: #fff;
  box-shadow: 0 4px 14px rgba(79, 140, 255, 0.22);
}

.v-primary:hover {
  filter: brightness(1.1);
}

.v-primary:active {
  transform: scale(0.98);
}

.v-plain {
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text-dim);
}

.v-plain:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.v-accent {
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent);
}

.v-accent:hover {
  background: var(--accent);
  color: #fff;
}

.v-danger {
  background: rgba(255, 95, 86, 0.14);
  color: var(--red);
  border: 1px solid rgba(255, 95, 86, 0.4);
}

.v-danger:hover {
  background: rgba(255, 95, 86, 0.24);
}

.v-ghost {
  background: transparent;
  color: var(--text-dim);
  border: 1px solid transparent;
}

.v-ghost:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.block {
  width: 100%;
}
</style>
