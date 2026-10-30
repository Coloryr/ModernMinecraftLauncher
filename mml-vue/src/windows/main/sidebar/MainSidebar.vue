<script setup lang="ts">
// 主窗口侧栏：搜索 / 分组列表（拖拽排序）/ 平铺
// 状态与交互全部通过 props / emits 与 MainWindow 联通；
// 分组 / 平铺 / 列表的切换入口不在这里，在设置窗口「界面 → 主窗口」
//
// 两种模式的实例列表都走虚拟滚动（components/ui/VirtualList）：实例上百时只渲染视口附近
// 那几行。行高按行种钉死（分组标题 / 添加 / 实例 / 平铺卡片各一档，见 ROW_H_*），窗口与
// 滚动条长度都按常量算，不用渲染出来再量。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import DragGhost from "../../../components/ui/DragGhost.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import VirtualList from "../../../components/ui/VirtualList.vue";
import InstanceIcon from "../../../components/InstanceIcon.vue";
import { t } from "../../../lib/i18n";
import type { InstanceInfoDto, ViewMode } from "../../../lib/bindings";
import type { DragCandidate, GroupView } from "../types";

const props = defineProps<{
  /** 当前显示模式：group 分组列表 / grid 平铺 */
  mode: ViewMode;
  /** 搜索框当前文本，受控 */
  searchText: string;
  /** 全部分组，含空分组；用来区分"一个实例都没有"与"搜索没结果"两种空态 */
  groups: GroupView[];
  /** 过滤后的分组，分组列表实际渲染的就是它 */
  filteredGroups: GroupView[];
  /** 平铺模式渲染的实例列表 */
  filteredInstances: InstanceInfoDto[];
  /** 是否正在搜索；搜索时分组强制展开，便于查看命中结果 */
  searching: boolean;
  /** 当前选中实例 */
  selected: InstanceInfoDto | null;
  /** 是否处于多选模式 */
  multiSelect: boolean;
  /** 多选模式下已勾选的实例 uuid */
  selectedIds: Set<string>;
  /** 本次拖拽的候选；null 表示没在拖 */
  dragActive: DragCandidate | null;
  /** 正在拖的实例 uuid；那一行不渲染，位置交给插入占位 */
  draggingUuid: string | null;
  /** 跟手浮起副本（被抓住的实例 / 分组 + 指针位置）；null 表示没在拖 */
  ghost: { item: DragCandidate; x: number; y: number } | null;
  /** 分组是否收起 */
  isCollapsed: (id: string) => boolean;
  /** 实例行或分组标题按下，交给父组件起拖拽 */
  onDragPointerDown: (e: PointerEvent, cand: DragCandidate) => void;
  /** 点击实例行 */
  onInstClick: (inst: InstanceInfoDto, event?: MouseEvent) => void;
  /** 右键实例行 */
  onInstContext: (e: MouseEvent, inst: InstanceInfoDto) => void;
  /** 右键分组标题 */
  onGroupContext: (e: MouseEvent, groupId: string) => void;
  /** 点击分组标题，切换收起 / 展开 */
  onGroupTitleClick: (id: string) => void;
  /** 组内该下标处是否显示实例插入占位 */
  isInstInsert: (groupId: string, idx: number) => boolean;
  /** 分组末尾是否显示实例插入占位 */
  isInstInsertEnd: (groupId: string) => boolean;
  /** 该下标处是否显示分组插入占位 */
  isGroupInsert: (index: number) => boolean;
  /** 侧栏是否已经向右占满，右侧详情区收起 */
  maximized?: boolean;
}>();

/** 侧栏动作，一律抛给主窗口执行 */
const emit = defineEmits<{
  /** 搜索框文本变化 */
  (e: "update:searchText", v: string): void;
  /** 添加实例 */
  (e: "add-instance"): void;
  /** 添加分组 */
  (e: "add-group"): void;
  /** 收起整个侧栏 */
  (e: "collapse"): void;
  /** 向右占满 / 收回 */
  (e: "toggle-maximize"): void;
}>();

/** 搜索框输入：把最新文本抛给父组件的 `v-model` */
function onSearchInput(e: Event) {
  emit("update:searchText", (e.target as HTMLInputElement).value);
}

/** 分组的实例列表是否展开：拖拽实例 / 搜索 / 多选时强制展开，便于投放与查看结果 */
function isGroupOpen(id: string) {
  return (
    !props.isCollapsed(id) ||
    props.searching ||
    props.multiSelect ||
    props.dragActive?.kind === "instance"
  );
}

// ---------------- 虚拟列表的行表 ----------------

/** 分组标题行高度，单位 px；与 CSS 的 .vrow-group 同源 */
const ROW_H_GROUP = 48;
/** 实例行 / 添加行 / 实例插入占位的高度，单位 px；与 CSS 的 .vrow-inst 等同源 */
const ROW_H_INST = 60;
/** 平铺卡片高度，单位 px；与 CSS 的 .tile height 同源 */
const TILE_H = 112;
/** 平铺的行间距，单位 px；与 .vrow-tiles 里多出来的那截同源 */
const TILE_GAP = 8;
/** 一行平铺卡片的高度，单位 px */
const ROW_H_TILE = TILE_H + TILE_GAP;
/** 平铺卡片的最小宽度，单位 px；未占满时与 CSS 原来的 minmax(96px, 1fr) 同源 */
const TILE_MIN = 96;
/** 占满时的最小宽度，单位 px；与原来的 minmax(120px, 1fr) 同源 */
const TILE_MIN_MAX = 120;

/** 平铺模式的一格：添加卡或实例卡 */
type TileCell = { kind: "add" } | { kind: "inst"; inst: InstanceInfoDto };

/** 虚拟列表的一行 */
type Row =
  | { kind: "group"; key: string; gi: number; group: GroupView }
  | { kind: "ghost-group"; key: string; gi: number }
  | { kind: "add"; key: string; gi: number; groupId: string }
  | { kind: "inst"; key: string; gi: number; group: GroupView; idx: number; inst: InstanceInfoDto }
  | { kind: "ghost-inst"; key: string; gi: number; groupId: string }
  | { kind: "tiles"; key: string; gi: number; cells: TileCell[] };

/** 滚动容器（VirtualList 的根），量宽度与按行滚动都通过它 */
const listRef = ref<{
  el: HTMLElement | null;
  scrollToIndex: (index: number, align?: "nearest" | "center") => void;
} | null>(null);

/** 滚动容器去掉左右内边距后的宽度，单位 px；平铺的列数按它算 */
const contentWidth = ref(0);

/** 平铺卡片的最小宽度，单位 px：占满时放宽一档，一排不至于全是小方块 */
const tileMin = computed(() => (props.maximized ? TILE_MIN_MAX : TILE_MIN));

/** 平铺每行的列数：与 CSS 的 auto-fill 同一口径（宽度减去最小宽度，按间距取整） */
const columns = computed(() => {
  const w = contentWidth.value;
  return Math.max(1, Math.floor((w + TILE_GAP) / (tileMin.value + TILE_GAP)));
});

/** 平铺一行的网格样式：列数按容器宽度现算，与脚本里切行的口径一致 */
const tileGridStyle = computed(() => ({
  gridTemplateColumns: "repeat(" + columns.value + ", minmax(0, 1fr))",
  gap: TILE_GAP + "px",
}));

/** 量一次滚动容器的内容宽度 */
function measureWidth() {
  const el = listRef.value?.el ?? null;
  if (!el) return;
  const cs = getComputedStyle(el);
  const pad = (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0);
  contentWidth.value = Math.max(0, el.clientWidth - pad);
}

const widthObserver = new ResizeObserver(() => measureWidth());

onMounted(() => measureWidth());
onUnmounted(() => widthObserver.disconnect());

// 挂载后滚动容器才出现（两种模式共用同一个实例），盯住它量宽度
watch(
  () => listRef.value?.el ?? null,
  (el) => {
    widthObserver.disconnect();
    if (!el) return;
    widthObserver.observe(el);
    measureWidth();
  },
  { flush: "post" },
);

/**
 * 把一串格子按列数切成若干行
 *
 * - `cells`: 这一组（或整个平铺列表）的格子
 * - `key`: 行的 key 前缀，分组 uuid 或 grid
 * - `gi`: 分组下标；平铺模式给 -1
 *
 * # 返回值
 *
 * 见 [`Row`] 里的 tiles 行
 */
function chunkTiles(cells: TileCell[], key: string, gi: number): Row[] {
  const out: Row[] = [];
  const cols = Math.max(1, columns.value);
  for (let i = 0; i < cells.length; i += cols) {
    out.push({ kind: "tiles", key: "t-" + key + "-" + i, gi, cells: cells.slice(i, i + cols) });
  }
  return out;
}

/** 平铺模式的行表：添加卡 + 全部实例，按列数切行 */
function gridRows(list: InstanceInfoDto[]): Row[] {
  const cells: TileCell[] = [];
  if (!props.multiSelect) cells.push({ kind: "add" });
  for (const inst of list) cells.push({ kind: "inst", inst });
  return chunkTiles(cells, "grid", -1);
}

/**
 * 分组模式的行表：分组标题 + 添加行 + 实例行
 *
 * 收起的组只留标题行 —— 组内一行都不渲染，滚起来才省事。
 * 占满时组内改用平铺卡片，与平铺模式同一套格子。
 */
const rows = computed<Row[]>(() => {
  if (props.mode === "grid") return gridRows(props.filteredInstances);
  const out: Row[] = [];
  props.filteredGroups.forEach((g, gi) => {
    if (props.isGroupInsert(gi)) out.push({ kind: "ghost-group", key: "gg-" + gi, gi });
    out.push({ kind: "group", key: "g-" + g.id, gi, group: g });
    if (!isGroupOpen(g.id)) return;
    if (props.maximized) {
      const cells: TileCell[] = [];
      if (!props.multiSelect) cells.push({ kind: "add" });
      for (const inst of g.items) cells.push({ kind: "inst", inst });
      out.push(...chunkTiles(cells, g.id, gi));
      return;
    }
    if (!props.multiSelect) out.push({ kind: "add", key: "a-" + g.id, gi, groupId: g.id });
    g.items.forEach((inst, idx) => {
      // 正在拖的实例行不渲染，位置由插入占位显示
      if (inst.uuid === props.draggingUuid) return;
      if (props.isInstInsert(g.id, idx)) {
        out.push({ kind: "ghost-inst", key: "pi-" + g.id + "-" + idx, gi, groupId: g.id });
      }
      out.push({ kind: "inst", key: "i-" + inst.uuid, gi, group: g, idx, inst });
    });
    if (props.isInstInsertEnd(g.id)) {
      out.push({ kind: "ghost-inst", key: "pe-" + g.id, gi, groupId: g.id });
    }
  });
  if (props.isGroupInsert(props.filteredGroups.length)) {
    out.push({ kind: "ghost-group", key: "gg-end", gi: props.filteredGroups.length });
  }
  return out;
});

/** 每行高度，单位 px；顺序与 rows 一致，变高虚拟列表按它算窗口 */
const rowHeights = computed(() =>
  rows.value.map((row) => {
    if (row.kind === "tiles") return ROW_H_TILE;
    if (row.kind === "group" || row.kind === "ghost-group") return ROW_H_GROUP;
    return ROW_H_INST;
  }),
);

/**
 * 平铺卡按下：只有分组模式（占满）支持拖拽，平铺模式的卡片不拖
 *
 * - `e`: 指针按下事件
 * - `inst`: 被按住的实例
 */
function onTilePointerDown(e: PointerEvent, inst: InstanceInfoDto) {
  if (props.mode !== "group") return;
  props.onDragPointerDown(e, { kind: "instance", instance: inst });
}

/**
 * 把某个实例滚进视野
 *
 * 虚拟列表里被藏起来的行没有 DOM，scrollIntoView 找不到，所以按行表下标滚。
 *
 * - `uuid`: 实例 uuid；行表里没有（分组收起 / 搜索过滤掉）时什么都不做
 */
function scrollToInstance(uuid: string) {
  // 占满时实例在平铺卡片行里，行号要按"这一行里有它"来找
  const index = rows.value.findIndex((row) =>
    row.kind === "inst"
      ? row.inst.uuid === uuid
      : row.kind === "tiles" && row.cells.some((c) => c.kind === "inst" && c.inst.uuid === uuid),
  );
  if (index >= 0) listRef.value?.scrollToIndex(index, "center");
}

defineExpose({ scrollToInstance });
</script>

<template>
  <aside class="sidebar" :class="{ 'is-max': maximized }">
    <!-- 搜索框与收起按钮同一行：侧栏顶部只占一行，下面整段留给列表 -->
    <div class="sidebar-head">
      <div class="search-box">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round">
          <circle cx="11" cy="11" r="7" />
          <path d="m20 20-3.5-3.5" />
        </svg>
        <input :value="searchText" class="search-input" :placeholder="t('search.placeholder')" spellcheck="false"
          @input="onSearchInput" />
        <button v-if="searchText" class="input-clear" @click="emit('update:searchText', '')">
          <GlyphIcon name="close" :size="12" :weight="2.4" />
        </button>
      </div>

      <!-- 向右占满：把详情区让出来，实例按多列排开；多选时锁死在这一档 -->
      <button class="icon-btn" :class="{ on: maximized }" :disabled="multiSelect"
        v-tip="multiSelect ? t('sidebar.maximizeLocked') : (maximized ? t('sidebar.restore') : t('sidebar.maximize'))"
        :aria-label="maximized ? t('sidebar.restore') : t('sidebar.maximize')"
        @click="emit('toggle-maximize')">
        <GlyphIcon :name="maximized ? 'restore' : 'maximize'" :size="16" />
      </button>

      <button class="icon-btn" v-tip="t('sidebar.collapse')" @click="emit('collapse')">
        <GlyphIcon name="chevron-left" :size="16" />
      </button>
    </div>

    <!-- 分组 / 平铺共用一张虚拟列表：行表由脚本按模式铺好，这里只按行种画 -->
    <VirtualList ref="listRef" class="sidebar-list" :class="mode === 'group' ? 'group-list' : 'tile-list'"
      :count="rows.length" :heights="rowHeights" :row-gap="0">
      <template #default="{ start, end }">
        <template v-for="row in rows.slice(start, end)" :key="row.key">
          <!-- 分组标题：收起 / 展开只影响行表里有没有组内那几行 -->
          <div v-if="row.kind === 'group'" class="vrow vrow-group" :data-group="row.group.id" :data-gi="row.gi">
            <div class="group-title-row" @contextmenu.prevent="onGroupContext($event, row.group.id)"
              @pointerdown="onDragPointerDown($event, { kind: 'group', groupId: row.group.id, groupName: row.group.name })">
              <button class="group-title" v-tip="t('group.collapse')" @click="onGroupTitleClick(row.group.id)">
                <svg class="group-chevron" :class="{ collapsed: isCollapsed(row.group.id) }" viewBox="0 0 24 24"
                  width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="m6 9 6 6 6-6" />
                </svg>
                <span><HighlightText :text="row.group.name" :query="searchText" /></span>
                <span class="group-count">{{ row.group.items.length }}</span>
              </button>
            </div>
          </div>

          <!-- 分组拖拽插入占位 -->
          <div v-else-if="row.kind === 'ghost-group'" class="vrow vrow-group">
            <div class="drop-ghost-group">
              <span class="drop-ghost-group-name">{{ dragActive?.groupName }}</span>
            </div>
          </div>

          <!-- 添加实例：与实例行同尺寸，多选时整行不进行表 -->
          <div v-else-if="row.kind === 'add'" class="vrow vrow-add" :data-group="row.groupId">
            <button class="add-inst-row" :disabled="dragActive?.kind === 'instance'" @click="emit('add-instance')">
              <span class="add-inst-icon">
                <GlyphIcon name="plus" :size="18" />
              </span>
              <span class="add-inst-text">{{ t("list.add") }}</span>
            </button>
          </div>

          <!-- 实例拖拽插入占位（淡化的实例，位于鼠标对应位置） -->
          <div v-else-if="row.kind === 'ghost-inst'" class="vrow vrow-ghost-inst" :data-group="row.groupId">
            <div class="drop-ghost-row">
              <InstanceIcon :name="dragActive?.instance?.name ?? ''" :uuid="dragActive?.instance?.uuid ?? '0'"
                :size="38" />
              <span class="inst-name">{{ dragActive?.instance?.name ?? "" }}</span>
            </div>
          </div>

          <!-- 平铺卡片行：占满时与平铺模式同一套卡，外观、角标、运行点位置都一致 -->
          <div v-else-if="row.kind === 'tiles'" class="vrow vrow-tiles">
            <div class="tile-grid" :style="tileGridStyle">
              <template v-for="(cell, ci) in row.cells" :key="ci">
                <div v-if="cell.kind === 'add'" class="tile add-tile" @click="emit('add-instance')">
                  <span class="add-plus">
                    <GlyphIcon name="plus" :size="24" :weight="1.8" />
                  </span>
                  <span class="tile-name">{{ t("group.add") }}</span>
                </div>
                <div v-else class="tile" :data-uuid="cell.inst.uuid" :class="{
                  active: selected?.uuid === cell.inst.uuid,
                  'multi-checked': multiSelect && selectedIds.has(cell.inst.uuid),
                }" @pointerdown="onTilePointerDown($event, cell.inst)" @click="onInstClick(cell.inst, $event)"
                  @contextmenu.prevent="onInstContext($event, cell.inst)">
                  <span v-if="multiSelect" class="row-check" :class="{ on: selectedIds.has(cell.inst.uuid) }">
                    <GlyphIcon name="check" :size="12" :weight="2.6" />
                  </span>
                  <InstanceIcon :name="cell.inst.name" :uuid="cell.inst.uuid" :size="44" />
                  <span v-if="cell.inst.loader !== 'normal'" class="loader-corner loader-text"
                    :class="`loader-${cell.inst.loader}`">{{ t(`add.loader.${cell.inst.loader}`) }}</span>
                  <span class="tile-name"><HighlightText :text="cell.inst.name" :query="searchText" /></span>
                  <span v-if="cell.inst.running" class="run-dot" v-tip="t('launch.running')"></span>
                </div>
              </template>
            </div>
          </div>

          <!-- 实例行 -->
          <div v-else class="vrow vrow-inst" :data-group="row.group.id">
            <div class="inst-row" :data-uuid="row.inst.uuid" :data-idx="row.idx" :data-group="row.group.id" :class="{
              active: selected?.uuid === row.inst.uuid,
              'multi-checked': multiSelect && selectedIds.has(row.inst.uuid),
              running: row.inst.running,
            }" @pointerdown="onDragPointerDown($event, { kind: 'instance', instance: row.inst })"
              @click="onInstClick(row.inst, $event)" @contextmenu.prevent="onInstContext($event, row.inst)">
              <span v-if="multiSelect" class="row-check" :class="{ on: selectedIds.has(row.inst.uuid) }">
                <GlyphIcon name="check" :size="12" :weight="2.6" />
              </span>
              <InstanceIcon :name="row.inst.name" :uuid="row.inst.uuid" :size="38" />
              <span v-if="row.inst.loader !== 'normal'" class="loader-text"
                :class="`loader-${row.inst.loader}`">{{ t(`add.loader.${row.inst.loader}`) }}</span>
              <span class="inst-name"><HighlightText :text="row.inst.name" :query="searchText" /></span>
              <span v-if="row.inst.running" class="run-dot" v-tip="t('launch.running')"></span>
            </div>
          </div>
        </template>

        <!-- 空态与「添加分组」跟在最后一行后面，不参与窗口计算 -->
        <template v-if="mode === 'group'">
          <div v-if="groups.length === 0" class="empty-tip">{{ t("list.empty") }}</div>
          <div v-else-if="filteredGroups.length === 0" class="empty-tip">{{ t("search.empty") }}</div>
          <button class="add-row-btn add-group-row" :disabled="dragActive?.kind === 'instance'"
            @click="emit('add-group')">
            <GlyphIcon name="plus" :size="14" />
            <span>{{ t("group.addGroup") }}</span>
          </button>
        </template>
        <div v-else-if="filteredInstances.length === 0" class="empty-tip tile-empty">{{ t("search.empty") }}</div>
      </template>
    </VirtualList>

    <!-- 跟手浮起副本：被抓住的实例 / 分组，跟着指针走 -->
    <DragGhost v-if="ghost" :x="ghost.x" :y="ghost.y">
      <InstanceIcon v-if="ghost.item.kind === 'instance'" :name="ghost.item.instance?.name ?? ''"
        :uuid="ghost.item.instance?.uuid ?? '0'" :size="32" />
      <GlyphIcon v-else name="folder" :size="16" />
      <span>
        {{ ghost.item.kind === "instance" ? (ghost.item.instance?.name ?? "") : (ghost.item.groupName ?? "") }}
      </span>
    </DragGhost>

  </aside>
</template>

<style scoped>
.sidebar {
  width: 300px;
  min-width: 300px;
  background: var(--bg-side);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  /* 侧栏左右内边距：搜索行、分组列表、平铺网格共用一处 ——
     三处各写一个数时搜索框 / 收起按钮与下面的实例对不齐。
     取值以顶部那一行为准（收起按钮到右边的距离） */
  --side-pad-x: 12px;
}

/* 占满：宽度跟着槽位走，两侧边框都不要 —— 这时它已经铺到窗口边缘 */
.sidebar.is-max {
  width: 100%;
  min-width: 0;
  border-right: none;
  border-left: none;
}

.main.side-right .sidebar {
  border-right: none;
  border-left: 1px solid var(--border);
}

.main.side-right .sidebar.is-max {
  border-left: none;
}

/* 搜索框 + 收起按钮同一行：行内间距由这里给，搜索框自己不留外边距 */
.sidebar-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px var(--side-pad-x) 10px;
}

/* 实例搜索框：吃掉按钮之外的那一段宽度 */
.search-box {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  height: var(--ctl-h);
  border-radius: var(--ctl-radius);
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-dim);
}

/* 与 .field-input 同一套反馈：悬停淡淡一档 */
.search-box:hover {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.search-box:focus-within {
  border-color: var(--accent);
}

.search-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  outline: none;
  font-family: inherit;
}

.search-input::placeholder {
  color: var(--text-dim);
}

.icon-btn {
  width: var(--ctl-h);
  height: var(--ctl-h);
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}

.icon-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  border-color: var(--accent);
}

/* 占满态：这一枚是"点一下就收回"，用强调色标出当前状态 */
.icon-btn.on {
  border-color: var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent);
}

.icon-btn:disabled {
  opacity: 0.45;
  cursor: default;
}

/* ----- 虚拟列表（分组 / 平铺共用） ----- */

/* 滚动与"只渲染视口附近那几行"由 VirtualList 负责，这里只管左右内边距 */
.sidebar-list {
  flex: 1;
  scrollbar-gutter: stable;
  padding: 2px var(--side-pad-x) 14px;
  /* 与 scrollbar-gutter 成对：减掉常驻的那 9px，行才是左右各 10px，见 styles/scrollbar.css */
  padding-right: calc(var(--side-pad-x) - 9px);
}

.tile-list {
  padding-top: 6px;
}

/* 每种虚拟行的高度都钉死：窗口与滚动条长度按这些常量算，改这里要同时改脚本的 ROW_H_* */
.vrow {
  flex-shrink: 0;
}

/* 分组标题行：标题上下各留一点空，分组之间才分得开 */
.vrow-group {
  height: 48px;
  display: flex;
  align-items: center;
}

/* 实例行 / 添加行 / 实例插入占位：56 的行身 + 4 的行距 */
.vrow-inst,
.vrow-add,
.vrow-ghost-inst {
  height: 60px;
}

.vrow-inst .inst-row,
.vrow-add .add-inst-row,
.vrow-ghost-inst .drop-ghost-row {
  height: 56px;
}

/* 平铺卡片行：112 的卡 + 8 的行距 */
.vrow-tiles {
  height: 120px;
}

.tile-grid {
  display: grid;
  align-content: start;
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.group-title {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 700;
  color: var(--text-dim);
  padding: 8px 6px;
  border: none;
  background: transparent;
  cursor: pointer;
  letter-spacing: 0.5px;
  font-family: inherit;
  border-radius: 8px;
  transition: background 0.12s;
  text-align: left;
}

.group-title:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.group-chevron {
  transition: transform 0.15s;
  flex-shrink: 0;
}

.group-chevron.collapsed {
  transform: rotate(-90deg);
}

.group-count {
  font-size: 11px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 20px;
  padding: 0 8px;
  color: var(--text-dim);
  font-weight: 500;
  margin-left: auto;
}

/* 添加实例行（与实例行同尺寸，位于实例上方）。
   边框也占布局位置：内边距减掉那 1px，内容左右 / 上下才和**不带边框**的实例行对齐 */
.add-inst-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 11px;
  border: 1px dashed var(--accent-border);
  border-radius: 10px;
  background: var(--bg-raised);
  color: var(--accent);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.12s;
}

/* 默认态就用主题色，和绿色系的「添加分组」区分开 */
.add-inst-row:not(:disabled) .add-inst-icon {
  background: var(--accent-soft);
  color: var(--accent);
}

.add-inst-row:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-soft);
}

/* 拖拽实例时禁用：变灰、禁止光标 */
.add-inst-row:disabled {
  cursor: not-allowed;
  border-color: var(--border);
  color: var(--text-dim);
  background: var(--bg);
  opacity: 0.55;
}

.add-inst-row:disabled .add-inst-icon {
  background: var(--bg-hover);
  color: var(--text-dim);
}

/* 添加分组行（列表底部）：外观走全局 .add-row-btn，这里只管与上方实例列表的间距 */
.add-group-row {
  margin-top: 6px;
}

.add-inst-icon {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  flex-shrink: 0;
}

.add-inst-row:hover:not(:disabled) .add-inst-icon {
  background: var(--accent-soft);
}

.inst-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 10px;
  cursor: grab;
  transition: background 0.12s;
}

.inst-row:active {
  cursor: grabbing;
}

/* 拖拽插入占位（淡化的实例 / 分组）。
   pointer-events: none 是必须的：落点靠 elementFromPoint 命中，占位自己接事件就选不中底下的行 */
.drop-ghost-row {
  display: flex;
  align-items: center;
  gap: 10px;
  /* 同 .add-inst-row：内边距减掉边框那 1.5px，占位与它替换的实例行逐像素对齐
     （否则拖动时整列会上下跳一下） */
  padding: 7.5px 10.5px;
  border-radius: 10px;
  border: 1.5px dashed var(--accent-border);
  background: var(--accent-soft);
  color: var(--text-dim);
  font-size: 13px;
  opacity: 0.75;
  pointer-events: none;
}

.drop-ghost-group {
  /* 同上：减掉边框宽度，分组名与实例行的内容左缘对齐 */
  padding: 7.5px 10.5px;
  border: 1.5px dashed var(--accent-border);
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 700;
  opacity: 0.75;
  pointer-events: none;
}

.inst-row:hover {
  background: var(--bg-hover);
}

.inst-row.active {
  background: var(--accent-soft);
  outline: 1px solid var(--accent-border);
}

.inst-name {
  font-size: 13.5px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

/* ----- 平铺卡片 ----- */

/* 高度钉死：一行平铺卡片的高度按它算（脚本的 TILE_H），名字一两行都占同样高 */
.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  height: 112px;
  padding: 16px 8px 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.15s;
}

.tile:hover {
  background: var(--bg-hover);
  transform: translateY(-2px);
}

.tile.active {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.tile.add-tile {
  border: 1px solid var(--border);
  background: var(--bg-card);
  justify-content: center;
}

.tile.add-tile:hover {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-soft);
}

.add-plus {
  font-size: 26px;
  line-height: 1;
  color: var(--text-dim);
}

.tile.add-tile:hover .add-plus {
  color: var(--accent);
}

.add-tile .tile-name {
  color: var(--accent);
  font-weight: 600;
}

.tile-empty {
  grid-column: 1 / -1;
}

.loader-corner {
  position: absolute;
  top: 8px;
  left: 8px;
}

/* 多选勾选圈也钉在左上角：加载器角标往右让一格，两个不叠在一起 */
.tile .row-check ~ .loader-corner {
  left: 30px;
}

.tile-name {
  font-size: 12px;
  text-align: center;
  line-height: 1.3;
  word-break: break-all;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.tile .run-dot {
  position: absolute;
  top: 10px;
  right: 10px;
}

/* 运行中的绿点：贴行右缘并**脱离文档流**。
   留在流里它会占一格（名字可用宽度跟着缩），行高也随它参与计算 */
.inst-row.running .run-dot {
  position: absolute;
  top: 50%;
  right: 12px;
  transform: translateY(-50%);
}

/* 给绿点让出位置：长名字的省略号不会压到它 */
.inst-row.running .inst-name {
  padding-right: 17px;
}

.run-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--green);
  box-shadow: 0 0 6px var(--green);
  flex-shrink: 0;
}

/* 多选勾选 */
/* outline 画在元素外沿，会被分组内容容器的 overflow: hidden（收起动画需要）裁掉，故统一向内收 1px */
.inst-row.active,
.inst-row.multi-checked {
  outline-offset: -1px;
}

.loader-text {
  font-size: 10px;
  padding: 1px 7px;
  border-radius: 8px;
  /* 实心色（同账户窗口的「当前」角标）：选中态用的是**淡**强调色底 --accent-soft，
     角标若也用淡底，平铺模式下就和"已选中"混在一起分不出来了。
     底色由下面的 per-loader 类给；这里是兜底（未知加载器） */
  background: var(--accent);
  color: #fff;
  white-space: nowrap;
  flex-shrink: 0;
  line-height: 1.6;
}

/* 6 种加载器各一色：蓝 / 黄 / 紫 / 青 / 橙 / 玫红（色相彼此拉开，一眼能分）。
   黄是浅底 → 配深字；其余深底配白字。
   类名必须与模板里的 `loader-${inst.loader}` 一致（loader-forge / loader-fabric …） */
.loader-text.loader-forge {
  background: #1d4ed8;
  /* 蓝 */
  color: #fff;
}

.loader-text.loader-fabric {
  background: #a16207;
  /* 黄（压深到能配白字的金褐） */
  color: #fff;
}

.loader-text.loader-quilt {
  background: #7c3aed;
  /* 紫 */
  color: #fff;
}

.loader-text.loader-neoforge {
  background: #0d9488;
  /* 青 */
  color: #fff;
}

.loader-text.loader-cleanroom {
  background: var(--loader-cleanroom);
  /* 橙 */
  color: #fff;
}

.loader-text.loader-liteloader {
  background: var(--loader-liteloader);
  /* 玫红 */
  color: #fff;
}
</style>
<style scoped src="../../../styles/parts/instance-check.css"></style>
