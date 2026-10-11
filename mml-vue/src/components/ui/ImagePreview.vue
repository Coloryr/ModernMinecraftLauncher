<script setup lang="ts">
// 图片放大预览：全屏浮层，滚轮缩放 1~8 倍、放大后拖动平移、点空白处关闭。
//
// 从「下载整合包」的项目详情搬过来的 —— 那边原本写死在页面里，
// 资源窗口的截图预览也要同一套观感，所以抽成组件，两边共用一份。
// 两处各写一遍迟早会漂移成"一个能拖一个不能"。
//
// **Esc 不在这里处理**：上层可能还有别的 Esc 语义 —— 项目详情页是
// "先关预览、再关详情页"，两个 window 级 keydown 监听同时挂着时谁先跑取决于挂载顺序，
// 很容易变成"一次 Esc 把详情页也关了"。所以 Esc 留给调用方。
import { onUnmounted, ref, watch } from "vue";
import { t } from "../../lib/i18n";

const props = defineProps<{
  /** 图片地址；空串 = 不显示，调用方也可以直接用 v-if */
  src: string;
}>();

const emit = defineEmits<{
  /** 请求关闭浮层；拖动结束那一下不算 */
  (e: "close"): void;
}>();

/** 缩放倍率（1 = 适配大小） */
const scale = ref(1);
/** 平移偏移，单位 px；放大后拖动看局部 */
const offset = ref({ x: 0, y: 0 });

/** 本次按下是否真的拖动过：拖动结束那一下的 click 不该关闭预览 */
let dragged = false;
let from = { x: 0, y: 0, ox: 0, oy: 0 };

function reset() {
  scale.value = 1;
  offset.value = { x: 0, y: 0 };
}

// 换一张图就回到适配大小，别把上一张的缩放 / 偏移带过来
watch(() => props.src, reset);

/** 按下开始拖动；未放大时没有可平移的余量，直接返回 */
function onDown(e: PointerEvent) {
  if (scale.value <= 1) return;
  dragged = false;
  from = { x: e.clientX, y: e.clientY, ox: offset.value.x, oy: offset.value.y };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
}

function onMove(e: PointerEvent) {
  const dx = e.clientX - from.x;
  const dy = e.clientY - from.y;
  // 3px 阈值：手抖不算拖动
  if (Math.abs(dx) > 3 || Math.abs(dy) > 3) dragged = true;
  offset.value = { x: from.ox + dx, y: from.oy + dy };
}

function onUp() {
  window.removeEventListener("pointermove", onMove);
  window.removeEventListener("pointerup", onUp);
}

// 拖动过程中被关掉（点空白 / Esc）时也要摘掉 window 监听，否则会一直挂着
onUnmounted(onUp);

/** 点空白处关闭；刚拖过的那一下不算点击 */
function onClick() {
  if (dragged) {
    dragged = false;
    return;
  }
  emit("close");
}

/** 滚轮缩放；向上放大、向下缩小，1~8 倍 */
function onWheel(e: WheelEvent) {
  const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
  const next = Math.min(8, Math.max(1, scale.value * factor));
  scale.value = next;
  // 缩回适配大小：偏移一并归零，否则图会停在屏幕外看不见
  if (next === 1) offset.value = { x: 0, y: 0 };
}
</script>

<template>
  <Teleport to="body">
    <transition name="img-preview-fade">
      <div v-if="src" class="img-preview" :class="{ pannable: scale > 1, panning: dragged }" @click="onClick"
        @wheel.prevent="onWheel" @pointerdown="onDown">
        <img :src="src" alt="" :style="{ transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})` }" />
        <!--
          底部条：提示 + 调用方自己的操作，截图那边放"打开文件夹 / 删除"。
          `@click.stop` 是必须的 —— 不然点按钮会顺带触发浮层的"点空白关闭"
        -->
        <div class="img-preview-bar" @click.stop @pointerdown.stop>
          <span class="img-preview-hint">{{ t("common.imagePreviewHint") }}</span>
          <slot />
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.img-preview {
  position: fixed;
  /* 不盖住标题栏 */
  inset: var(--titlebar-h) 0 0 0;
  z-index: 200;
  background: rgb(0 0 0 / 85%);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: zoom-out;
  /* 放大后溢出部分裁掉 */
  overflow: hidden;
  user-select: none;
  touch-action: none;
}

/* 放大后可拖动看局部：光标换成抓手 */
.img-preview.pannable {
  cursor: grab;
}

.img-preview.panning {
  cursor: grabbing;
}

.img-preview img {
  max-width: 92%;
  max-height: 92%;
  object-fit: contain;
  box-shadow: var(--shadow-lg);
  /* 拖动时不要触发系统拖图；transform 交给 GPU */
  -webkit-user-drag: none;
  will-change: transform;
}

/* 底部：提示 + 可选操作，整条居中 */
.img-preview-bar {
  position: absolute;
  bottom: 14px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
}

.img-preview-hint {
  padding: 4px 12px;
  border-radius: 999px;
  background: rgb(0 0 0 / 55%);
  color: rgb(255 255 255 / 85%);
  font-size: 12px;
  white-space: nowrap;
  pointer-events: none;
}

.img-preview-fade-enter-active,
.img-preview-fade-leave-active {
  transition: opacity 0.15s ease;
}

.img-preview-fade-enter-from,
.img-preview-fade-leave-to {
  opacity: 0;
}
</style>
