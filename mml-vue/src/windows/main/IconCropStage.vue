<script setup lang="ts">
// 修改图标 · 选区裁剪：在图片上拖一个方框选范围
//
// 只用 DOM 定位，不上 canvas：展示区按图片等比缩放到上限以内，选区是区内一个方框，
// 拖动移动、右下角把手缩放，恒为正方形 —— 图标框就是方的。
// 对外只给**原图像素坐标**，后端直接拿去 `crop_imm`；坐标换算只在这一处：
// 因而 `src` 可以是降采样预览，`width` / `height` 必须是原图尺寸，两者由比例换算连起来。
import { computed, onUnmounted, ref, watch } from "vue";

const props = defineProps<{
  /** 预览图 data URL，由后端 `block_read_icon_source` 给出，已降采样，可直接当 src */
  src: string;
  /** **原图**宽，单位 px；选区与坐标换算都按原图，预览尺寸不参与 */
  width: number;
  /** **原图**高，单位 px；与 `width` 一起定预览比例 */
  height: number;
}>();

/** 选区变化，抛出原图像素坐标下的裁剪框 */
const emit = defineEmits<{ (e: "change", area: { x: number; y: number; w: number; h: number }): void }>();

/** 展示区上限，单位 px；小图不放大，免得糊 */
const MAX = 560;
/**
 * 矮窗口下的展示区下限，单位 px
 *
 * 弹窗整体受 `max-height: 85vh` 约束：展示区顶到 560 时，768p 这类屏幕上标题与按钮会被
 * 挤出可视区，弹窗一滚选区就看不见了，所以按窗口高度收一收。只在挂载时算一次——
 * 组件随弹窗 `v-if` 创建，开弹窗时取到的就是当时的窗口高度。
 */
const MIN_STAGE = 280;
/** 选区最小边长，单位 px，按展示坐标算 */
const MIN_SEL = 24;

/** 本次实际使用的展示区上限，单位 px */
const maxBox = Math.max(MIN_STAGE, Math.min(MAX, Math.round(window.innerHeight * 0.55)));

const scale = computed(() => Math.min(1, maxBox / Math.max(props.width || 1, props.height || 1)));
const boxW = computed(() => Math.max(1, Math.round(props.width * scale.value)));
const boxH = computed(() => Math.max(1, Math.round(props.height * scale.value)));

/** 选区（展示坐标） */
const sel = ref({ x: 0, y: 0, size: 0 });

/** 默认选区：居中的最大正方形 */
function resetSel() {
  const size = Math.min(boxW.value, boxH.value);
  sel.value = {
    x: Math.round((boxW.value - size) / 2),
    y: Math.round((boxH.value - size) / 2),
    size,
  };
}

watch(() => [props.src, boxW.value, boxH.value], resetSel, { immediate: true });

/** 选区换算成原图像素（后端按这个裁） */
const area = computed(() => {
  const s = sel.value;
  const size = Math.max(1, Math.round(s.size / scale.value));
  return {
    x: Math.round(s.x / scale.value),
    y: Math.round(s.y / scale.value),
    w: size,
    h: size,
  };
});

watch(area, (v) => emit("change", v), { immediate: true, deep: true });

// ---------- 拖拽：移动 / 右下角缩放 ----------

type DragKind = "move" | "resize";
let dragging: DragKind | null = null;
let from = { mx: 0, my: 0, x: 0, y: 0, size: 0 };

function onDown(e: PointerEvent, kind: DragKind) {
  e.preventDefault();
  dragging = kind;
  from = {
    mx: e.clientX,
    my: e.clientY,
    x: sel.value.x,
    y: sel.value.y,
    size: sel.value.size,
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
}

function onMove(e: PointerEvent) {
  if (!dragging) return;
  const dx = e.clientX - from.mx;
  const dy = e.clientY - from.my;
  if (dragging === "move") {
    const size = sel.value.size;
    sel.value = {
      ...sel.value,
      x: Math.min(Math.max(0, from.x + dx), Math.max(0, boxW.value - size)),
      y: Math.min(Math.max(0, from.y + dy), Math.max(0, boxH.value - size)),
    };
    return;
  }
  // 缩放：左上角为锚点、保持正方形，上界是容器剩下的空间
  const limit = Math.max(MIN_SEL, Math.min(boxW.value - from.x, boxH.value - from.y));
  const size = Math.min(Math.max(MIN_SEL, from.size + Math.max(dx, dy)), limit);
  sel.value = { x: from.x, y: from.y, size };
}

function onUp() {
  dragging = null;
  window.removeEventListener("pointermove", onMove);
  window.removeEventListener("pointerup", onUp);
}

onUnmounted(onUp);
</script>

<template>
  <div class="stage" :style="{ width: boxW + 'px', height: boxH + 'px' }">
    <img class="stage-img" :src="src" :width="boxW" :height="boxH" alt="" draggable="false" />
    <!-- 选区外压暗：靠一层巨大的 box-shadow 铺满，省掉四块遮罩 -->
    <div class="sel" :style="{
      left: sel.x + 'px',
      top: sel.y + 'px',
      width: sel.size + 'px',
      height: sel.size + 'px',
    }" @pointerdown="onDown($event, 'move')">
      <span class="sel-handle" @pointerdown.stop="onDown($event, 'resize')"></span>
    </div>
  </div>
</template>

<style scoped>
.stage {
  position: relative;
  /* 只裁掉选区外压暗的那层 box-shadow，**不给圆角**：
     截图要能看到图片真实的四个角，圆角会挡住边界、选区贴角时也会误导 */
  overflow: hidden;
  background: var(--bg-side);
  /* 拖拽时不要选中图片 / 触发系统拖图 */
  user-select: none;
  touch-action: none;
}

.stage-img {
  display: block;
  -webkit-user-drag: none;
}

.sel {
  position: absolute;
  box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.45);
  outline: 1px solid var(--accent);
  cursor: move;
}

.sel-handle {
  position: absolute;
  right: -6px;
  bottom: -6px;
  width: 12px;
  height: 12px;
  border: 2px solid var(--bg-card);
  border-radius: 3px;
  background: var(--accent);
  cursor: nwse-resize;
}
</style>
