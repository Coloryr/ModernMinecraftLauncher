<script setup lang="ts">
// 搜索框：放大镜 + 输入 + 清空键，标题栏与工具条共用这一份。
//
// 外观**整块交给 .field-input**（自己只加左右两段留白给图标和清空键），
// 所以不透明底 / 主题描边 / 圆角 / 高度 / 字号 / 悬停聚焦反馈永远与全 app 的输入框一致 ——
// 之前账户窗口与设置窗口各抄了一份标题栏搜索，底色一个 --bg-card 一个 --bg-solid、
// 字号一个 13 一个 12.5，就是这么走样的。
// 结果下拉由调用方通过 `drop` 具名插槽给（设置窗口用，账户窗口不用），自己只提供定位参照。
import { computed } from "vue";
import GlyphIcon from "./GlyphIcon.vue";

const props = withDefaults(
  defineProps<{
    /** 输入内容，配合 v-model 使用 */
    modelValue: string;
    /** 占位文案，调用方已 `t()` 好的成品 */
    placeholder?: string;
    /** 整块宽度，CSS 长度；标题栏里各有各的富余空间，所以由调用方给 */
    width?: string;
    /** 清空键的无障碍名称；不给就退回占位文案 */
    clearLabel?: string;
  }>(),
  {
    placeholder: "",
    width: "168px",
    clearLabel: "",
  },
);

const emit = defineEmits<{
  /** v-model 回写 */
  (e: "update:modelValue", v: string): void;
  /** 输入框获得焦点 */
  (e: "focus"): void;
  /** 输入框失去焦点 */
  (e: "blur"): void;
  /** 输入框按键；下拉的 ↑ / ↓ / Enter / Esc 由调用方处理 */
  (e: "keydown", ev: KeyboardEvent): void;
}>();

const hasText = computed(() => props.modelValue.length > 0);

/** 清空键：回写空串 */
function clear() {
  emit("update:modelValue", "");
}
</script>

<template>
  <div class="search-box" :style="{ width }">
    <GlyphIcon class="sb-icon" name="search" :size="13" :weight="2" />
    <input :value="modelValue" class="field-input sb-input" :placeholder="placeholder" spellcheck="false"
      autocomplete="off" @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      @focus="emit('focus')" @blur="emit('blur')" @keydown="emit('keydown', $event)" />
    <!-- 清空键与输入框内的其它清空键同一枚：20px 方形热区，见 styles/controls.css；
         mousedown 已 prevent，点它不会先触发 blur（设置窗口的下拉靠 blur 收起） -->
    <button v-if="hasText" class="input-clear sb-clear" :aria-label="clearLabel || placeholder" @mousedown.prevent
      @click="clear">
      <GlyphIcon name="close" :size="12" :weight="2.4" />
    </button>
    <slot name="drop" />
  </div>
</template>

<style scoped>
.search-box {
  position: relative;
  flex: 0 0 auto;
}

/* 图标与清空键都压在输入框内部：绝对定位，不参与 flex 行 —— 输入框因此仍是**整块**
   .field-input（宽 100%、自己的内边距），而不是被拆成"图标 | 框 | 键"三段 */
.sb-icon {
  position: absolute;
  left: 9px;
  top: 50%;
  transform: translateY(-50%);
  /* 必须穿透：图标是输入框的兄弟节点，不设这条时点到图标不会把焦点给输入框 */
  pointer-events: none;
  color: var(--text-dim);
}

/* 左右留白量出图标与清空键的位置；其余外观全走 .field-input */
.sb-input {
  padding-left: 29px;
  padding-right: 28px;
}

.sb-clear {
  position: absolute;
  top: 50%;
  right: 6px;
  transform: translateY(-50%);
}
</style>
