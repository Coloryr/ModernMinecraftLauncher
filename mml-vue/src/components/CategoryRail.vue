<script setup lang="ts">
// 左侧分类栏：全部 + 一组分类 / 分组，条目数跟在右边，栏底可给一行「新建分组」
//
// 方块列表与资源收藏共用这一份外观与交互，各窗口只管把条目算好传进来。
//
// 拖动排序（`sortable`）：按下记候选 → 移动超阈值才开始拖 → 按指针纵坐标算插到第几个 →
// 松手把新顺序抛给上层落盘。拖动期间列表里那一项原地淡出，另有一份跟着指针的浮起副本。
//
// 投放高亮（`dropKey`）：别的窗口把条目拖到分组上时，给对应那一项描一道虚线框。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import { t } from "../lib/i18n";
import { useSlidingThumb } from "../composables/useSlidingThumb";
import { useRailWidth } from "../composables/useRailWidth";

/** 左栏的一项 */
export interface CatItem {
  /** 分类 uuid 或分组名（「全部」不在这里，空串由外壳表示） */
  id: string;
  /** 显示名 */
  label: string;
  /** 条目数 */
  count: number;
  /** 用户自建：可以改名 / 删除，右键给菜单 */
  custom?: boolean;
  /** 为 true 时不当拖放目标：不渲染 `data-group`，指针落上去等于没落在分组上 */
  dropDisabled?: boolean;
  /** 能不能拖动排序；为 false 时按不动（位置由上层定死） */
  reorderable?: boolean;
}

const props = withDefaults(
  defineProps<{
    /** 「全部」那一项的文案 */
    allLabel: string;
    /** 「全部」的条目数 */
    allCount: number;
    /** 其余分类，顺序就是显示顺序 */
    items: CatItem[];
    /** 当前选中的 ID；空串表示「全部」 */
    active: string;
    /** 允许拖动排序 */
    sortable?: boolean;
    /** 别的窗口拖过来的落点：命中的那一项高亮成投放目标；`null` 表示没有 */
    dropKey?: string | null;
    /** 栏底「新建分组」的文案；空串则不显示这一行 */
    newLabel?: string;
  }>(),
  { sortable: false, dropKey: null, newLabel: "" },
);

/** 栏宽度：拖右边缘改，拖过的值记在本地，三个窗口共用一份 */
const { width, railStyle, onResizeStart } = useRailWidth();

const emit = defineEmits<{
  /** 选中一个分类，空串表示「全部」 */
  (e: "pick", id: string): void;
  /** 拖完顺序：给的是新的完整顺序 */
  (e: "reorder", order: string[]): void;
  /** 右键某一项，交给上层弹菜单 */
  (e: "group-menu", payload: { event: MouseEvent; id: string; label: string }): void;
  /** 点了「新建分组」 */
  (e: "new-group"): void;
}>();

// ---------- 拖动排序 ----------

const navEl = ref<HTMLElement | null>(null);
/** 当前选中项下标：「全部」固定第 0 项，其余按传进来的顺序 */
const activeIndex = computed(() => {
  if (props.active === "") return 0;
  const index = props.items.findIndex((item) => item.id === props.active);
  return index < 0 ? -1 : index + 1;
});
const { thumbStyle, thumbReady, refresh: refreshThumb } = useSlidingThumb(
  navEl,
  ".cat-item",
  () => activeIndex.value,
);
watch(() => props.items, refreshThumb, { deep: true });

/** 开始拖拽的位移阈值（与其它拖拽一致） */
const DRAG_THRESHOLD = 5;

/** 按下但还没超过阈值（可能只是点击） */
let candidate: string | null = null;
/** 真正在拖的分类 */
const dragging = ref<string | null>(null);
/** 会插到第几项之前，`null` 表示还没算出来 */
const insertAt = ref<number | null>(null);
/** 跟着指针走的浮起副本位置，视口坐标 */
const ghost = ref({ x: 0, y: 0 });
// 拖拽时插入线会占位，选中色块要跟着重量
watch([dragging, insertAt], refreshThumb);

let pointerId: number | null = null;
let startX = 0;
let startY = 0;
/** 拖拽结束后抑制紧随的 click，否则松手会当成"点了这个分类"而切过去 */
let suppressClick = false;

/** 正在拖的那个分类；没在拖时是 `null` */
const dragged = computed(() => props.items.find((c) => c.id === dragging.value) ?? null);

function onPointerDown(e: PointerEvent, item: CatItem) {
  if (!props.sortable || e.button !== 0 || item.reorderable === false) return;
  suppressClick = false;
  pointerId = e.pointerId;
  startX = e.clientX;
  startY = e.clientY;
  candidate = item.id;
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
  const rows = [...list.querySelectorAll("[data-cat]")] as HTMLElement[];
  let index = rows.length;
  for (let i = 0; i < rows.length; i++) {
    const rect = rows[i].getBoundingClientRect();
    if (e.clientY < rect.top + rect.height / 2) {
      index = i;
      break;
    }
  }
  // 不可拖的项只占位：落到它们前面，等于落到第一个可拖项前面
  const firstMovable = props.items.findIndex((item) => item.reorderable !== false);
  if (firstMovable >= 0 && index < firstMovable) {
    index = firstMovable;
  }
  insertAt.value = index;
}

function onPointerUp(e: PointerEvent) {
  if (pointerId === null || e.pointerId !== pointerId) return;
  const id = dragging.value;
  const index = insertAt.value;
  reset();
  if (!id || index === null) return;

  // 从顺序里摘掉被拖的那一项，再插到目标位置
  const list = props.items.map((c) => c.id);
  const from = list.indexOf(id);
  if (from < 0) return;
  list.splice(from, 1);
  const to = index > from ? index - 1 : index;
  list.splice(Math.max(0, Math.min(list.length, to)), 0, id);
  suppressClick = true;
  emit("reorder", list);
}

function reset() {
  pointerId = null;
  candidate = null;
  dragging.value = null;
  insertAt.value = null;
}

/** 点击：拖拽刚结束时忽略这一下，见 onPointerUp 的 suppressClick */
function onClick(id: string) {
  if (suppressClick) {
    suppressClick = false;
    return;
  }
  emit("pick", id);
}

/** 插入线是否画在第 index 项之前 */
function showLine(index: number): boolean {
  return dragging.value !== null && insertAt.value === index;
}

/** 右键：交给上层决定给不给菜单（「全部」与自带分类由上层忽略） */
function onContextMenu(e: MouseEvent, item: CatItem) {
  e.preventDefault();
  emit("group-menu", { event: e, id: item.id, label: item.label });
}

onMounted(() => {
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("pointercancel", reset);
});
onUnmounted(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", reset);
});
</script>

<template>
  <aside ref="navEl" class="cat-rail" :class="{ ready: thumbReady }" :style="railStyle" role="tablist"
    aria-orientation="vertical">
    <!-- 分类列表：整栏里唯一滚动的一段（栏底那颗添加键不跟着滚，见 .cat-new-slot） -->
    <div class="cat-scroll">
      <span class="cat-thumb" :style="thumbStyle" aria-hidden="true"></span>
      <!-- 全部：不参与排序，也不给右键 -->
      <button type="button" role="tab" class="cat-item" :class="{ on: active === '' }" :aria-selected="active === ''"
        @click="onClick('')">
        <span class="cat-name">{{ allLabel }}</span>
        <span class="cat-num">{{ allCount }}</span>
      </button>

      <template v-for="(c, idx) in items" :key="c.id">
        <!-- 插入线：松手后这一项会落到这里 -->
        <div v-if="showLine(idx)" class="cat-insert">
          <span class="cat-name">{{ dragged?.label }}</span>
          <span class="cat-num">{{ dragged?.count }}</span>
        </div>
        <button type="button" role="tab" class="cat-item"
          :class="{ on: active === c.id, dragging: dragging === c.id, 'drop-target': dropKey === c.id }" :data-cat="c.id"
          :data-group="c.dropDisabled ? undefined : c.id" :aria-selected="active === c.id"
          @pointerdown="onPointerDown($event, c)" @click="onClick(c.id)" @contextmenu="onContextMenu($event, c)">
          <span class="cat-name">{{ c.label }}</span>
          <span class="cat-num">{{ c.count }}</span>
        </button>
      </template>
      <div v-if="showLine(items.length)" class="cat-insert">
        <span class="cat-name">{{ dragged?.label }}</span>
        <span class="cat-num">{{ dragged?.count }}</span>
      </div>
    </div>

    <!-- 添加分组：不属于滚动内容，始终贴在这一栏的底部（与资源管理栏底那颗「添加资源」
         同一处、同一套外观） -->
    <div v-if="newLabel" class="cat-new-slot">
      <button type="button" class="add-row-btn" @click="emit('new-group')">
        <GlyphIcon name="plus" :size="14" :weight="2.2" />
        <span class="cat-name">{{ newLabel }}</span>
      </button>
    </div>

    <!-- 右边缘：拖动改宽度。热区比视觉上宽一点，贴着栏边不挡内容 -->
    <span class="rail-split" v-tip="t('common.railResize')" :aria-label="t('common.railResize')"
      role="separator" aria-orientation="vertical" @pointerdown="onResizeStart" />

    <!-- 拖动时的浮起副本：跟着指针走，位置见 ghost；列表里那一项留在原位淡出 -->
    <Teleport to="body">
      <div v-if="dragged" class="cat-ghost"
        :style="{ left: `${ghost.x}px`, top: `${ghost.y}px`, width: `${width - 2}px` }">
        <span class="cat-name">{{ dragged.label }}</span>
        <span class="cat-num">{{ dragged.count }}</span>
      </div>
    </Teleport>
  </aside>
</template>

<style scoped>
.cat-rail {
  position: relative;
  width: var(--rail-w);
  min-width: var(--rail-w);
  display: flex;
  flex-direction: column;
  /* 栏自己不滚：只有上面的分类列表滚，栏底的添加键固定不动 */
  overflow: hidden;
  /* 占满窗口左侧：向外吃掉内容区的内边距（上下 22、左 26），贴到窗口三面；
     底色与右侧描边跟主窗口侧栏同一套 */
  margin: -22px 0 -22px -26px;
  padding: 22px 8px 22px 12px;
  background: var(--bg-side);
  border-right: 1px solid var(--border);
}

/* 分类列表：整栏里唯一滚动的那一段；上下留白由栏自己的 padding 给 */
.cat-scroll {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  padding: 0;
}

/* 选中色块：分类切换时在栏内上下滑动 */
.cat-thumb {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 0;
  border-radius: var(--ctl-radius);
  background: var(--accent-soft);
  opacity: 0;
  pointer-events: none;
  transition: none;
}

.cat-rail.ready .cat-thumb {
  opacity: 1;
  transition:
    transform 0.18s var(--ease-out),
    width 0.18s var(--ease-out),
    height 0.18s var(--ease-out);
}

.cat-item {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 11px;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.cat-item:hover:not(.on) {
  background: var(--bg-hover);
}

.cat-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.cat-item.on {
  background: transparent;
  color: var(--accent);
  font-weight: 600;
}

/* 第一次量完之前没有色块，给选中项一个背景兜底 */
.cat-rail:not(.ready) .cat-item.on {
  background: var(--accent-soft);
}

/* 正被拖的分类：留在原位淡下去（顺序由插入线表达） */
.cat-item.dragging {
  opacity: 0.45;
}

/* 投放目标：别的窗口把条目拖到这一组上时描一道虚线。
   outline 不占布局，指针经过时不会把下面的项顶动 */
.cat-item.drop-target {
  outline: 1px dashed var(--accent);
  outline-offset: -1px;
  background: var(--accent-soft);
}


/* 栏底的添加键：不参与滚动，永远贴在这一栏底部，与上面的分类留 6px */
.cat-new-slot {
  flex-shrink: 0;
  padding: 6px 0 0;
}

/* 拖拽落点占位：虚线框 + 被拖分类的名字与数量 */
.cat-insert {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin: 2px 0;
  padding: 8px 11px;
  border: 1px dashed var(--accent-border);
  border-radius: var(--ctl-radius);
  background: var(--accent-soft);
  opacity: 0.7;
}

.cat-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cat-num {
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.cat-item.on .cat-num {
  color: var(--accent);
}

/* 跟手浮起的那一份：底色 + 强调色描边 + 大阴影做出"被拿起来"的观感。
   位置拿到的是**指针坐标**，所以整体往左上挪一截，别被光标盖住 */
.cat-ghost {
  position: fixed;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: calc(var(--rail-w) - 2px);
  padding: 8px 11px;
  border: 1px solid var(--accent-border);
  border-radius: var(--ctl-radius);
  background: var(--bg-modal);
  color: var(--accent);
  font-size: 13px;
  font-weight: 600;
  box-shadow: var(--shadow-lg);
  transform: translate(-40%, -50%) scale(1.04);
  pointer-events: none;
}
</style>
