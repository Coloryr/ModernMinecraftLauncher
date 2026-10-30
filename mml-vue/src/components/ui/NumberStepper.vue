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
    /** 禁用：输入框与加减按钮都不可用。跟随全局设置这类"值只作展示"的场合用 */
    disabled?: boolean;
  }>(),
  { min: 0, max: 99999, step: 1, disabled: false },
);

const emit = defineEmits<{
  /** `v-model` 更新：夹在 `min`~`max` 之间的新值 */
  (e: "update:modelValue", v: number): void;
}>();

const editing = ref(false);
const draft = ref(String(props.modelValue));

/** 把值夹进 min~max，越界取边界 */
function clamp(v: number) {
  return Math.min(props.max, Math.max(props.min, v));
}

/** 夹好再回写 v-model */
function commit(v: number) {
  emit("update:modelValue", clamp(v));
}

function dec() {
  commit(props.modelValue - props.step);
}

function inc() {
  commit(props.modelValue + props.step);
}

/** 进入编辑态，草稿从当前值起 */
function onFocus() {
  editing.value = true;
  draft.value = String(props.modelValue);
}

/** 打字只更新草稿，等失焦或回车再提交 */
function onInput(e: Event) {
  draft.value = (e.target as HTMLInputElement).value;
}

/** 失焦 / 回车：草稿是非空数字就提交，否则保持原值（清空不能变成 0） */
function onCommit() {
  editing.value = false;
  const text = draft.value.trim();
  if (!text) return;
  const n = Number(text);
  if (Number.isFinite(n)) commit(n);
}
</script>

<template>
  <div class="stepper" :class="{ 'is-disabled': disabled }">
    <button class="step-btn" :disabled="disabled" @click="dec">
      <GlyphIcon name="minus" :size="14" />
    </button>
    <input class="step-input" type="number" :min="min" :max="max" :step="step" :disabled="disabled"
      :value="editing ? draft : modelValue" @focus="onFocus" @input="onInput" @change="onCommit"
      @keyup.enter="($event.target as HTMLInputElement).blur()" @blur="onCommit" />
    <button class="step-btn" :disabled="disabled" @click="inc">
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

.step-btn:hover:not(:disabled) {
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

/* 禁用态：整块压暗，鼠标变成禁止符。值仍然看得见 —— 跟随全局时那正是会用的数字 */
.stepper.is-disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.stepper.is-disabled .step-btn,
.stepper.is-disabled .step-input {
  cursor: not-allowed;
}
</style>
