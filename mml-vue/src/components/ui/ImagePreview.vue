<script setup lang="ts">
// 图片放大预览：全屏浮层，滚轮缩放 1~8 倍、放大后拖动平移、点击关闭。
//
// 展开与收起走 FLIP：图从被点的那张缩略图的位置、尺寸出发，飞到浮层中央，期间底衬逐渐变暗；
// 关掉时反向飞回缩略图。调用方把缩略图的视口矩形当 `origin` 传进来，不传就退回"整张图淡入放大"。
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
import { animations } from "../../lib/appearance";
import type { PreviewOrigin } from "../../lib/previewOrigin";

const props = defineProps<{
  /** 图片地址；空串 = 不显示，调用方也可以直接用 v-if */
  src: string;
  /** 展开起点（缩略图的视口矩形）；缺省时整张图淡入放大，见 [`PreviewOrigin`] */
  origin?: PreviewOrigin | null;
}>();

const emit = defineEmits<{
  /** 请求关闭浮层；拖动结束那一下不算 */
  (e: "close"): void;
}>();

/** 展开 / 收起共用的缓动，与 --ease-out 是同一条曲线 */
const EASE_OUT = "cubic-bezier(0.22, 1, 0.36, 1)";
/** 往回缩用先慢后快再收尾的曲线，读起来是"飞回去"而不是"再弹一下" */
const EASE_IN_OUT = "cubic-bezier(0.65, 0, 0.35, 1)";

/** 缩放倍率（1 = 适配大小） */
const scale = ref(1);
/** 平移偏移，单位 px；放大后拖动看局部 */
const offset = ref({ x: 0, y: 0 });

/** 本次按下是否真的拖动过：拖动结束那一下的 click 不该关闭预览 */
let dragged = false;
let from = { x: 0, y: 0, ox: 0, oy: 0 };

/** 回到适配大小并清零平移 */
function reset() {
  scale.value = 1;
  offset.value = { x: 0, y: 0 };
}

// 换一张图就回到适配大小，别把上一张的缩放 / 偏移带过来。
// 收起时空串不算换图：归位动画要靠缩放状态判断"能不能飞回缩略图"
watch(() => props.src, (src) => {
  if (src) reset();
});

/** 按下开始拖动；未放大时没有可平移的余量，直接返回 */
function onDown(e: PointerEvent) {
  if (scale.value <= 1) return;
  dragged = false;
  from = { x: e.clientX, y: e.clientY, ox: offset.value.x, oy: offset.value.y };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  // 指针被系统收走时 pointerup 不会来，这里也要收尾
  window.addEventListener("pointercancel", onUp);
}

/** 跟随指针平移；位移超过 3px 就算真拖动过 */
function onMove(e: PointerEvent) {
  const dx = e.clientX - from.x;
  const dy = e.clientY - from.y;
  // 3px 阈值：手抖不算拖动
  if (Math.abs(dx) > 3 || Math.abs(dy) > 3) dragged = true;
  offset.value = { x: from.ox + dx, y: from.oy + dy };
}

/** 结束拖动，摘掉窗口监听 */
function onUp() {
  window.removeEventListener("pointermove", onMove);
  window.removeEventListener("pointerup", onUp);
  window.removeEventListener("pointercancel", onUp);
}

// 拖动过程中浮层被关掉时也要摘掉 window 监听，否则会一直挂着
onUnmounted(onUp);

/** 点击关闭；底部操作条自己拦了点击，刚拖过的那一下也不算 */
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

/**
 * 能不能做动画
 *
 * 关掉界面动画、环境没有 Web Animations、或窗口没焦点（WebKit 会暂停动画）时都不做，
 * 直接完成切换 —— 否则内容会停在动画起点，等兜底超时才出现。
 */
function animatable(): boolean {
  return animations.value && typeof Element.prototype.animate === "function" && document.hasFocus();
}

/**
 * 图在浮层里的**未缩放**盒子
 *
 * 放大后 `getBoundingClientRect()` 给的是缩放后的框，拿它算归位会从放大的位置往回缩。
 * 图是浮层里唯一在流的子元素、又是居中摆放的，所以用 offset 尺寸加浮层中心推回来。
 *
 * - `el`: 浮层根节点
 * - `img`: 大图元素
 *
 * # 返回值
 *
 * 视口坐标下的矩形，单位 px
 */
function layoutRect(el: Element, img: HTMLImageElement) {
  const host = el.getBoundingClientRect();
  const width = img.offsetWidth;
  const height = img.offsetHeight;
  return {
    x: host.x + (host.width - width) / 2,
    y: host.y + (host.height - height) / 2,
    width,
    height,
  };
}

/**
 * 从缩略图飞到浮层中央的位移与缩放
 *
 * - `origin`: 缩略图的视口矩形
 * - `target`: 图在浮层里的盒子，见 [`layoutRect`]
 *
 * # 返回值
 *
 * 起点的 transform（左上角对齐的位移 + 非等比缩放），目标端恒为 none
 */
function flyTransform(origin: PreviewOrigin, target: { x: number; y: number; width: number; height: number }) {
  const dx = origin.x - target.x;
  const dy = origin.y - target.y;
  return `translate(${dx}px, ${dy}px) scale(${origin.width / target.width}, ${origin.height / target.height})`;
}

/**
 * 进场：底衬变暗，图从缩略图的位置与尺寸缩放到浮层中央
 *
 * - `el`: 浮层根节点
 * - `done`: Vue 的完成回调
 */
function onEnter(el: Element, done: () => void) {
  if (!animatable()) {
    done();
    return;
  }
  const veil = el.querySelector(".img-preview-veil");
  const img = el.querySelector<HTMLImageElement>(".img-preview img");
  const made: Animation[] = [];
  let settled = false;
  const settle = () => {
    if (settled) return;
    settled = true;
    made.forEach((animation) => animation.cancel());
    if (img) img.style.transformOrigin = "";
    done();
  };
  // 窗口失焦时 WebKit 会暂停 Web Animations，finished 永远不来：超时兜底
  const watchdog = window.setTimeout(settle, 1200);
  let left = 0;
  const finish = () => {
    left -= 1;
    if (left <= 0) {
      window.clearTimeout(watchdog);
      settle();
    }
  };

  // 底衬单独一层：只有它在变暗，飞过来的图不跟着淡
  if (veil) {
    made.push(veil.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 200, easing: EASE_OUT, fill: "both" }));
  }
  made.push(...barFade(el, 0, 1, 160, 100));

  const origin = props.origin;
  const target = img ? layoutRect(el, img) : null;
  if (img && origin && target && target.width > 1 && target.height > 1) {
    // FLIP 的位移与缩放都按左上角对齐，动画结束就还原（缩放 / 平移仍以中心为原点）
    img.style.transformOrigin = "top left";
    made.push(img.animate([
      { transform: flyTransform(origin, target) },
      { transform: "none" },
    ], { duration: 240, easing: EASE_OUT, fill: "both" }));
  } else if (img) {
    // 量不出盒子（图还没解码完）或没有起点：退回整张图淡入放大
    made.push(img.animate([{ opacity: 0, scale: 0.92 }, { opacity: 1, scale: 1 }], {
      duration: 160,
      easing: EASE_OUT,
      fill: "both",
    }));
  }

  left = made.length;
  if (!left) {
    window.clearTimeout(watchdog);
    settle();
    return;
  }
  made.forEach((animation) => {
    animation.finished.catch(() => undefined).finally(finish);
  });
}

/** 底部操作条跟着淡入 / 淡出，别在飞行的图前面先"啪"地出现 */
function barFade(el: Element, from: number, to: number, ms: number, delay: number) {
  const bar = el.querySelector(".img-preview-bar");
  if (!bar) return [];
  return [bar.animate([{ opacity: from }, { opacity: to }], { duration: ms, delay, easing: EASE_OUT, fill: "both" })];
}

/**
 * 退场：底衬褪掉，图缩回缩略图的位置
 *
 * 放大看局部时不做归位 —— 从放大后的框往回缩会扯出很大一块，直接淡出更稳。
 *
 * - `el`: 浮层根节点
 * - `done`: Vue 的完成回调
 */
function onLeave(el: Element, done: () => void) {
  if (!animatable()) {
    done();
    return;
  }
  const veil = el.querySelector(".img-preview-veil");
  const img = el.querySelector<HTMLImageElement>(".img-preview img");
  const made: Animation[] = [];
  let settled = false;
  let left = 0;
  const settle = () => {
    if (settled) return;
    settled = true;
    made.forEach((animation) => animation.cancel());
    if (img) img.style.transformOrigin = "";
    done();
  };
  const watchdog = window.setTimeout(settle, 1000);
  const finish = () => {
    left -= 1;
    if (left <= 0) {
      window.clearTimeout(watchdog);
      settle();
    }
  };

  if (veil) {
    made.push(veil.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 180, easing: EASE_OUT, fill: "both" }));
  }
  made.push(...barFade(el, 1, 0, 120, 0));

  const origin = props.origin;
  const target = img ? layoutRect(el, img) : null;
  if (img && origin && target && target.width > 1 && scale.value === 1) {
    img.style.transformOrigin = "top left";
    made.push(img.animate([
      { transform: "none" },
      { transform: flyTransform(origin, target) },
    ], { duration: 200, easing: EASE_IN_OUT, fill: "both" }));
  } else if (img) {
    made.push(img.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 120, easing: EASE_OUT, fill: "both" }));
  }

  left = made.length;
  if (!left) {
    window.clearTimeout(watchdog);
    settle();
    return;
  }
  made.forEach((animation) => {
    animation.finished.catch(() => undefined).finally(finish);
  });
}
</script>

<template>
  <Teleport to="body">
    <!-- appear 不能省：调用方可能用 v-if 挂本组件，不挂 appear 时首次挂载不播进场动画 -->
    <transition name="img-preview-fade" :css="false" appear @enter="onEnter" @leave="onLeave">
      <div v-if="src" class="img-preview" :class="{ pannable: scale > 1, panning: dragged }" @click="onClick"
        @wheel.prevent="onWheel" @pointerdown="onDown">
        <!-- 底衬单独一层：进场时只有它在变暗，图是按缩略图位置飞过来的，不跟着淡 -->
        <span class="img-preview-veil" aria-hidden="true"></span>
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
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: zoom-out;
  /* 放大后溢出部分裁掉 */
  overflow: hidden;
  user-select: none;
  touch-action: none;
}

/* 遮罩：单独一层，进场时由它慢慢变暗，见 onEnter / onLeave */
.img-preview-veil {
  position: absolute;
  inset: 0;
  z-index: 0;
  background: var(--overlay-strong);
  pointer-events: none;
}

/* 放大后可拖动看局部：光标换成抓手 */
.img-preview.pannable {
  cursor: grab;
}

.img-preview.panning {
  cursor: grabbing;
}

.img-preview img {
  /* 压在底衬上面：底衬是定位元素，不抬这一层会被它盖住 */
  position: relative;
  z-index: 1;
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
  z-index: 2;
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
  background: var(--overlay);
  color: var(--on-accent-text);
  font-size: 12px;
  white-space: nowrap;
  pointer-events: none;
}
</style>
