<script setup lang="ts">
// 设置窗口 · 保存状态文字，配合 composables/useSaveState
//
// **不显示"已保存"**：设置里大部分改动是即时落盘的，成功本来就是常态，
// 每改一处就冒一句"已保存"只是噪音。只在需要用户注意时才出声：
// 保存中 / 保存失败 / 有未保存的改动（后者见 `pending`）。
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import type { SaveState } from "../composables/useSaveState";

const props = defineProps<{
  /** 当前保存状态，来自 useSaveState */
  state: SaveState;
  /** 显式保存按钮的场景：idle 时显示"有未保存的改动"而不是空白 */
  pending?: boolean;
}>();

const text = computed(() => {
  switch (props.state) {
    case "saving":
      return t("winSettings.stateSaving");
    case "error":
      return t("winSettings.stateFailed");
    case "saved":
      // 成功后与 idle 一样安静
      return "";
    default:
      return props.pending ? t("winSettings.statePending") : "";
  }
});

/** 样式类：状态名保留给主题覆盖，具体颜色用工具类 */
const kind = computed(() => {
  if (props.state === "idle" && props.pending) return "pending";
  if (props.state === "saved") return "ok";
  if (props.state === "error") return "err";
  return props.state;
});

/** 状态对应的文字色；未命中时沿用 .save-state 的默认弱化色 */
const kindColor = computed(() => {
  switch (kind.value) {
    case "ok":
      return "text-green";
    case "err":
      return "text-red";
    case "pending":
      return "text-yellow";
    default:
      return "";
  }
});
</script>

<template>
  <span v-if="text"
    class="save-state inline-flex items-center gap-[5px] text-meta text-dim mr-1 whitespace-nowrap"
    :class="[kind, kindColor]" role="status" aria-live="polite">
    {{ text }}
  </span>
</template>
