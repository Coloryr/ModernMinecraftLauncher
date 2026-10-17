<script setup lang="ts">
// 主窗口侧栏：搜索 / 分组列表（拖拽排序）/ 平铺
// 状态与交互全部通过 props / emits 与 MainWindow 联通；
// 分组 / 平铺 / 列表的切换入口不在这里，在设置窗口「界面 → 主窗口」
import DragGhost from "../../../components/ui/DragGhost.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import InstanceIcon from "../../../components/InstanceIcon.vue";
import { t } from "../../../lib/i18n";
import type { InstanceInfoDto } from "../../../lib/bindings";
import type { ViewMode } from "../../../lib/bindings";
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
  onInstClick: (inst: InstanceInfoDto) => void;
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
}>();

function onSearchInput(e: Event) {
  emit("update:searchText", (e.target as HTMLInputElement).value);
}

/** 分组的实例列表是否展开：拖拽实例 / 搜索时强制展开，便于投放与查看结果 */
function isGroupOpen(id: string) {
  return (
    !props.isCollapsed(id) || props.searching || props.dragActive?.kind === "instance"
  );
}
</script>

<template>
  <aside class="sidebar">
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

      <button class="icon-btn" v-tip="t('sidebar.collapse')" @click="emit('collapse')">
        <GlyphIcon name="chevron-left" :size="16" />
      </button>
    </div>

    <!-- 分组：可收缩，组内顶部有添加 -->
    <div v-if="mode === 'group'" class="group-list">
      <template v-for="(g, gi) in filteredGroups" :key="g.id">
        <!-- 分组拖拽插入占位 -->
        <div v-if="isGroupInsert(gi)" class="drop-ghost-group">
          <span class="drop-ghost-group-name">{{ dragActive?.groupName }}</span>
        </div>
        <div class="group-block" :data-group="g.id">
          <div class="group-title-row" @contextmenu.prevent="onGroupContext($event, g.id)"
            @pointerdown="onDragPointerDown($event, { kind: 'group', groupId: g.id, groupName: g.name })">
            <button class="group-title" v-tip="t('group.collapse')" @click="onGroupTitleClick(g.id)">
              <svg class="group-chevron" :class="{ collapsed: isCollapsed(g.id) }" viewBox="0 0 24 24" width="13"
                height="13" fill="none" stroke="currentColor" stroke-width="2">
                <path d="m6 9 6 6 6-6" />
              </svg>
              <span><HighlightText :text="g.name" :query="searchText" /></span>
              <span class="group-count">{{ g.items.length }}</span>
            </button>
          </div>

          <!-- 拖拽实例时自动展开，便于投放；展开 / 收起走高度动画（grid-template-rows 0fr↔1fr） -->
          <div class="group-items-wrap" :class="{ collapsed: !isGroupOpen(g.id) }">
            <div class="group-items">
              <!-- 添加实例（与实例行同尺寸；多选时隐藏，拖拽实例时禁用并变色） -->
              <button v-show="!multiSelect" class="add-inst-row" :disabled="dragActive?.kind === 'instance'"
                @click="emit('add-instance')">
                <span class="add-inst-icon">
                  <GlyphIcon name="plus" :size="18" />
                </span>
                <span class="add-inst-text">{{ t("list.add") }}</span>
              </button>
              <template v-for="(inst, idx) in g.items" :key="inst.uuid">
                <!-- 正在拖拽的实例行不渲染（位置由插入占位显示），避免出现空位 / 双占位 -->
                <template v-if="inst.uuid !== draggingUuid">
                  <!-- 实例拖拽插入占位（淡化的实例，位于鼠标对应位置） -->
                  <div v-if="isInstInsert(g.id, idx)" class="drop-ghost-row">
                    <InstanceIcon :name="dragActive?.instance?.name ?? ''" :uuid="dragActive?.instance?.uuid ?? '0'"
                      :size="38" />
                    <span class="inst-name">{{ dragActive?.instance?.name ?? "" }}</span>
                  </div>
                  <div class="inst-row" :data-uuid="inst.uuid" :class="{
                    active: selected?.uuid === inst.uuid,
                    'multi-checked': multiSelect && selectedIds.has(inst.uuid),
                  }" @pointerdown="onDragPointerDown($event, { kind: 'instance', instance: inst })"
                    @click="onInstClick(inst)" @contextmenu.prevent="onInstContext($event, inst)">
                    <span v-if="multiSelect" class="row-check" :class="{ on: selectedIds.has(inst.uuid) }">
                      <GlyphIcon name="check" :size="12" :weight="2.6" />
                    </span>
                    <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="38" />
                    <span v-if="inst.loader !== 'normal'" class="loader-text" :class="`loader-${inst.loader}`">{{
                      t(`add.loader.${inst.loader}`) }}</span>
                    <span class="inst-name"><HighlightText :text="inst.name" :query="searchText" /></span>
                    <span v-if="inst.running" class="run-dot" v-tip="'running'"></span>
                  </div>
                </template>
              </template>
              <!-- 分组末尾插入占位 -->
              <div v-if="isInstInsertEnd(g.id)" class="drop-ghost-row">
                <InstanceIcon :name="dragActive?.instance?.name ?? ''" :uuid="dragActive?.instance?.uuid ?? '0'"
                  :size="38" />
                <span class="inst-name">{{ dragActive?.instance?.name ?? "" }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>
      <!-- 分组列表末尾插入占位 -->
      <div v-if="isGroupInsert(filteredGroups.length)" class="drop-ghost-group">
        <span class="drop-ghost-group-name">{{ dragActive?.groupName }}</span>
      </div>
      <!-- 跟手浮起副本：被抓住的实例 / 分组，跟着指针走 -->
      <DragGhost v-if="ghost" :x="ghost.x" :y="ghost.y">
        <InstanceIcon v-if="ghost.item.kind === 'instance'" :name="ghost.item.instance?.name ?? ''"
          :uuid="ghost.item.instance?.uuid ?? '0'" :size="32" />
        <GlyphIcon v-else name="folder" :size="16" />
        <span>
          {{ ghost.item.kind === "instance" ? (ghost.item.instance?.name ?? "") : (ghost.item.groupName ?? "") }}
        </span>
      </DragGhost>

      <div v-if="groups.length === 0" class="empty-tip">{{ t("list.empty") }}</div>
      <div v-else-if="filteredGroups.length === 0" class="empty-tip">{{ t("search.empty") }}</div>

      <!-- 添加分组（虚线行，与组内添加实例一致） -->
      <button class="add-inst-row add-group-row" :disabled="dragActive?.kind === 'instance'" @click="emit('add-group')">
        <span class="add-inst-icon">
          <GlyphIcon name="plus" :size="18" />
        </span>
        <span>{{ t("group.addGroup") }}</span>
      </button>
    </div>

    <!-- 平铺：首格为添加 -->
    <div v-else class="tile-list">
      <div v-show="!multiSelect" class="tile add-tile" @click="emit('add-instance')">
        <span class="add-plus">
          <GlyphIcon name="plus" :size="24" :weight="1.8" />
        </span>
        <span class="tile-name">{{ t("group.add") }}</span>
      </div>
      <div v-for="inst in filteredInstances" :key="inst.uuid" class="tile" :class="{
        active: selected?.uuid === inst.uuid,
        'multi-checked': multiSelect && selectedIds.has(inst.uuid),
      }" @click="onInstClick(inst)" @contextmenu.prevent="onInstContext($event, inst)">
        <span v-if="multiSelect" class="row-check" :class="{ on: selectedIds.has(inst.uuid) }">
          <GlyphIcon name="check" :size="12" :weight="2.6" />
        </span>
        <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="44" />
        <span v-if="inst.loader !== 'normal'" class="loader-corner loader-text" :class="`loader-${inst.loader}`">{{
          t(`add.loader.${inst.loader}`) }}</span>
        <span class="tile-name"><HighlightText :text="inst.name" :query="searchText" /></span>
        <span v-if="inst.running" class="run-dot" v-tip="'running'"></span>
      </div>
      <div v-if="filteredInstances.length === 0" class="empty-tip tile-empty">{{ t("search.empty") }}</div>
    </div>
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
  /* 侧栏一行的高度：实例行、拖拽占位、底部的「添加实例 / 添加分组」都取这一档，
     全列节奏才一致（原来 48 / 50 / 50 三处各写一个数） */
  --row-h: 48px;
}

.main.side-right .sidebar {
  border-right: none;
  border-left: 1px solid var(--border);
}

/* 搜索框 + 收起按钮同一行：行内间距由这里给，搜索框自己不留外边距 */
.sidebar-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 12px 10px;
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

.icon-btn:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}

/* ----- 分组模式列表 ----- */

.group-list {
  flex: 1;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  padding: 2px 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 2px;
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

/* 分组内容的收起动画：grid-template-rows 0fr↔1fr 可以在不知道内容高度的前提下过渡高度 */
.group-items-wrap {
  display: grid;
  grid-template-rows: 1fr;
  transition: grid-template-rows 0.22s ease;
}

.group-items-wrap.collapsed {
  grid-template-rows: 0fr;
}

.group-items {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 0;
  /* 收起时裁掉内容（配合 0fr 行高），同时给出过渡的最小高度基准 */
  overflow: hidden;
  min-height: 0;
}

/* 添加实例行（与实例行同尺寸，位于实例上方） */
.add-inst-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 12px;
  border: 1px dashed var(--accent-border);
  border-radius: 10px;
  background: var(--bg-raised);
  color: var(--accent);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.12s;
  min-height: var(--row-h);
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

/* 添加分组行（列表底部）：绿色系，与「添加实例」的主题色区分 */
.add-group-row {
  margin-top: 6px;
  border-color: color-mix(in srgb, var(--green) 42%, transparent);
  color: var(--green);
}

.add-group-row:not(:disabled) .add-inst-icon {
  background: color-mix(in srgb, var(--green) 14%, transparent);
  color: var(--green);
}

.add-group-row:hover:not(:disabled) {
  border-color: var(--green);
  color: var(--green);
  background: color-mix(in srgb, var(--green) 12%, transparent);
}

.add-group-row:hover:not(:disabled) .add-inst-icon {
  background: color-mix(in srgb, var(--green) 18%, transparent);
  color: var(--green);
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
  min-height: var(--row-h);
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
  padding: 9px 12px;
  min-height: var(--row-h);
  border-radius: 10px;
  border: 1.5px dashed var(--accent-border);
  background: var(--accent-soft);
  color: var(--text-dim);
  font-size: 13px;
  opacity: 0.75;
  pointer-events: none;
}

.drop-ghost-group {
  margin: 2px 0;
  padding: 9px 12px;
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

/* ----- 平铺模式网格 ----- */

.tile-list {
  flex: 1;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  padding: 6px 10px 14px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: 10px;
  align-content: start;
}

.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
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

/* 4 种加载器各一色：蓝 / 黄 / 紫 / 青（色相彼此拉开，一眼能分）。
   黄是浅底 → 配深字；其余三个深底配白字。
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
</style>
<style scoped src="../../../styles/parts/instance-check.css"></style>
