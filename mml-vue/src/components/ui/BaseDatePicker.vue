<script setup lang="ts">
// 日期选择器：点击输入框弹出**自绘**的月历，取代表单里的原生 <input type="date">。
// 原生的日历弹层是浏览器画的，除了 color-scheme 之外没有可定制的伪元素，
// 与界面主题对不上；这里自己画一份，外观全走主题变量。
//
// 值统一是 YYYY-MM-DD 字符串，与后端 DTO 同口径；不做时区换算。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { t } from "../../lib/i18n";
import { formatDate, monthCells, parseDate, shiftMonth } from "../../lib/dateGrid";

const props = withDefaults(
  defineProps<{
    /** 当前日期，YYYY-MM-DD；空串表示未选 */
    modelValue: string;
    /** 可选的最早日期，YYYY-MM-DD；不传表示不限 */
    min?: string;
    /** 可选的最晚日期，YYYY-MM-DD；不传表示不限 */
    max?: string;
    /** 无障碍名称，也是悬停提示 */
    ariaLabel?: string;
    /** 未选日期时输入框里的提示；不给就用通用的「选择日期」 */
    placeholder?: string;
  }>(),
  { min: "", max: "", ariaLabel: "", placeholder: "" },
);

const emit = defineEmits<{
  /** v-model 更新：选中的日期，YYYY-MM-DD */
  (e: "update:modelValue", v: string): void;
}>();

/** 今天的日期，用来定位初始视图与标「今天」 */
function todayParts() {
  const d = new Date();
  return { year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate() };
}

/** 弹层是否展开 */
const open = ref(false);
/** 弹层的位置：fixed 坐标，挂到 body 后自己算，不受父级滚动容器裁切 */
const popStyle = ref<Record<string, string>>({});
/** 当前显示的月份 */
const viewYear = ref(todayParts().year);
const viewMonth = ref(todayParts().month);

/** 根元素与弹层元素：定位、点外面关闭都靠它们 */
const rootEl = ref<HTMLElement | null>(null);
const popEl = ref<HTMLElement | null>(null);

/** 六个星期的表头，周一起始 */
const WEEKDAYS = computed(() => {
  const miss = [t("date.mon"), t("date.tue"), t("date.wed"), t("date.thu"), t("date.fri"), t("date.sat"), t("date.sun")];
  // 英文太宽，弹层里用单字母；中文本来就一个字
  return miss.map((w) => (w.length > 1 ? w.slice(0, 2) : w));
});

/** 标题：中文按「2026 年 10 月」，英文按「October 2026」 */
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

/** 输入框里显示的文本：选了就是日期，没选用调用方给的提示，最后才回落通用提示 */
const label = computed(() => props.modelValue || props.placeholder || t("date.empty"));

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

/** 弹层与窗口边缘至少留这么多 */
const EDGE = 8;

/**
 * 按触发按钮的位置算弹层的 fixed 坐标
 *
 * 优先贴着按钮左下展开；右边放不下就右对齐，下面放不下就翻到上面，
 * 两个方向最后都夹进窗口内 —— 弹层挂在 body 上，没有父级 overflow 帮忙裁。
 */
function place() {
  const pop = popEl.value;
  const root = rootEl.value;
  if (!pop || !root) return;
  const rect = root.getBoundingClientRect();
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

/** 展开 / 收起；展开后量一次尺寸再摆位 */
async function toggle() {
  if (open.value) {
    open.value = false;
    return;
  }
  syncView();
  // 先给个能放下的初始位置，等 DOM 出来量到真实宽度再摆
  const rect = rootEl.value?.getBoundingClientRect();
  popStyle.value = { left: (rect?.left ?? EDGE) + "px", top: (rect?.bottom ?? 0) + 4 + "px" };
  open.value = true;
  await nextTick();
  place();
}

/**
 * 选一个日期
 *
 * - `date`: YYYY-MM-DD
 */
function pick(date: string) {
  if (!selectable(date)) return;
  emit("update:modelValue", date);
  open.value = false;
}

/** 翻月 */
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

/** 点外面收起来：弹层铺在窗口上，点别处要能关掉 */
function onDocPointerDown(e: PointerEvent) {
  if (!open.value) return;
  const target = e.target as Node | null;
  if (target && (rootEl.value?.contains(target) || popEl.value?.contains(target))) return;
  open.value = false;
}

/** Esc 关闭 */
function onKeydown(e: KeyboardEvent) {
  if (open.value && e.key === "Escape") open.value = false;
}

/** 页面滚动时收起：fixed 坐标不会跟着内容走，留在原地会飘 */
function onScroll() {
  if (open.value) open.value = false;
}

/** 窗口大小变了：重新摆位，不用收起来 */
function onResize() {
  if (open.value) place();
}

onMounted(() => {
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
watch(
  () => props.modelValue,
  () => {
    if (open.value) syncView();
  },
);
</script>

<template>
  <div ref="rootEl" class="dp">
    <button type="button" class="field-input dp-trigger" :class="{ 'is-open': open }"
      :aria-label="ariaLabel || undefined" :title="ariaLabel || undefined" @click="toggle">
      <span class="dp-text" :class="{ 'is-empty': !modelValue }">{{ label }}</span>
      <span class="dp-icon" aria-hidden="true"></span>
    </button>

    <!-- 弹层挂到 body：父级是滚动容器，absolute 会被 overflow 裁掉（窗口右边、底边都能切） -->
    <Teleport to="body">
      <Transition name="drop">
        <div v-if="open" ref="popEl" class="dp-pop" :style="popStyle">
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
            <span v-for="(w, i) in WEEKDAYS" :key="i">{{ w }}</span>
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
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.dp {
  position: relative;
  min-width: 0;
}

/* 触发按钮借用 .field-input 的外观；按钮的内容对齐与省略要自己补 */
.dp-trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  text-align: left;
  cursor: pointer;
}

/* 展开态与聚焦态同一个观感，别让用户以为没打开 */
.dp-trigger.is-open {
  border-color: var(--accent);
}

.dp-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.dp-text.is-empty {
  color: var(--text-dim);
}

/* 日历图标与 .field-select 的箭头同尺寸同位置，图来自主题变量 */
.dp-icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
  background: var(--date-icon) center / 12px 12px no-repeat;
  opacity: 0.85;
  transition: opacity 0.12s;
}

.dp-trigger:hover .dp-icon {
  opacity: 1;
}

/* ---------- 弹层 ---------- */

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
  color: #fff;
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
