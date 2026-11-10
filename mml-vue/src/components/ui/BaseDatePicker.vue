<script setup lang="ts">
// 日期选择器：点击输入框弹出**自绘**的月历，取代表单里的原生 <input type="date">。
// 原生的日历弹层是浏览器画的，除了 color-scheme 之外没有可定制的伪元素，
// 与界面主题对不上；这里自己画一份，外观全走主题变量。
//
// 值统一是 YYYY-MM-DD 字符串，与后端 DTO 同口径；不做时区换算。
// 月历弹层拆在 DatePickerPopover：这里只管触发按钮、开关状态与回写。
import { computed, ref } from "vue";
import { t } from "../../lib/i18n";
import DatePickerPopover from "./DatePickerPopover.vue";

const props = withDefaults(
  defineProps<{
    /** 当前日期，YYYY-MM-DD；空串表示未选 */
    modelValue: string;
    /** 可选的最早日期，YYYY-MM-DD；不传表示不限 */
    min?: string;
    /** 可选的最晚日期，YYYY-MM-DD；不传表示不限 */
    max?: string;
    /** 无障碍名称，也是悬停提示 */
    ariaLabel?: string;
    /** 未选日期时输入框里的提示；不给就用通用的「选择日期」 */
    placeholder?: string;
  }>(),
  { min: "", max: "", ariaLabel: "", placeholder: "" },
);

const emit = defineEmits<{
  /** v-model 更新：选中的日期，YYYY-MM-DD */
  (e: "update:modelValue", v: string): void;
}>();

/** 弹层是否展开 */
const open = ref(false);

/** 根元素：既是触发按钮的定位参照，也传给弹层排除"点自己" */
const rootEl = ref<HTMLElement | null>(null);

/** 输入框里显示的文本：选了就是日期，没选用调用方给的提示，最后才回落通用提示 */
const label = computed(() => props.modelValue || props.placeholder || t("date.empty"));

/** 展开 / 收起；弹层自己负责定位与收起 */
function toggle() {
  open.value = !open.value;
}

/**
 * 选中某天
 *
 * - `date`: YYYY-MM-DD
 */
function onPick(date: string) {
  emit("update:modelValue", date);
  open.value = false;
}
</script>

<template>
  <div ref="rootEl" class="dp">
    <button type="button" class="field-input dp-trigger" :class="{ 'is-open': open }"
      :aria-label="ariaLabel || undefined" :title="ariaLabel || undefined" @click="toggle">
      <span class="dp-text" :class="{ 'is-empty': !modelValue }">{{ label }}</span>
      <span class="dp-icon" aria-hidden="true"></span>
    </button>

    <!-- 弹层挂到 body：父级是滚动容器，absolute 会被 overflow 裁掉（窗口右边、底边都能切） -->
    <Teleport to="body">
      <Transition name="drop">
        <DatePickerPopover v-if="open" :model-value="modelValue" :min="min" :max="max" :anchor="rootEl"
          @pick="onPick" @close="open = false" />
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.dp {
  position: relative;
  min-width: 0;
}

/* 触发按钮借用 .field-input 的外观；按钮的内容对齐与省略要自己补 */
.dp-trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  text-align: left;
  cursor: pointer;
}

/* 展开态与聚焦态同一个观感，别让用户以为没打开 */
.dp-trigger.is-open {
  border-color: var(--accent);
}

.dp-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.dp-text.is-empty {
  color: var(--text-dim);
}

/* 日历图标与 .field-select 的箭头同尺寸同位置，图来自主题变量 */
.dp-icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
  background: var(--date-icon) center / 12px 12px no-repeat;
  opacity: 0.85;
  transition: opacity 0.12s;
}

.dp-trigger:hover .dp-icon {
  opacity: 1;
}
</style>
