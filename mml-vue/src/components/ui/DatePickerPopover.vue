<script setup lang="ts">
// 日期选择器的月历弹层：定位、翻月、选日期、Esc / 滚动 / 点外面收起都在这里。
//
// 触发按钮留在 BaseDatePicker：那边只管开关与回写，弹层挂到 body 上，
// 不受父级滚动容器裁切。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { t } from "../../lib/i18n";
import { formatDate, monthCells, parseDate, shiftMonth } from "../../lib/dateGrid";

const props = defineProps<{
  /** 当前日期，YYYY-MM-DD；空串表示未选 */
  modelValue: string;
  /** 可选的最早日期，YYYY-MM-DD；空串表示不限 */
  min: string;
  /** 可选的最晚日期，YYYY-MM-DD；空串表示不限 */
  max: string;
  /** 锚点：触发按钮，弹层贴着它展开 */
  anchor: HTMLElement | null;
}>();

const emit = defineEmits<{
  /** 选中某天，YYYY-MM-DD */
  (e: "pick", date: string): void;
  /** 请求收起弹层 */
  (e: "close"): void;
}>();

/** 今天的日期，用来定位初始视图与标「今天」 */
function todayParts() {
  const d = new Date();
  return { year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate() };
}

/** 弹层的位置：fixed 坐标，挂到 body 后自己算；首帧先按锚点摆，避免闪到窗口左上角 */
const anchorRect = props.anchor?.getBoundingClientRect();
const popStyle = ref<Record<string, string>>(
  anchorRect ? { left: anchorRect.left + "px", top: anchorRect.bottom + 4 + "px" } : {},
);
/** 弹层元素：定位与"点外面收起"都靠它 */
const popEl = ref<HTMLElement | null>(null);

/** 当前显示的月份 */
const viewYear = ref(todayParts().year);
const viewMonth = ref(todayParts().month);

/** 星期的表头，周一起始；格子窄，标签截到两个字符以内 */
const WEEKDAYS = computed(() => {
  const days = [t("date.mon"), t("date.tue"), t("date.wed"), t("date.thu"), t("date.fri"), t("date.sat"), t("date.sun")];
  return days.map((w) => (w.length > 1 ? w.slice(0, 2) : w));
});

/** 标题：中文按「2026 年 10 月」，英文按「10/2026」 */
const title = computed(() =>
  t("date.monthTitle", { y: viewYear.value, m: viewMonth.value }),
);

/** 42 格 */
const cells = computed(() => monthCells(viewYear.value, viewMonth.value));

/** 今天，用来给那一格加描边 */
const today = computed(() => {
  const p = todayParts();
  return formatDate(p.year, p.month, p.day);
});

/**
 * 某一格能不能选
 *
 * - `date`: 该格日期
 *
 * # 返回值
 *
 * 落在 [min, max] 之内才是 true
 */
function selectable(date: string): boolean {
  if (props.min && date < props.min) return false;
  if (props.max && date > props.max) return false;
  return true;
}

/** 打开时把视图定位到当前值所在月，没有值就看今天 */
function syncView() {
  const parsed = parseDate(props.modelValue);
  const base = parsed ?? todayParts();
  viewYear.value = base.year;
  viewMonth.value = base.month;
}

// 初始视图跟着当前值走，别先画今天再跳
syncView();

/** 弹层与窗口边缘至少留这么多，单位 px */
const EDGE = 8;

/**
 * 按触发按钮的位置算弹层的 fixed 坐标
 *
 * 优先贴着按钮左下展开；右边放不下就右对齐，下面放不下就翻到上面，
 * 两个方向最后都夹进窗口内 —— 弹层挂在 body 上，没有父级 overflow 帮忙裁。
 */
function place() {
  const pop = popEl.value;
  const anchor = props.anchor;
  if (!pop || !anchor) return;
  const rect = anchor.getBoundingClientRect();
  const w = pop.offsetWidth;
  const h = pop.offsetHeight;

  let left = rect.left;
  if (left + w > window.innerWidth - EDGE) left = rect.right - w;
  left = Math.max(EDGE, Math.min(left, window.innerWidth - w - EDGE));

  let top = rect.bottom + 4;
  if (top + h > window.innerHeight - EDGE) {
    // 下面放不下：翻到按钮上方；上方也不够就贴着窗口下沿
    top = rect.top - h - 4;
    if (top < EDGE) top = Math.max(EDGE, window.innerHeight - h - EDGE);
  }

  popStyle.value = { left: left + "px", top: top + "px" };
}

/**
 * 选一个日期
 *
 * - `date`: YYYY-MM-DD
 */
function pick(date: string) {
  if (!selectable(date)) return;
  emit("pick", date);
  emit("close");
}

/**
 * 翻月
 *
 * - `delta`: 负数为上个月，正数为下个月
 */
function shift(delta: number) {
  const next = shiftMonth(viewYear.value, viewMonth.value, delta);
  viewYear.value = next.year;
  viewMonth.value = next.month;
}

/** 跳到今天所在的月；今天可选时顺手选上 */
function pickToday() {
  const p = todayParts();
  viewYear.value = p.year;
  viewMonth.value = p.month;
  const date = formatDate(p.year, p.month, p.day);
  if (selectable(date)) pick(date);
}

/** 点外面收起来：锚点自己也排除，否则点触发按钮会被这里先关掉、再被按钮打开 */
function onDocPointerDown(e: PointerEvent) {
  const target = e.target as Node | null;
  if (!target) return;
  if (popEl.value?.contains(target) || props.anchor?.contains(target)) return;
  emit("close");
}

/** Esc 关闭 */
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

/** 页面滚动时收起：fixed 坐标不会跟着内容走，留在原地会飘 */
function onScroll() {
  emit("close");
}

/** 窗口大小变了：重新摆位，不用收起来 */
function onResize() {
  place();
}

onMounted(async () => {
  await nextTick();
  place();
  document.addEventListener("pointerdown", onDocPointerDown, true);
  document.addEventListener("keydown", onKeydown);
  window.addEventListener("scroll", onScroll, true);
  window.addEventListener("resize", onResize);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown, true);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("scroll", onScroll, true);
  window.removeEventListener("resize", onResize);
});

// 外部改了值（如双击柱子跳到某天）：开着的话把视图跟过去
watch(() => props.modelValue, syncView);
</script>

<template>
  <div ref="popEl" class="dp-pop" :style="popStyle">
    <div class="dp-head">
      <button type="button" class="dp-nav" :aria-label="t('date.prevMonth')" @click="shift(-1)">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2"
          stroke-linecap="round">
          <path d="m15 18-6-6 6-6" />
        </svg>
      </button>
      <span class="dp-title">{{ title }}</span>
      <button type="button" class="dp-nav" :aria-label="t('date.nextMonth')" @click="shift(1)">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2"
          stroke-linecap="round">
          <path d="m9 18 6-6-6-6" />
        </svg>
      </button>
    </div>

    <div class="dp-week">
      <span v-for="w in WEEKDAYS" :key="w">{{ w }}</span>
    </div>

    <div class="dp-grid">
      <button v-for="cell in cells" :key="cell.date" type="button" class="dp-cell"
        :class="{ outside: !cell.inMonth, today: cell.date === today, picked: cell.date === modelValue }"
        :disabled="!selectable(cell.date)" @click="pick(cell.date)">
        {{ cell.day }}
      </button>
    </div>

    <div class="dp-foot">
      <button type="button" class="dp-today" @click="pickToday">{{ t("date.today") }}</button>
    </div>
  </div>
</template>

<style scoped>
.dp-pop {
  /* fixed：弹层挂在 body 上，坐标由 place() 算，不受父级滚动容器的 overflow 裁切 */
  position: fixed;
  /* 挂到 body 后要盖过模态（200）里的表单控件，又不压全局提示（300），见 §16.6 的层级表 */
  z-index: 250;
  width: 244px;
  padding: 10px;
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
}

.dp-head {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 6px;
}

.dp-title {
  flex: 1;
  text-align: center;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.dp-nav {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.dp-nav:hover {
  background: var(--bg-hover);
  color: var(--text);
}

/* 星期表头与日期格共用同一套列宽 */
.dp-week,
.dp-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 2px;
}

.dp-week span {
  text-align: center;
  font-size: 10.5px;
  color: var(--text-dim);
  line-height: 22px;
}

.dp-cell {
  height: 28px;
  padding: 0;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: 12.5px;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.dp-cell:hover:not(:disabled) {
  background: var(--bg-hover);
}

/* 上月末 / 下月初：压暗一档，但仍可选 */
.dp-cell.outside {
  color: var(--text-dim);
}

/* 今天：描一圈强调色，不抢选中态的实心块 */
.dp-cell.today {
  box-shadow: inset 0 0 0 1px var(--accent);
}

.dp-cell.picked {
  background: var(--accent);
  color: var(--on-accent);
  font-weight: 600;
}

.dp-cell.picked:hover {
  background: var(--accent);
}

.dp-cell:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.dp-foot {
  display: flex;
  justify-content: center;
  margin-top: 6px;
  padding-top: 6px;
  border-top: 1px solid var(--border);
}

.dp-today {
  padding: 4px 12px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: 12.5px;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}

.dp-today:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}
</style>
