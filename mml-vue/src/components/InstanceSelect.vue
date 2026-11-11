<script setup lang="ts">
// 实例下拉选择：主窗口列表模式与各窗口「挑一个实例」共用这一份
//
// 实例一多，原生 <select> 与只列名字的浮层都不好用：几十行全靠翻。这里统一提供
// 搜索、按分组分节、虚拟滚动；浮层挂到 body，父级是滚动容器也不会被裁。
// 触发按钮两档外观：
// - `card`：主窗口列表模式的大按钮，名字 + 版本 / 加载器
// - `field`：表单与标题栏的一行下拉，外观全走 .field-select，只补对齐与省略
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import VirtualList from "./ui/VirtualList.vue";
import SearchBox from "./ui/SearchBox.vue";
import { t } from "../lib/i18n";
import { api } from "../lib/api";
import { groupInstances, hasInstanceMeta, type InstanceOption } from "../lib/instancePicker";
import type { GroupDto } from "../lib/bindings";

/** 分组标题行高，单位 px；与 .group-head 的 height 同源 */
const GROUP_ROW_H = 26;
/** 只有名字的实例行高，单位 px；与 .option 的 height 同源 */
const ITEM_ROW_H = 36;
/** 带版本 / 加载器副标题的实例行高，单位 px；与 .option.with-meta 的 height 同源 */
const ITEM_META_H = 48;

/** 浮层与窗口边缘至少留这么多，单位 px */
const EDGE = 8;
/** 浮层与触发按钮之间的缝隙，单位 px */
const GAP = 4;
/** 向下弹出至少要有的可用高度，单位 px；比这还窄才改向上 */
const DROP_MIN = 180;

/** 下拉里的一行：分组标题或一个实例 */
interface GroupRow {
  kind: "group";
  /** 行键，带前缀避免与实例 uuid 撞车 */
  key: string;
  /** 分组显示名 */
  name: string;
  /** 组内实例数 */
  count: number;
}

interface ItemRow {
  kind: "item";
  key: string;
  inst: InstanceOption;
}

type Row = GroupRow | ItemRow;

const props = withDefaults(
  defineProps<{
    /** 可选实例；带 `group` 的按分组显示，没有分组信息的保持平铺 */
    instances: InstanceOption[];
    /** 当前选中实例 uuid；空串配合 `emptyLabel` 表示额外那一项（全部 / 不锁定） */
    modelValue: string | null;
    /** 触发按钮外观，见文件头 */
    variant?: "card" | "field";
    /** 禁用：不响应点击 */
    disabled?: boolean;
    /** 没选中时按钮上的占位文案，调用方已 `t()` 好的成品 */
    placeholder?: string;
    /** 列表顶部额外一项的文案，选中它回写空串；不给就没有 */
    emptyLabel?: string;
    /** 浮层最小宽度，单位 px；窄的标题栏下拉也想放得下完整名字 */
    menuMinWidth?: number;
    /** 列表底部「新建实例」入口的文案，点它发 `add`；不给就没有 */
    addLabel?: string;
  }>(),
  {
    variant: "field",
    disabled: false,
    placeholder: "",
    emptyLabel: "",
    menuMinWidth: 0,
    addLabel: "",
  },
);

const emit = defineEmits<{
  /** `v-model` 更新：新选中的实例 uuid，空串表示选了额外那一项 */
  (e: "update:modelValue", uuid: string): void;
  /** 下拉底部的「新建实例」入口，由窗口决定怎么开新建窗口 */
  (e: "add"): void;
}>();

/** 浮层是否展开 */
const open = ref(false);
/** 搜索词；每次打开清空 */
const query = ref("");
/** 分组表；列表里没有分组字段时不拉 */
const groups = ref<GroupDto[]>([]);
/** 正在拉分组表，避免重复请求 */
const loadingGroups = ref(false);
/** 键盘高亮的行下标（rows 的下标） */
const activeIndex = ref(0);
/** 浮层坐标：fixed，挂到 body 后由 place() 现算 */
const menuStyle = ref<Record<string, string>>({});

const rootEl = ref<HTMLDivElement | null>(null);
const btnEl = ref<HTMLButtonElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);
const searchRef = ref<{ focus: () => void } | null>(null);
const listRef = ref<{
  el: HTMLElement | null;
  scrollToIndex: (index: number, align?: "nearest" | "center") => void;
} | null>(null);

/** 当前选中的实例；选的是额外那一项或没选时为 null */
const selected = computed(() => props.instances.find((inst) => inst.uuid === props.modelValue) ?? null);
/** 选中的是「全部 / 不锁定」那一项 */
const emptyPicked = computed(() => !!props.emptyLabel && props.modelValue === "");
/** 关闭态按钮上的文案 */
const label = computed(() => {
  if (emptyPicked.value) return props.emptyLabel;
  return selected.value?.name ?? props.placeholder;
});

/** 分组 uuid → 显示名；默认分组的名字留白，后面按界面语言补 */
const groupNames = computed(() => new Map(groups.value.map((group) => [group.uuid, group.name.trim()])));

/** 默认分组的显示名 */
const defaultGroupName = computed(() => t("group.default"));

/** 按搜索词筛过的实例；分组名也参与匹配，搜组名能把整组带出来 */
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.instances;
  return props.instances.filter((inst) => {
    const id = inst.group && groupNames.value.has(inst.group) ? inst.group : "";
    const groupName = groupNames.value.get(id) || defaultGroupName.value;
    return [inst.name, inst.version ?? "", inst.loader ?? "", groupName].some((text) =>
      text.toLowerCase().includes(q),
    );
  });
});

/** 分节；实例都归不进分组表时只剩一节，标题就不画 */
const sections = computed(() => groupInstances(filtered.value, groups.value, defaultGroupName.value));
/** 只有不止一节时才画分组标题，单组用户看到的仍是原来的平铺列表 */
const showHeaders = computed(() => sections.value.length > 1);

/** 要渲染的行；虚拟列表按这个列表算高度 */
const rows = computed<Row[]>(() => {
  if (!showHeaders.value) {
    return filtered.value.map((inst) => ({ kind: "item", key: inst.uuid, inst }));
  }
  const out: Row[] = [];
  for (const section of sections.value) {
    out.push({ kind: "group", key: `g:${section.id}`, name: section.name, count: section.items.length });
    for (const inst of section.items) out.push({ kind: "item", key: inst.uuid, inst });
  }
  return out;
});

/** 每行高度，单位 px；虚拟列表按它算窗口，与 CSS 里钉死的常量同源 */
const heights = computed(() =>
  rows.value.map((row) =>
    row.kind === "group" ? GROUP_ROW_H : hasInstanceMeta(row.inst) ? ITEM_META_H : ITEM_ROW_H,
  ),
);

/** 下一个可选的实例行（跳过分组标题）；没有就返回 -1 */
function firstItemIndex(from = 0): number {
  for (let index = from; index < rows.value.length; index += 1) {
    if (rows.value[index].kind === "item") return index;
  }
  return -1;
}

/** 拉分组表；列表里没有分组字段就跳过，统计窗口的 StatsInstanceDto 没必要多跑一趟 */
async function loadGroups() {
  if (!props.instances.some((inst) => "group" in inst)) return;
  if (loadingGroups.value) return;
  loadingGroups.value = true;
  try {
    groups.value = await api.getGroups();
  } catch {
    groups.value = [];
  } finally {
    loadingGroups.value = false;
  }
}

/** 把当前值滚进视野；没有选中时落到第一个实例 */
function revealActive() {
  const index = rows.value.findIndex((row) => row.kind === "item" && row.inst.uuid === props.modelValue);
  activeIndex.value = index >= 0 ? index : firstItemIndex();
  if (activeIndex.value >= 0) listRef.value?.scrollToIndex(activeIndex.value, "nearest");
}

/** 键盘上下移动高亮，跳过分组标题 */
function moveActive(delta: number) {
  const total = rows.value.length;
  if (!total) return;
  let index = activeIndex.value;
  for (let step = 0; step < total; step += 1) {
    index = (index + delta + total) % total;
    if (rows.value[index].kind === "item") {
      activeIndex.value = index;
      listRef.value?.scrollToIndex(index, "nearest");
      return;
    }
  }
}

/**
 * 按触发按钮的位置算浮层坐标
 *
 * 先按"不限高"量一次整高。卡片档（主窗口列表模式）**跨过触发按钮**、上下都用：
 * 只放一侧时实例一多就只能在下面挤一条。字段档放在一侧，默认朝下；整高几乎总是超过
 * 任何一侧，若按"放不放得下"比大小，按钮只要比窗口中线低一点就永远向上弹，
 * 所以只有下面窄到放不下一段可用高度、且上面确实更宽时才向上。
 * 左右都夹进窗口内 —— 浮层挂在 body 上，没有父级帮忙裁。
 */
function place() {
  const btn = btnEl.value;
  const menu = menuEl.value;
  if (!btn || !menu) return;
  const rect = btn.getBoundingClientRect();
  const width = Math.min(Math.max(rect.width, props.menuMinWidth), window.innerWidth - EDGE * 2);
  const list = listRef.value?.el ?? null;
  // 被 max-height 裁掉的那截也加回来，才是"不限高时"的整高
  const hidden = list ? Math.max(0, list.scrollHeight - list.clientHeight) : 0;
  const natural = menu.offsetHeight + hidden;
  if (natural <= 0) return;

  let left = rect.left;
  if (left + width > window.innerWidth - EDGE) left = rect.right - width;
  left = Math.max(EDGE, Math.min(left, window.innerWidth - width - EDGE));

  // 卡片档：以按钮中心为锚点上下对半分，盖住触发按钮；列表再长也能占满整窗高度。
  // 贴着窗口上下边缘时把整块夹回来，超高就封顶后由列表自己滚。
  if (props.variant === "card") {
    const maxHeight = Math.max(160, Math.min(natural, window.innerHeight - EDGE * 2));
    const top = Math.max(
      EDGE,
      Math.min(
        rect.top + rect.height / 2 - maxHeight / 2,
        window.innerHeight - EDGE - maxHeight,
      ),
    );
    menuStyle.value = {
      left: `${left}px`,
      top: `${top}px`,
      width: `${width}px`,
      maxHeight: `${maxHeight}px`,
    };
    return;
  }

  const below = window.innerHeight - rect.bottom - GAP - EDGE;
  const above = rect.top - GAP - EDGE;
  // 下面还能露出 DROP_MIN 就不往上翻：整高放不下时向下滚动，比跳到按钮上方更符合预期
  const up = below < DROP_MIN && above > below;
  const maxHeight = Math.max(160, Math.min(natural, up ? above : below));

  menuStyle.value = {
    left: `${left}px`,
    width: `${width}px`,
    maxHeight: `${maxHeight}px`,
    ...(up
      ? { bottom: `${window.innerHeight - rect.top + GAP}px` }
      : { top: `${rect.bottom + GAP}px` }),
  };
}

/**
 * 展开 / 收起
 *
 * 展开后要量两次：第一次让浮层以"不限高"渲染出来量整高，第二次等封顶后的高度
 * 落进 DOM 再滚到选中项 —— 高度没收缩时列表以为整列都看得见，就不会滚。
 */
async function toggle() {
  if (props.disabled) return;
  if (open.value) {
    close();
    return;
  }
  query.value = "";
  void loadGroups();
  const rect = btnEl.value?.getBoundingClientRect();
  if (rect) {
    menuStyle.value = {
      left: `${rect.left}px`,
      top: `${rect.bottom + GAP}px`,
      width: `${Math.max(rect.width, props.menuMinWidth)}px`,
    };
  }
  open.value = true;
  await nextTick();
  place();
  await nextTick();
  revealActive();
  searchRef.value?.focus();
}

/**
 * 收起浮层
 *
 * - `restoreFocus`: 选完 / 按 Esc 时把焦点还给触发按钮；点外面收起时不要抢焦点
 */
function close(restoreFocus = false) {
  if (!open.value) return;
  open.value = false;
  if (restoreFocus) btnEl.value?.focus({ preventScroll: true });
}

/** 选中一个实例 */
function pick(inst: InstanceOption) {
  emit("update:modelValue", inst.uuid);
  close(true);
}

/** 选中「全部 / 不锁定」那一项 */
function pickEmpty() {
  emit("update:modelValue", "");
  close(true);
}

/** 底部的「新建实例」：先收起，再交给窗口处理 */
function addNew() {
  close();
  emit("add");
}

/** 浮层内的键盘：上下选、回车确认、Esc 收起 */
function onMenuKeydown(e: KeyboardEvent) {
  // 浮层盖在弹窗上：这里吞掉的键不能再落到底下的弹窗（否则 Esc 会把弹窗一起关掉）
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    e.stopPropagation();
    moveActive(e.key === "ArrowDown" ? 1 : -1);
    return;
  }
  if (e.key === "Enter") {
    const row = rows.value[activeIndex.value];
    if (row?.kind === "item") {
      e.preventDefault();
      e.stopPropagation();
      pick(row.inst);
    }
    return;
  }
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    close(true);
  }
}

/** 点浮层外面收起；浮层挂在 body 上，根元素与浮层都要判 */
function onDocPointerDown(e: PointerEvent) {
  if (!open.value) return;
  const target = e.target as Node | null;
  if (target && (rootEl.value?.contains(target) || menuEl.value?.contains(target))) return;
  close();
}

/** 页面滚动时收起：fixed 坐标不会跟着内容走；浮层自己内部滚不算 */
function onScroll(e: Event) {
  if (!open.value) return;
  const target = e.target;
  if (target instanceof Node && menuEl.value?.contains(target)) return;
  close();
}

/** 窗口大小变了：重新摆位，不用收起来 */
function onResize() {
  if (open.value) place();
}

onMounted(() => {
  document.addEventListener("pointerdown", onDocPointerDown, true);
  window.addEventListener("scroll", onScroll, true);
  window.addEventListener("resize", onResize);
  void loadGroups();
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown, true);
  window.removeEventListener("scroll", onScroll, true);
  window.removeEventListener("resize", onResize);
});

// 搜索词变了：高亮回到第一个命中项，列表跟着滚回顶部，别停在上一份结果的滚动位置
watch(query, () => {
  if (!open.value) return;
  activeIndex.value = firstItemIndex();
  if (activeIndex.value >= 0) listRef.value?.scrollToIndex(activeIndex.value, "nearest");
});

// 分组表迟到或更新后行会重排：高亮行可能落到分组标题上，挪到下一个实例
watch(rows, () => {
  if (open.value && rows.value[activeIndex.value]?.kind !== "item") {
    activeIndex.value = firstItemIndex();
  }
});

// 打开着被禁用（下载 / 生成开始）：先收起，别留一个点不动的浮层
watch(
  () => props.disabled,
  (disabled) => {
    if (disabled) close();
  },
);
</script>

<template>
  <div ref="rootEl" class="instance-select" :class="`is-${variant}`">
    <!-- 卡片档：主窗口列表模式，名字 + 版本 / 加载器两行 -->
    <button v-if="variant === 'card'" ref="btnEl" type="button" class="select-btn card-btn" :class="{ open }"
      :disabled="disabled" :aria-label="label" aria-haspopup="listbox" :aria-expanded="open" @click="toggle">
      <span class="select-text">
        <span v-if="selected" class="sel-name">{{ selected.name }}</span>
        <span v-else class="sel-placeholder">{{ placeholder || t("launch.selectInstance") }}</span>
        <span v-if="selected && hasInstanceMeta(selected)" class="sel-sub">
          <span v-if="selected.version">{{ selected.version }}</span>
          <span v-if="selected.loader && selected.loader !== 'normal'" class="loader-text">
            {{ t(`add.loader.${selected.loader}`) }}
          </span>
        </span>
      </span>
      <svg class="chevron" :class="{ flip: open }" viewBox="0 0 24 24" width="14" height="14" fill="none"
        stroke="currentColor" stroke-width="2">
        <path d="m6 9 6 6 6-6" />
      </svg>
    </button>

    <!-- 字段档：单行下拉，外观全走 .field-select（底色 / 边框 / 高度 / 箭头） -->
    <button v-else ref="btnEl" type="button" class="select-btn field-select field-btn" :class="{ open }"
      :disabled="disabled" :title="label" :aria-label="label" aria-haspopup="listbox" :aria-expanded="open"
      @click="toggle">
      <span class="field-text" :class="{ 'is-empty': !selected && !emptyPicked }">{{ label }}</span>
    </button>

    <!-- 浮层挂到 body：各窗口的父级多是滚动容器，absolute 会被 overflow 裁掉 -->
    <Teleport to="body">
      <Transition name="drop">
        <div v-if="open" ref="menuEl" class="select-menu" :style="menuStyle" @keydown="onMenuKeydown">
          <div class="menu-search">
            <SearchBox ref="searchRef" v-model="query" width="100%" :placeholder="t('search.placeholder')" />
          </div>

          <!-- 额外一项（全部实例 / 不锁定）：固定在搜索结果之外，搜索时也一直看得见 -->
          <button v-if="emptyLabel" type="button" class="option empty-option" :class="{ 'is-selected': emptyPicked }"
            @click="pickEmpty">
            <span class="option-text">
              <span class="option-name">{{ emptyLabel }}</span>
            </span>
          </button>

          <!-- 实例列表：浮层里唯一可滚动的地方 -->
          <VirtualList ref="listRef" class="menu-list" role="listbox" :count="rows.length" :row-h="ITEM_ROW_H"
            :heights="heights" :row-gap="0">
            <template #default="{ start, end }">
              <template v-for="(row, index) in rows.slice(start, end)" :key="row.key">
                <div v-if="row.kind === 'group'" class="group-head">
                  <span class="group-name">{{ row.name }}</span>
                  <span class="group-count">{{ row.count }}</span>
                </div>
                <button v-else type="button" role="option" :aria-selected="row.inst.uuid === modelValue" class="option"
                  :class="{
                  'with-meta': hasInstanceMeta(row.inst),
                  'is-selected': row.inst.uuid === modelValue,
                  'is-active': activeIndex === start + index,
                  running: row.inst.running,
                }" @mouseenter="activeIndex = start + index" @click="pick(row.inst)">
                  <span class="option-text">
                    <span class="option-name">{{ row.inst.name }}</span>
                    <span v-if="hasInstanceMeta(row.inst)" class="option-sub">
                      <span v-if="row.inst.version">{{ row.inst.version }}</span>
                      <span v-if="row.inst.loader && row.inst.loader !== 'normal'" class="loader-text">
                        {{ t(`add.loader.${row.inst.loader}`) }}
                      </span>
                    </span>
                  </span>
                  <span v-if="row.inst.running" class="run-dot" v-tip="t('launch.running')"></span>
                </button>
              </template>
            </template>
          </VirtualList>

          <div v-if="!rows.length" class="menu-empty">{{ t("search.empty") }}</div>

          <!-- 底部固定：新建实例入口在滚动区之外，实例再多也一直看得见 -->
          <div v-if="addLabel" class="menu-foot">
            <div class="menu-sep"></div>
            <button type="button" class="option add-option" @click="addNew">
              <span class="option-text">
                <span class="option-name">{{ addLabel }}</span>
              </span>
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.instance-select {
  position: relative;
  min-width: 0;
}

.instance-select.is-card {
  width: 100%;
  max-width: 460px;
}

/* 触发按钮：两档共用布局，外观各自写 */
.select-btn {
  display: flex;
  align-items: center;
  gap: 12px;
  font-family: inherit;
  cursor: pointer;
}

/* ---------- 卡片档 ---------- */

.card-btn {
  width: 100%;
  padding: 10px 14px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text);
  transition: all 0.15s;
}

.card-btn:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--bg-hover);
}

.select-text {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-width: 0;
  line-height: 1.3;
  text-align: left;
}

.sel-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.sel-placeholder {
  font-size: 13px;
  color: var(--text-dim);
}

.sel-sub {
  font-size: 11.5px;
  color: var(--text-dim);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.loader-text {
  font-size: 10.5px;
  padding: 0 6px;
  border-radius: 8px;
  background: var(--accent-soft);
  color: var(--accent);
  line-height: 1.6;
}

.chevron {
  color: var(--text-dim);
  transition: transform 0.15s;
  flex-shrink: 0;
}

.chevron.flip {
  transform: rotate(180deg);
}

/* ---------- 字段档 ---------- */

/* 尺寸与外观都来自 .field-select，这里只补 flex 对齐与单行省略 */
.field-btn {
  width: 100%;
  gap: 8px;
  text-align: left;
  overflow: hidden;
  white-space: nowrap;
}

.field-btn.open {
  border-color: var(--accent);
}

.field-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field-text.is-empty {
  color: var(--text-dim);
}

/* ---------- 浮层 ---------- */

.select-menu {
  /* fixed：坐标由 place() 现算；挂到 body 后不受父级 overflow 裁切 */
  position: fixed;
  /* 要盖过模态（200）里的控件，又不压全局提示（300），见 §16.6 的层级表 */
  z-index: 250;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px;
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  /* 自己滚会把滚动条拉到整块浮层的高度，溢出统一交给 .menu-list */
  overflow: hidden;
}

.menu-search {
  flex: none;
  padding: 2px 2px 4px;
}

/* 实例列表：浮层里唯一可滚动的地方 */
.menu-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 滚到底不把滚动交给页面：浮层是 fixed，页面一动它就脱离按钮 */
  overscroll-behavior: contain;
}

/* 分组标题：高度与脚本里的 GROUP_ROW_H 同源 */
.group-head {
  height: 26px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 600;
}

.group-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-count {
  margin-left: auto;
  font-size: 10.5px;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
}

.option {
  position: relative;
  width: 100%;
  /* 高度钉死：虚拟列表按它算窗口（ITEM_ROW_H / ITEM_META_H），文字一两行都占同样高 */
  height: 36px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 10px;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  cursor: pointer;
  font-family: inherit;
  text-align: left;
}

.option.with-meta {
  height: 48px;
}

/* 列表上面的显式固定行：浮层被 max-height 压扁时不让它跟着收缩 */
.empty-option {
  flex: none;
}

.option:hover {
  background: var(--bg-hover);
}

.option.is-active {
  background: var(--bg-hover);
}

.option.is-selected {
  background: var(--accent-soft);
  outline: 1px solid var(--accent-border);
  /* outline 画在元素外沿，而列表是滚动容器：向外那 1px 会被裁掉，向内收 1px */
  outline-offset: -1px;
}

.option-text {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.3;
}

.option-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.option-sub {
  font-size: 11.5px;
  color: var(--text-dim);
  display: flex;
  align-items: center;
  gap: 4px;
}

.option-sub .loader-text {
  font-size: 10.5px;
}

/* 运行中的绿点：贴右缘并脱离文档流，不给选项行带来任何尺寸影响 */
.option.running .run-dot {
  position: absolute;
  top: 50%;
  right: 10px;
  transform: translateY(-50%);
}

/* 给绿点让位：名字 / 副标题的省略号不会压到它 */
.option.running .option-text {
  padding-right: 16px;
}

.run-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--green);
  box-shadow: 0 0 5px var(--green);
  flex-shrink: 0;
}

.menu-empty {
  padding: 18px 10px;
  color: var(--text-dim);
  font-size: 12.5px;
  text-align: center;
}

/* 新建实例入口：与实例项之间用分隔线隔开 */
.menu-sep {
  height: 1px;
  background: var(--border);
  margin: 6px 4px;
}

.menu-foot {
  flex: none;
}

.add-option {
  color: var(--accent);
}
</style>
