<script setup lang="ts">
// 单窗口模式下的日志浮层：**可以同时开多个**，每个锁定一个实例
//
// 为什么不用 BaseModal：它是模态弹窗（整屏遮罩 + 居中），叠多个会一层层压暗、面板还完全
// 重叠。日志是"边跑边看"的工具窗，这里改成无遮罩的层叠卡片：页面照常可见可点，卡片按打开
// 顺序向右下错开一档，点哪张哪张提到最前（顺序由父组件维护，见 windowManager 的 logPopups）。
//
// 多窗口模式不走这里：那边每个实例一个真实窗口，见后端 windows/registry 的 log_window_id。
import LogWindow from "./LogWindow.vue";

const props = defineProps<{
  /** 已打开的日志窗口，按打开顺序；最后一项在最上层 */
  uuids: string[];
}>();

const emit = defineEmits<{
  /** 关掉某个实例的日志窗口 */
  (e: "close", uuid: string): void;
  /** 把某个实例的日志窗口提到最前 */
  (e: "raise", uuid: string): void;
}>();

/**
 * 按下某张卡片：把它提到最前
 *
 * 已经在最上面的一张不重排 —— 重排会移动 DOM 节点，正在拖选日志文字的那一手会断掉。
 *
 * - `uuid`: 实例 uuid
 * - `index`: 在打开顺序里的下标
 */
function onSlotDown(uuid: string, index: number) {
  if (index === props.uuids.length - 1) return;
  emit("raise", uuid);
}

/** 层叠步长，单位 px；错开太多会跑到屏幕外，所以最多错开四档 */
const STEP_X = 28;
const STEP_Y = 24;
const MAX_STEP = 4;

/**
 * 第 `index` 张卡片的错位与层级
 *
 * - `index`: 在打开顺序里的下标，0 是最早打开的
 *
 * # 返回值
 *
 * 内联样式，含位移与 z-index
 */
function slotStyle(index: number) {
  const step = Math.min(index, MAX_STEP);
  return {
    transform: `translate(${step * STEP_X}px, ${step * STEP_Y}px)`,
    zIndex: index + 1,
  };
}
</script>

<template>
  <Teleport to="body">
    <div class="log-popups">
      <div v-for="(uuid, index) in uuids" :key="uuid" class="popup-slot" :style="slotStyle(index)"
        @pointerdown="onSlotDown(uuid, index)">
        <LogWindow :uuid="uuid" @close="emit('close', uuid)" />
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
/* 浮层本身不吃事件，只有卡片吃：否则整块透明区域会把页面挡死 */
.log-popups {
  position: fixed;
  top: var(--titlebar-h);
  left: 0;
  right: 0;
  bottom: 0;
  /* 在页面内容与多选工具条（90）之上，在弹窗（100）与右键菜单（120）之下 */
  z-index: 95;
  pointer-events: none;
}

.popup-slot {
  position: absolute;
  top: 22px;
  left: 36px;
  pointer-events: auto;
}
</style>
