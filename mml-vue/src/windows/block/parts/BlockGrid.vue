<script setup lang="ts">
// 方块列表 · 网格（虚拟滚动）
//
// 一千多个方块一次性挂载会明显卡顿，每个格子一个组件加一张贴图，所以只渲染视口内的行：
// 容器宽度定列数、尺寸档定固定格高，行高 = 格高 + 间距，再用一个等高的占位撑出滚动条。
//
// 键盘：格子间用方向键移动，roving tabindex 只有当前格 tabindex=0；Home / End 到首尾；
// 移动前会先把目标行滚进视口 —— 否则目标还没被渲染出来，取不到 DOM 也就聚焦不了。
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import AsyncImage from "../../../components/ui/AsyncImage.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import type { BlockItemDto } from "../../../lib/bindings";
import { BLOCK_SIZES, type BlockSize } from "../types";

const props = defineProps<{
  /** 过滤后的方块条目，按后端顺序 */
  items: BlockItemDto[];
  /** 搜索词，用于名字高亮 */
  keyword: string;
  /** 图标尺寸档，决定列宽、格高与字号 */
  size: BlockSize;
  /** 详情里正在看的方块（网格里高亮） */
  activeId: string | null;
  /** 玩家头颅分类的原始 id：这类格子带悬停删除角标 */
  skinCat: string;
  /** 是否处于筛选态，决定空状态要不要给「清除筛选」 */
  filtered: boolean;
  /** 收藏判定：收藏过的方块在左上角常显实心星 */
  isFavorite?: (id: string) => boolean;
  /** 切换收藏：点悬停露出的星框或已收藏的星标时调用 */
  toggleFavorite?: (id: string) => void;
  /** 多选模式：点击格子只切换勾选，不打开详情 */
  selectMode?: boolean;
  /** 某个方块当前是否被勾选 */
  isSelected?: (id: string) => boolean;
}>();

const emit = defineEmits<{
  /** 点开某个方块的详情 */
  (e: "open", b: BlockItemDto): void;
  /** 删除某个玩家头颅，仅 `skinCat` 那一类会发 */
  (e: "remove-skin", b: BlockItemDto): void;
  /** 空状态里请求清掉搜索词与分类 */
  (e: "clear-filters"): void;
  /** 右键某个方块：交给外壳弹收藏 / 分组菜单 */
  (e: "item-menu", payload: { event: MouseEvent; item: BlockItemDto }): void;
  /** 多选模式下点了一个方块：切换它的勾选状态 */
  (e: "toggle-select", b: BlockItemDto): void;
}>();

/** 点击格子：多选模式下只切换勾选，否则打开详情 */
function onCellClick(b: BlockItemDto) {
  if (props.selectMode) emit("toggle-select", b);
  else emit("open", b);
}

/** 右键格子：多选模式不弹单项菜单，批量操作走多选条 */
function onCellMenu(event: MouseEvent, b: BlockItemDto) {
  if (!props.selectMode) emit("item-menu", { event, item: b });
}

/** 视口外上下各多渲染几行，滚动时不至于看到空白 */
const OVERSCAN = 2;

/**
 * 列表末尾的留白，单位 px
 *
 * 由滚动内容自己提供，即加在占位高度上，而不是把滚动区从底部缩进来：
 * 这样滚动条能一直延伸到底边，而"最后一行下面有空档"只在滚到最下面时出现。
 * 数值等于窗口内容区原本的下内边距 22px，观感与之前一致。
 */
const BOTTOM_SPACE = 22;

// ---------- 度量 ----------

const metrics = computed(() => BLOCK_SIZES[props.size]);
const gap = computed(() => metrics.value.gap);
/** 虚拟滚动的行高；必须与实际渲染的格子高度一致 */
const rowH = computed(() => metrics.value.cell + metrics.value.gap);

const scroller = ref<HTMLDivElement | null>(null);
/** 网格本体：列宽要按它的实际宽度算；它比滚动容器窄，右侧留了 26px 呼吸 */
const gridEl = ref<HTMLDivElement | null>(null);
const gridW = ref(0);
const viewportH = ref(0);
const scrollTop = ref(0);

let ro: ResizeObserver | null = null;
let raf = 0;

function measure(el: HTMLElement) {
  // 容器通栏（滚动条贴窗口右边），网格在容器里右侧缩进；列数按**网格**宽度算，
  // 否则列会按"更宽"的容器宽度去分，格子被压窄、最后一列还会顶到留白区里
  gridW.value = gridEl.value?.clientWidth || el.clientWidth;
  viewportH.value = el.clientHeight;
}

/** 拿到 DOM 后重新量一次；网格是 v-if 挂上来的，首次量的时候可能还不存在 */
async function remeasure() {
  const el = scroller.value;
  if (!el) return;
  await nextTick();
  measure(el);
}

onMounted(() => {
  const el = scroller.value;
  if (!el) return;
  void remeasure();
  ro = new ResizeObserver(() => measure(el));
  ro.observe(el);
});

// 列表从空变有（或反向）时网格才会挂上/卸载，宽度要跟着重算
watch(
  () => props.items.length,
  () => void remeasure(),
);

onUnmounted(() => {
  ro?.disconnect();
  if (raf) cancelAnimationFrame(raf);
});

/** 滚动只记最新位置：一帧内的多次 scroll 合并成一次计算 */
function onScroll() {
  if (raf) return;
  raf = requestAnimationFrame(() => {
    raf = 0;
    scrollTop.value = scroller.value?.scrollTop ?? 0;
  });
}

// ---------- 可视窗口 ----------

const cols = computed(() => {
  const { minCol } = metrics.value;
  return Math.max(1, Math.floor((gridW.value + gap.value) / (minCol + gap.value)));
});
const rows = computed(() => Math.ceil(props.items.length / cols.value));
const startRow = computed(() => Math.max(0, Math.floor(scrollTop.value / rowH.value) - OVERSCAN));
const endRow = computed(() =>
  Math.min(rows.value, Math.ceil((scrollTop.value + viewportH.value) / rowH.value) + OVERSCAN),
);
const startIndex = computed(() => startRow.value * cols.value);
const visible = computed(() => props.items.slice(startIndex.value, endRow.value * cols.value));
/** 占位高度（末行后面不留 gap） */
const totalH = computed(() => Math.max(0, rows.value * rowH.value - gap.value));
const offsetY = computed(() => startRow.value * rowH.value);

/** 尺寸档通过 CSS 变量注入格子样式，虚拟滚动与实际高度不会各写一套 */
const gridStyle = computed(() => {
  const m = metrics.value;
  return {
    "grid-template-columns": `repeat(${cols.value}, minmax(0, 1fr))`,
    gap: `${m.gap}px`,
    transform: `translateY(${offsetY.value}px)`,
    "--cell-h": `${m.cell}px`,
    "--cell-img": `${m.img}px`,
    "--cell-pad": `${m.pad}px`,
    "--cell-inner-gap": `${Math.max(4, Math.round(m.gap * 0.6))}px`,
    "--cell-font": `${m.font}px`,
  };
});

// 筛掉一堆方块后内容变矮，浏览器会把滚动位置夹回去并可能不再发 scroll 事件，
// 这里在 DOM 更新后同步一次，免得可视窗口按旧的偏大偏移量算成空白
watch(
  totalH,
  () => {
    const el = scroller.value;
    if (el && el.scrollTop !== scrollTop.value) scrollTop.value = el.scrollTop;
  },
  { flush: "post" },
);

// ---------- 键盘 ----------

const focusIndex = ref(0);

/**
 * 唯一可 Tab 进入的格子
 *
 * 虚拟列表里 `focusIndex` 指向的项可能已被滚出渲染范围，此时窗口内没有任何格子
 * tabindex=0，整个网格会被 Tab 跳过，所以窗口内找不到它时退回到窗口第一项。
 */
const tabbableIndex = computed(() => {
  const start = startIndex.value;
  const end = start + visible.value.length;
  return focusIndex.value >= start && focusIndex.value < end ? focusIndex.value : start;
});

// 列表长度变化（过滤 / 换语言）后把焦点索引收回范围
watch(
  () => props.items.length,
  (n) => {
    if (focusIndex.value >= n) focusIndex.value = 0;
  },
);

function clampIndex(i: number) {
  const n = props.items.length;
  return n ? Math.min(n - 1, Math.max(0, i)) : 0;
}

/** 焦点落到第 i 个：先把它所在行滚进视口，等渲染完再聚焦 */
async function focusAt(i: number) {
  const idx = clampIndex(i);
  focusIndex.value = idx;
  const el = scroller.value;
  if (el) {
    const top = Math.floor(idx / cols.value) * rowH.value;
    const bottom = top + rowH.value;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = bottom - el.clientHeight;
    // 立刻同步，别等 rAF：下面 nextTick 时可视窗口要已经切过去
    scrollTop.value = el.scrollTop;
  }
  await nextTick();
  scroller.value?.querySelector<HTMLElement>(`[data-idx="${idx}"]`)?.focus();
}

function onCellKey(e: KeyboardEvent, i: number) {
  const c = cols.value;
  switch (e.key) {
    case "ArrowLeft":
      e.preventDefault();
      void focusAt(i - 1);
      break;
    case "ArrowRight":
      e.preventDefault();
      void focusAt(i + 1);
      break;
    case "ArrowUp":
      e.preventDefault();
      void focusAt(i - c);
      break;
    case "ArrowDown":
      e.preventDefault();
      void focusAt(i + c);
      break;
    case "Home":
      e.preventDefault();
      void focusAt(0);
      break;
    case "End":
      e.preventDefault();
      void focusAt(props.items.length - 1);
      break;
  }
}
</script>

<template>
  <div ref="scroller" class="grid-scroll" @scroll.passive="onScroll">
    <div v-if="items.length" class="grid-pad" :style="{ height: totalH + BOTTOM_SPACE + 'px' }">
      <div ref="gridEl" class="block-grid" :style="gridStyle">
        <button v-for="(b, k) in visible" :key="b.id" type="button" class="block-cell"
          :class="{ on: b.id === activeId, 'multi-checked': selectMode && isSelected?.(b.id) }"
          :data-idx="startIndex + k" :tabindex="startIndex + k === tabbableIndex ? 0 : -1"
          @click="onCellClick(b)" @focus="focusIndex = startIndex + k"
          @keydown="onCellKey($event, startIndex + k)" @contextmenu.prevent="onCellMenu($event, b)">
          <!-- 多选模式：每格右上角一枚勾选圈，选定后填充强调色 -->
          <span v-if="selectMode" class="row-check" :class="{ on: isSelected?.(b.id) }">
            <GlyphIcon v-if="isSelected?.(b.id)" name="check" :size="11" :weight="3" />
          </span>
          <!-- 收藏：未收藏只在悬停 / 键盘聚焦时露出星框，收藏后常显实心星；多选时让位给勾选圈 -->
          <span v-if="!selectMode" class="block-fav" :class="{ 'is-on': isFavorite?.(b.id) }"
            v-tip="isFavorite?.(b.id) ? t('blocks.unfavorite') : t('blocks.favorite')"
            @click.stop="toggleFavorite?.(b.id)">
            <GlyphIcon name="star" :size="13" :weight="2" />
          </span>
          <!-- 玩家头颅：悬停角标删除；多选时同样让位 -->
          <span v-if="!selectMode && b.catId === skinCat" class="block-del" v-tip="t('blocks.skinRemove')"
            @click.stop="emit('remove-skin', b)">
            <GlyphIcon name="close" :size="11" :weight="2.6" />
          </span>
          <AsyncImage class="block-img" :src="b.image" :alt="b.name" />
          <span class="block-name">
            <HighlightText :text="b.name" :query="keyword" />
          </span>
        </button>
      </div>
    </div>

    <div v-else class="empty-tip grid-empty">
      <span>{{ t("blocks.empty") }}</span>
      <button v-if="filtered" type="button" class="empty-clear" @click="emit('clear-filters')">
        {{ t("blocks.clearFilters") }}
      </button>
    </div>
  </div>
</template>

<style scoped src="../../../styles/parts/instance-check.css"></style>

<style scoped>
.grid-scroll {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  /* 顶部留白 = 窗口内容区原本的上内边距（22px）+ 悬停上浮的 2px 余量：
     它是滚动内容的一部分，所以只在滚到最上面时出现，滚动条轨道仍是整条；
     否则第一排格子悬停上浮时会被裁掉上边缘那条线 */
  padding-top: 24px;
  /* 右侧不留内边距：滚动条要贴窗口右边缘，父级已抵消 frame-body 的右内边距 */
}

/* 等高占位：撑出滚动条长度；真正的格子绝对定位在里面并按行偏移 */
.grid-pad {
  position: relative;
}

.block-grid {
  position: absolute;
  top: 0;
  left: 0;
  /* 右侧缩进与 .block-main 的 16px 间距一致：原来取 26px 与窗口左内边距对齐，
     但滚动条本身还占约 9px，看着右边比左边空得多；滚动条仍在窗口最右边。
     列数按这个更窄的宽度算，见 script 里的 gridW */
  right: 16px;
  display: grid;
}

/* 收藏角标：左上角，外观与资源窗口的星标一致 —— 透明底，未收藏是描边星，
   收藏后填充成固定黄色；悬停 / 键盘聚焦时出现，收藏过的常显 */
.block-fav {
  position: absolute;
  top: 2px;
  left: 2px;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 5px;
  background: transparent;
  color: var(--border);
  cursor: pointer;
  opacity: 0;
  /* 不可见时不吃点击：格子左上角仍可点开详情 */
  pointer-events: none;
  transition: opacity 0.12s ease, color 0.12s ease;
}

.block-cell:hover .block-fav,
.block-cell:focus-visible .block-fav {
  opacity: 1;
  pointer-events: auto;
}

.block-fav:hover {
  color: var(--star);
}

.block-fav.is-on {
  opacity: 1;
  pointer-events: auto;
  color: var(--star);
}

.block-fav svg {
  fill: none;
  stroke: currentColor;
}

.block-fav.is-on svg {
  fill: currentColor;
}

.block-cell {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--cell-inner-gap);
  height: var(--cell-h);
  padding: var(--cell-pad);
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  color: var(--text);
  font-family: inherit;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s, transform 0.15s;
}

.block-cell:hover {
  border-color: var(--accent);
  transform: translateY(-2px);
}

/* 键盘焦点与当前详情项各有一种强调，避免混在一起看不出来 */
.block-cell:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.block-cell.on,
.block-cell.multi-checked {
  border-color: var(--accent);
  background: var(--accent-soft);
}

/* 玩家头颅的删除角标（悬停或键盘聚焦时显示）。
   与小图标键同档（20px 方块 + 5px 圆角），但不挂 .input-clear：它不压在输入框里，
   而是常显在格子上的一枚角标，底色与悬停色都是自己的 */
.block-del {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 20px;
  height: 20px;
  display: none;
  align-items: center;
  justify-content: center;
  border-radius: 5px;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 11px;
  line-height: 1;
}

.block-cell:hover .block-del,
.block-cell:focus-visible .block-del {
  display: flex;
}

.block-del:hover {
  background: var(--accent-soft);
  color: var(--accent);
}

.block-img {
  width: var(--cell-img);
  height: var(--cell-img);
  border-radius: 8px;
  /* 不要 image-rendering: pixelated：这里的图标是内核渲染好的 256×256 PNG，
     即 mml-tex-draw 的 BLOCK_SIZE，显示时被缩到 40~88px，最近邻采样会把斜边采成
     锯齿、细纹理采丢。pixelated 只适合像素画贴图（如玩家头颅），缩放渲染图要用平滑插值 */
}

.block-name {
  width: 100%;
  font-size: var(--cell-font);
  color: var(--text);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.grid-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding-top: 48px;
}

.empty-clear {
  height: var(--ctl-h);
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: var(--bg-raised);
  color: var(--accent);
  font-size: 12.5px;
  font-family: inherit;
  cursor: pointer;
}

.empty-clear:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}
</style>
