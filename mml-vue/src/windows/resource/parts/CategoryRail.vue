<script setup lang="ts">
// 左侧分类导航：按用户拖出来的顺序渲染；拖一下即改顺序（写回视图偏好）
//
// 拖拽沿用仓库里那套指针模拟，见 composables/useInstanceDrag、useCollectDrag：
// 不用 HTML5 draggable，WebView2 里会出现"禁止"光标且投放不可靠；
// 按下记候选 → 移动超阈值才开始拖 → 按指针位置算插到第几个 → 松手写回。
// 这一列是**一维顺序**，没有"投放到哪个容器"的问题，所以只需要一个插入位置。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "../../../lib/i18n";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { RESOURCE_CATEGORIES, type CategoryId } from "../types";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceView } from "../composables/useResourceView";

const props = defineProps<{
  /** 数据层，当前选中的分类从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 视图偏好，分类顺序与写回从这里取 */
  view: ReturnType<typeof useResourceView>;
}>();

const emit = defineEmits<{
  /** 点了某个分类，外壳据此切换面板 */
  (e: "select", id: CategoryId): void;
}>();

const { category } = props.data;
const { order, setOrder } = props.view;

/** 按偏好顺序排好的分类，含文案键 */
const items = computed(() =>
  order.value
    .map((id) => RESOURCE_CATEGORIES.find((c) => c.id === id))
    .filter((c) => !!c),
);

/** 分类列表容器；算插入位置时只在本组件范围内查，不用全局选择器 */
const navEl = ref<HTMLElement | null>(null);

/** 开始拖拽的位移阈值（与实例 / 收藏拖拽一致） */
const DRAG_THRESHOLD = 5;

/** 按下但还没超过阈值（可能只是点击） */
let candidate: CategoryId | null = null;
/** 真正在拖的分类 */
const dragging = ref<CategoryId | null>(null);
/** 会插到第几项之前，`null` 表示还没算出来 */
const insertAt = ref<number | null>(null);

/** 跟着指针走的浮起副本位置，视口坐标 */
const ghost = ref({ x: 0, y: 0 });

/** 正在拖的那个分类；没在拖时是 `null` */
const dragged = computed(() => items.value.find((c) => c.id === dragging.value) ?? null);

let pointerId: number | null = null;
let startX = 0;
let startY = 0;
/** 拖拽结束后抑制紧随的 click，否则松手会当成"点了这个分类"而切过去 */
let suppressClick = false;

function onPointerDown(e: PointerEvent, id: CategoryId) {
  if (e.button !== 0) return;
  suppressClick = false;
  pointerId = e.pointerId;
  startX = e.clientX;
  startY = e.clientY;
  candidate = id;
  dragging.value = null;
  insertAt.value = null;
  ghost.value = { x: e.clientX, y: e.clientY };
}

function onPointerMove(e: PointerEvent) {
  if (pointerId === null || e.pointerId !== pointerId || !candidate) return;
  ghost.value = { x: e.clientX, y: e.clientY };
  if (!dragging.value) {
    if (Math.hypot(e.clientX - startX, e.clientY - startY) <= DRAG_THRESHOLD) return;
    dragging.value = candidate;
  }
  updateInsert(e);
}

/** 按指针纵坐标算插入位置；分类项是等高的一列，取每项中线比较 */
function updateInsert(e: PointerEvent) {
  const list = navEl.value;
  if (!list) return;
  const rows = [...list.querySelectorAll(".cat-item")] as HTMLElement[];
  let index = rows.length;
  for (let i = 0; i < rows.length; i++) {
    const rect = rows[i].getBoundingClientRect();
    if (e.clientY < rect.top + rect.height / 2) {
      index = i;
      break;
    }
  }
  insertAt.value = index;
}

function onPointerUp(e: PointerEvent) {
  if (pointerId === null || e.pointerId !== pointerId) return;
  const id = dragging.value;
  const index = insertAt.value;
  reset();
  if (!id || index === null) return;

  // 从顺序里摘掉被拖的那一项，再插到目标位置。
  // 被拖项原本在插入点之前时，摘掉之后目标下标要减一
  const list = [...order.value];
  const from = list.indexOf(id);
  if (from < 0) return;
  list.splice(from, 1);
  const to = index > from ? index - 1 : index;
  list.splice(Math.max(0, Math.min(list.length, to)), 0, id);
  suppressClick = true;
  setOrder(list);
}

function reset() {
  pointerId = null;
  candidate = null;
  dragging.value = null;
  insertAt.value = null;
}

/** 点击：拖拽刚结束时忽略这一下，见 onPointerUp 的 suppressClick */
function onClick(id: CategoryId) {
  if (suppressClick) {
    suppressClick = false;
    return;
  }
  emit("select", id);
}

/** 插入线是否画在第 index 项之前 */
function showLine(index: number): boolean {
  return dragging.value !== null && insertAt.value === index;
}

onMounted(() => {
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  // 指针被系统收走（触摸被打断等）当作取消：别把某一项留在"拖拽中"
  window.addEventListener("pointercancel", reset);
});
onUnmounted(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", reset);
});
</script>

<template>
  <aside ref="navEl" class="cat-nav">
    <template v-for="(c, idx) in items" :key="c.id">
      <!-- 插入线：松手后这一项会落到这里 -->
      <div v-if="showLine(idx) && dragged" class="cat-insert">
        <GlyphIcon :name="dragged.icon" :size="15" />
        <span class="cat-name">{{ t(dragged.labelKey) }}</span>
      </div>
      <button class="cat-item" :class="{ active: category === c.id, dragging: dragging === c.id }"
        @pointerdown="onPointerDown($event, c.id)" @click="onClick(c.id)">
        <GlyphIcon :name="c.icon" :size="15" />
        {{ t(c.labelKey) }}
      </button>
    </template>
    <div v-if="showLine(items.length) && dragged" class="cat-insert">
      <GlyphIcon :name="dragged.icon" :size="15" />
      <span class="cat-name">{{ t(dragged.labelKey) }}</span>
    </div>

    <!-- 拖动时的浮起副本：跟着指针走，位置见 ghost；列表里那一项留在原位淡出 -->
    <Teleport to="body">
      <div v-if="dragged" class="cat-ghost" :style="{ left: `${ghost.x}px`, top: `${ghost.y}px` }">
        <GlyphIcon :name="dragged.icon" :size="15" />
        {{ t(dragged.labelKey) }}
      </div>
    </Teleport>
  </aside>
</template>

<style scoped>
/* 拖动时那份跟手的副本：底色 + 强调色描边 + 大阴影做出"被拿起来"的观感。
   它 Teleport 到 body，摸不到 resource.css 里 .resource-layout 前缀那套，
   尺寸与排版在这里自己写一份。
   位置拿到的是**指针坐标**，所以整体往左上挪一截，别被光标盖住 */
.cat-ghost {
  position: fixed;
  z-index: 200;
  display: flex;
  align-items: center;
  gap: 9px;
  width: calc(var(--rail-w) - 2px);
  padding: 10px 12px;
  border: 1px solid var(--accent-border);
  border-radius: 10px;
  background: var(--bg-modal);
  color: var(--accent);
  font-size: 13px;
  font-weight: 600;
  box-shadow: var(--shadow-lg);
  transform: translate(-40%, -50%) scale(1.04);
  pointer-events: none;
}
</style>
