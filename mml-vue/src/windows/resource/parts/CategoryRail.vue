<script setup lang="ts">
// 左侧分类导航：按用户拖出来的顺序渲染；拖一下即改顺序（写回视图偏好）
//
// 拖拽沿用仓库里那套指针模拟，见 composables/useInstanceDrag、useCollectDrag：
// 不用 HTML5 draggable，WebView2 里会出现"禁止"光标且投放不可靠；
// 按下记候选 → 移动超阈值才开始拖 → 按指针位置算插到第几个 → 松手写回。
// 这一列是**一维顺序**，没有"投放到哪个容器"的问题，所以只需要一个插入位置。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import { useSlidingThumb } from "../../../composables/useSlidingThumb";
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
  /** 点了栏底的「添加资源」，外壳据此开下载窗口 */
  (e: "add"): void;
}>();

const { category, instanceUuid } = props.data;

/** 有实例才放行添加资源：下载窗口一个实例一个，没实例无处可放 */
const canAdd = computed(() => !!instanceUuid.value);
const { order, setOrder } = props.view;

/** 按偏好顺序排好的分类，含文案键 */
const items = computed(() =>
  order.value
    .map((id) => RESOURCE_CATEGORIES.find((c) => c.id === id))
    .filter((c) => !!c),
);

/** 分类列表容器；算插入位置时只在本组件范围内查，不用全局选择器 */
const navEl = ref<HTMLElement | null>(null);

/** 当前分类在顺序里的下标 */
const activeIndex = computed(() => items.value.findIndex((item) => item.id === category.value));
const { thumbStyle, thumbReady, refresh: refreshThumb } = useSlidingThumb(
  navEl,
  ".cat-item",
  () => activeIndex.value,
);
watch(items, refreshThumb, { deep: true });

/** 开始拖拽的位移阈值（与实例 / 收藏拖拽一致） */
const DRAG_THRESHOLD = 8;

/** 按下但还没超过阈值（可能只是点击） */
let candidate: CategoryId | null = null;
/** 真正在拖的分类 */
const dragging = ref<CategoryId | null>(null);
/** 会插到第几项之前，`null` 表示还没算出来 */
const insertAt = ref<number | null>(null);

/** 跟着指针走的浮起副本位置，视口坐标 */
const ghost = ref({ x: 0, y: 0 });
// 拖拽时插入线会占位，选中色块要跟着重量
watch([dragging, insertAt], refreshThumb);

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
  <aside ref="navEl" class="cat-nav relative flex w-[var(--rail-w)] min-w-[var(--rail-w)] flex-col gap-1" :class="{ ready: thumbReady }">
    <span class="cat-thumb pointer-events-none absolute left-0 top-0 z-0 rounded-[9px] bg-accent-soft opacity-0 transition-none" :style="thumbStyle" aria-hidden="true"></span>
    <template v-for="(c, idx) in items" :key="c.id">
      <!-- 插入线：松手后这一项会落到这里 -->
      <div v-if="showLine(idx) && dragged" class="cat-insert my-0.5 flex items-center gap-[9px] rounded-ctl border border-dashed border-accent-border bg-accent-soft px-[11px] py-2 opacity-70">
        <GlyphIcon :name="dragged.icon" :size="15" />
        <span class="cat-name">{{ t(dragged.labelKey) }}</span>
      </div>
      <button class="cat-item relative z-[1] flex cursor-pointer items-center gap-[9px] rounded-[9px] border-0 bg-transparent p-[10px_12px] text-left text-title text-dim transition-all duration-100 [font-family:inherit]" :class="{ active: category === c.id, dragging: dragging === c.id }"
        @pointerdown="onPointerDown($event, c.id)" @click="onClick(c.id)">
        <GlyphIcon :name="c.icon" :size="15" />
        {{ t(c.labelKey) }}
      </button>
    </template>
    <div v-if="showLine(items.length) && dragged" class="cat-insert my-0.5 flex items-center gap-[9px] rounded-ctl border border-dashed border-accent-border bg-accent-soft px-[11px] py-2 opacity-70">
      <GlyphIcon :name="dragged.icon" :size="15" />
      <span class="cat-name">{{ t(dragged.labelKey) }}</span>
    </div>

    <!-- 添加资源：原先在标题栏那颗加号，挪到分类栏**底部**（mt-auto 把它推到栏底），
         与另两个分类栏的「添加分组」同一档外观 -->
    <button type="button" class="add-row-btn cat-add mt-auto" :disabled="!canAdd" @click="emit('add')">
      <GlyphIcon name="plus" :size="14" :weight="2.2" />
      <span>{{ t("winTitle.addResource") }}</span>
    </button>

    <!-- 拖动时的浮起副本：跟着指针走，位置见 ghost；列表里那一项留在原位淡出 -->
    <Teleport to="body">
      <div v-if="dragged" class="cat-ghost pointer-events-none fixed z-[200] flex w-[calc(var(--rail-w)-2px)] items-center gap-[9px] rounded-card border border-accent-border bg-modal p-[10px_12px] text-title font-semibold text-accent shadow-[var(--shadow-lg)] [transform:translate(-40%,-50%)_scale(1.04)]" :style="{ left: `${ghost.x}px`, top: `${ghost.y}px` }">
        <GlyphIcon :name="dragged.icon" :size="15" />
        {{ t(dragged.labelKey) }}
      </div>
    </Teleport>
  </aside>
</template>

