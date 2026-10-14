<script setup lang="ts">
// 数字输入：[- 输入框 +]，可手动输入数字，也可用左右按钮
import { ref } from "vue";
import GlyphIcon from "./GlyphIcon.vue";

const props = withDefaults(
  defineProps<{
    /** 当前值，配合 `v-model` 使用 */
    modelValue: number;
    /** 允许的最小值 */
    min?: number;
    /** 允许的最大值 */
    max?: number;
    /** 加减按钮的步长 */
    step?: number;
  }>(),
  { min: 0, max: 99999, step: 1 },
);

const emit = defineEmits<{
  /** `v-model` 更新：夹在 `min`~`max` 之间的新值 */
  (e: "update:modelValue", v: number): void;
}>();

const editing = ref(false);
const draft = ref(String(props.modelValue));

function clamp(v: number) {
  return Math.min(props.max, Math.max(props.min, v));
}

function commit(v: number) {
  emit("update:modelValue", clamp(v));
}

function dec() {
  commit(props.modelValue - props.step);
}

function inc() {
  commit(props.modelValue + props.step);
}

function onFocus() {
  editing.value = true;
  draft.value = String(props.modelValue);
}

function onInput(e: Event) {
  draft.value = (e.target as HTMLInputElement).value;
}

function onCommit() {
  editing.value = false;
  const n = Number(draft.value);
  if (!Number.isNaN(n)) commit(n);
}
</script>

<template>
  <div class="stepper">
    <button class="step-btn" @click="dec">
      <GlyphIcon name="minus" :size="14" />
    </button>
    <input class="step-input" type="number" :min="min" :max="max" :step="step" :value="editing ? draft : modelValue"
      @focus="onFocus" @input="onInput" @change="onCommit" @keyup.enter="($event.target as HTMLInputElement).blur()"
      @blur="onCommit" />
    <button class="step-btn" @click="inc">
      <GlyphIcon name="plus" :size="14" />
    </button>
  </div>
</template>

<style scoped>
/* 一体式数字步进器：[- 输入框 +] 共用一个圆角边框盒。
   底色用 --bg-raised，比卡片明显亮一档的专用色阶，见 themes.css */
.stepper {
  display: inline-flex;
  align-items: stretch;
  height: var(--ctl-h);
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: var(--bg-raised);
  overflow: hidden;
}

.step-btn {
  width: var(--ctl-h);
  border: none;
  background: transparent;
  color: var(--text-dim);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  font-family: inherit;
  padding: 0;
  flex-shrink: 0;
}

.step-btn:first-child {
  border-right: 1px solid var(--border);
}

.step-btn:last-child {
  border-left: 1px solid var(--border);
}

.step-btn:hover {
  background: var(--accent-soft);
  color: var(--accent);
}

.step-btn:active {
  background: var(--accent-soft);
}

.step-input {
  width: 64px;
  border: none;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  text-align: center;
  outline: none;
  font-variant-numeric: tabular-nums;
  font-family: inherit;
  -moz-appearance: textfield;
}

.step-input::-webkit-outer-spin-button,
.step-input::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

/* 输入框无边框，聚焦反馈落在整体外框上 */
.stepper:focus-within {
  border-color: var(--accent);
}
</style>
