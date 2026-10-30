<script setup lang="ts">
// 游戏统计窗口：全局汇总 + 按实例与时间段查看每日游玩时间图 + 每实例汇总表
// 数据来自 Rust 侧 stats_get_data / stats_get_playtime，即内核 game_count::CountObj 快照；
// 运行中每 5 秒刷新，图表里的今天会跟着本次运行继续长
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import InstanceIcon from "../../components/InstanceIcon.vue";
import DaySharePie from "./parts/DaySharePie.vue";
import { instanceColor } from "../../lib/instanceColor";
import BaseDatePicker from "../../components/ui/BaseDatePicker.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { api } from "../../lib/api";
import { t } from "../../lib/i18n";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { createListScroll, useVirtualRows } from "../../composables/useVirtualRows";
import type { StatsDataDto, StatsInstanceDto, StatsPlaytimeDto } from "../../lib/bindings";

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次返回会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

/** 窗口框架：拿它的内容区当行虚拟化的滚动容器 */
const frame = ref<{ bodyEl: HTMLElement | null } | null>(null);
/** 行容器的根元素 */
const rowsEl = ref<HTMLElement | null>(null);

/** 表格一行的高度，单位 px；与 .stats-row 的 height 同源 */
const STATS_ROW_H = 48;

/** 整页滚动交给行虚拟化：几百个实例时只渲染视口附近那几行 */
const listScroll = createListScroll();

/** 全局汇总快照；null 表示还没读到 */
const data = ref<StatsDataDto | null>(null);

/** 表格可排序的列 */
type SortKey = "name" | "count" | "failed" | "seconds" | "last";

/** 当前排序列；默认按最近启动，与后端的初始顺序一致 */
const sortKey = ref<SortKey>("last");
/** 排序方向：true 降序 */
const sortDesc = ref(true);

/**
 * 按当前排序列排好的实例
 *
 * 同值时按名字兜底，刷新时顺序才不会来回跳。
 */
const sortedInstances = computed(() => {
  const list = [...(data.value?.instances ?? [])];
  const dir = sortDesc.value ? -1 : 1;
  list.sort((a, b) => {
    let cmp: number;
    switch (sortKey.value) {
      case "name":
        cmp = a.name.localeCompare(b.name);
        break;
      case "count":
        cmp = a.count - b.count;
        break;
      case "failed":
        cmp = a.failed - b.failed;
        break;
      case "seconds":
        cmp = a.seconds - b.seconds;
        break;
      default:
        // 没有启动记录的排在最后
        cmp = (a.last ?? -1) - (b.last ?? -1);
        break;
    }
    if (cmp === 0) cmp = a.name.localeCompare(b.name);
    return cmp * dir;
  });
  return list;
});

/**
 * 切换排序列
 *
 * - `key`: 点的那一列；再点同一列翻转方向
 */
function toggleSort(key: SortKey) {
  if (sortKey.value === key) {
    sortDesc.value = !sortDesc.value;
    return;
  }
  sortKey.value = key;
  // 名字默认升序，次数 / 时长 / 时间默认降序（先看大的）
  sortDesc.value = key !== "name";
}

/** 表头的 aria-sort 取值 */
function ariaSort(key: SortKey): "ascending" | "descending" | "none" {
  if (sortKey.value !== key) return "none";
  return sortDesc.value ? "descending" : "ascending";
}

const { start: rowStart, end: rowEnd, padTop, padBottom } = useVirtualRows({
  root: rowsEl,
  count: () => sortedInstances.value.length,
  rowH: STATS_ROW_H,
  scroll: listScroll,
});

/** 当前要渲染的那几行 */
const visibleInstances = computed(() =>
  sortedInstances.value.slice(rowStart.value, rowEnd.value),
);

// ---------------- 游玩时间图：实例 + 时间段 ----------------

/** 可选时间段，值是统计天数；默认近 30 天 */
const RANGE_DAYS = [7, 30, 90] as const;

/** 当前时间段，取值为 RANGE_DAYS 里的字符串 */
const range = ref("30");

/** 是否走自定义日期范围；开着时隐藏 7 / 30 / 90 预设 */
const customRange = ref(false);
/** 自定义范围的起止日期，YYYY-MM-DD */
const fromDate = ref("");
const toDate = ref("");
/** 今天的本地日期：没有统计记录时的兜底上界；跨天后由刷新补上 */
const todayDate = ref(localDate(new Date()));

/** 时间段选项文案，语言切换后跟着重算 */
const rangeOptions = computed(() =>
  RANGE_DAYS.map((days) => ({ value: String(days), label: t("winStats.range" + days) })),
);

/** 当前实例数，不含已删除的 */
const currentCount = computed(() => data.value?.instances.filter((inst) => !inst.deleted).length ?? 0);
/** 有历史统计、但实例已经删掉的个数 */
const deletedCount = computed(() => data.value?.instances.filter((inst) => inst.deleted).length ?? 0);

/** 图表选中的实例；空串表示全部实例 */
const selectedUuid = ref("");

/** 当前选中的实例；全部实例或实例已消失时为 null */
const selectedInstance = computed(() =>
  selectedUuid.value
    ? (data.value?.instances.find((inst) => inst.uuid === selectedUuid.value) ?? null)
    : null,
);

/** 图表范围里的实例：选中一个就是它，否则全部 */
const scopeInstances = computed(() =>
  selectedUuid.value
    ? (data.value?.instances ?? []).filter((inst) => inst.uuid === selectedUuid.value)
    : (data.value?.instances ?? []),
);

/** 统计范围的第一天：最早一条记录，没有记录时为空串 */
const minDate = computed(() => {
  let min: number | null = null;
  for (const inst of scopeInstances.value) {
    if (inst.first == null) continue;
    if (min === null || inst.first < min) min = inst.first;
  }
  return min === null ? "" : localDate(new Date(min));
});

/** 统计范围的最后一天：最近一次启动，没有记录时为空串 */
const maxDate = computed(() => {
  let max: number | null = null;
  for (const inst of scopeInstances.value) {
    if (inst.last == null) continue;
    if (max === null || inst.last > max) max = inst.last;
  }
  return max === null ? "" : localDate(new Date(max));
});

/** 实例下拉是否展开 */
const instOpen = ref(false);

/** 图表数据；null 表示还没读到 */
const playtime = ref<StatsPlaytimeDto | null>(null);
/** 至少成功读过一次，用来区分"正在读"与"这段时间真没有记录" */
const playtimeLoaded = ref(false);
/** 图表请求序号：换实例 / 换时间段后旧请求的结果直接丢掉 */
let playtimeSeq = 0;

/** 图表的实例下拉：全部实例 + 现有与有历史记录的实例 */
const instanceOptions = computed(() => data.value?.instances ?? []);

/** 图表覆盖的天数：日均与单日判断按它算 */
const dayCount = computed(() => playtime.value?.days.length ?? 0);

/** 逐日画柱子的上限；超过就把相邻几天合并成一根 */
const MERGE_OVER_DAYS = 90;

/** 合并后的柱子数量档位，从密到疏 */
const MERGE_BUCKETS = [90, 60, 30] as const;

/** 这次画多少根柱子：不超过 90 天一天一根，再长就从 90 / 60 / 30 里挑一档 */
const barCount = computed(() => {
  const days = dayCount.value;
  if (days <= MERGE_OVER_DAYS) return days;
  if (days <= 180) return MERGE_BUCKETS[0];
  if (days <= 360) return MERGE_BUCKETS[1];
  return MERGE_BUCKETS[2];
});

/** 图上的一根柱子：一天，或合并后的若干天 */
interface ChartBar {
  /** 起始日期，YYYY-MM-DD */
  from: string;
  /** 结束日期，含当天，YYYY-MM-DD */
  to: string;
  /** 这段的游玩秒数 */
  seconds: number;
  /** 这段的启动次数 */
  count: number;
  /** 这段的启动失败次数 */
  failed: number;
}

/** 按 barCount 把逐日数据切成等分的柱子，每根柱子覆盖的天数最多差一天 */
const chartBars = computed<ChartBar[]>(() => {
  const days = playtime.value?.days ?? [];
  const total = days.length;
  const buckets = barCount.value;
  if (total === 0 || buckets === 0) return [];
  const bars: ChartBar[] = [];
  for (let i = 0; i < buckets; i++) {
    const start = Math.floor((i * total) / buckets);
    const end = Math.max(start + 1, Math.floor(((i + 1) * total) / buckets));
    const slice = days.slice(start, Math.min(end, total));
    if (slice.length === 0) continue;
    bars.push({
      from: slice[0].date,
      to: slice[slice.length - 1].date,
      seconds: slice.reduce((sum, day) => sum + day.seconds, 0),
      count: slice.reduce((sum, day) => sum + day.count, 0),
      failed: slice.reduce((sum, day) => sum + day.failed, 0),
    });
  }
  return bars;
});

/** 只选了一天：此时用环形图看这一天各实例的占比 */
const isSingleDay = computed(() => playtime.value?.days.length === 1);

/** 范围内各实例的统计，按时长降序 */
const dayRows = computed(() => playtime.value?.byInstance ?? []);

/** 图表形态：`bars` 柱状图 / `pie` 饼图；只选一天时固定用饼图 */
const chartMode = ref("bars");

/** 当前是否画饼图：单日固定饼图，多日按用户选的 */
const showPie = computed(() => isSingleDay.value || chartMode.value === "pie");

/** 形态切换的选项文案，语言切换后跟着重算 */
const chartModeOptions = computed(() => [
  { value: "bars", label: t("winStats.modeBars") },
  { value: "pie", label: t("winStats.modePie") },
]);

/** 横轴网格列，柱子数量变化时重算 */
const chartColumns = computed(() => "repeat(" + Math.max(1, barCount.value) + ", minmax(0, 1fr))");

/** 柱间距；柱子一多，固定的间隙会把柱宽挤成 0，按数量收窄 */
const chartGap = computed(() => {
  const n = barCount.value;
  if (n > 240) return "0";
  if (n > 120) return "1px";
  return "2px";
});

/** 累计口径：选中实例时是该实例的总时长与启动次数，全部实例时是全局合计（不随日期范围变） */
const playSub = computed(() => {
  const inst = selectedInstance.value;
  const seconds = inst ? inst.seconds : (data.value?.totalSeconds ?? 0);
  const count = inst ? inst.count : (data.value?.launchCount ?? 0);
  return t("winStats.playSubInstance", { h: hoursNum(seconds), count });
});

/** 纵轴顶格小时数，至少 1 小时，向上取整到整小时 */
const axisMaxHours = computed(() => {
  const max = chartBars.value.reduce((acc, bar) => Math.max(acc, bar.seconds), 0);
  return Math.max(1, Math.ceil(max / 3600));
});

/** 纵轴刻度文案 */
const axisTop = computed(() => hourTick(axisMaxHours.value));
const axisMid = computed(() => hourTick(axisMaxHours.value / 2));
const axisBottom = computed(() => hourTick(0));

// ---------------- 数据读取 ----------------

let timer: number | null = null;

/** 拉一次全局统计；失败保留上次数据 */
async function load() {
  try {
    data.value = await api.getStatsData();
  } catch {
    return;
  }
  // 选中的实例可能已经被删掉：退回全部实例，不然下拉会停在空白项上
  if (selectedUuid.value && !data.value.instances.some((inst) => inst.uuid === selectedUuid.value)) {
    selectedUuid.value = "";
  }
}

/** 拉一次当前实例与日期范围内的每日游玩时长 */
async function loadPlaytime() {
  let from: string;
  let to: string;
  if (customRange.value) {
    if (!fromDate.value || !toDate.value) return;
    from = fromDate.value;
    to = toDate.value;
    // 两端填反了也不报错，按用户想要的那段画
    if (from > to) [from, to] = [to, from];
  } else {
    // 预设区间贴着"统计最后一天"往前数，不越过第一条记录
    const end = maxDate.value ? parseLocalDate(maxDate.value) : new Date();
    const start = new Date(end);
    start.setDate(start.getDate() - (Number(range.value) - 1));
    from = localDate(start);
    to = localDate(end);
    if (minDate.value && from < minDate.value) from = minDate.value;
  }

  const seq = ++playtimeSeq;
  try {
    const result = await api.getStatsPlaytime(selectedUuid.value || null, from, to);
    if (seq !== playtimeSeq) return;
    playtime.value = result;
    playtimeLoaded.value = true;
  } catch {
    // 读失败保留上一次图表，别把已有数据清空；第一次就读失败时至少结束「正在读取」
    if (seq === playtimeSeq) playtimeLoaded.value = true;
  }
}

/** 统计窗口的一次完整刷新：汇总表与图表一起更新 */
async function refreshAll() {
  todayDate.value = localDate(new Date());
  await load();
  await loadPlaytime();
  // 表格上方的图表高低会变（有记录 / 无记录两种状态），行的落点跟着挪，重量一次
  listScroll.bump();
}

/** 打开 5 秒轮询：汇总表与图表一起刷新 */
function startTimer() {
  if (timer === null) {
    timer = window.setInterval(() => {
      void refreshAll();
    }, 5000);
  }
}

/** 停掉轮询；窗口隐藏或切走时调用，别在后台空转 IPC */
function stopTimer() {
  if (timer !== null) {
    window.clearInterval(timer);
    timer = null;
  }
}

onMounted(async () => {
  // 滚动容器是窗口框架的内容区：表格行在整页滚动里虚拟化，不另起一个滚动条
  listScroll.attach(frame.value?.bodyEl ?? null);
  await refreshAll();
  startTimer();
});

// 单窗口模式：组件被 KeepAlive 缓存，切走不卸载。切走停掉轮询（别在后台空转 IPC），
// 切回先补一次数据再继续，useWindowRefresh 就是"切回时补一次"
useWindowRefresh(refreshAll);
onActivated(startTimer);
onDeactivated(stopTimer);

// 多窗口模式：每个窗口是独立 webview，窗口被最小化 / 隐藏时**不会**触发 onDeactivated，
// 光靠上面那对钩子会一直空转。这里再按文档可见性停一次；单窗口模式下切页面时
// document 始终可见，两者互不干扰。
function onVisibilityChange() {
  if (document.hidden) {
    stopTimer();
  } else {
    void refreshAll();
    startTimer();
  }
}
onMounted(() => document.addEventListener("visibilitychange", onVisibilityChange));

onBeforeUnmount(() => {
  stopTimer();
  document.removeEventListener("visibilitychange", onVisibilityChange);
});

// 换实例 / 换时间段 / 换自定义日期只重拉图表，汇总表跟着 5 秒轮询，不用为这一下再拉一遍
watch([selectedUuid, range, customRange, fromDate, toDate], () => {
  void loadPlaytime();
});

// 换实例或数据刷新后，自定义日期不能落在统计范围外
watch([selectedUuid, minDate, maxDate], clampCustomRange);

// ---------------- 展示格式 ----------------

/** 本地日期文本，YYYY-MM-DD；日期输入框与后端都用这个口径 */
function localDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return d.getFullYear() + "-" + p(d.getMonth() + 1) + "-" + p(d.getDate());
}

/**
 * 把 YYYY-MM-DD 解析成本地时间的零点
 *
 * 不能用 new Date("YYYY-MM-DD")：那是按 UTC 解析，东八区会退到前一天。
 *
 * - `text`: 日期文本，YYYY-MM-DD
 *
 * # 返回值
 *
 * 当天零点的本地时间
 */
function parseLocalDate(text: string): Date {
  const [y, m, d] = text.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/**
 * 双击柱子：把范围收窄到这根柱子覆盖的那段
 *
 * 合并柱可能跨好几天，收窄后按新范围重画；只剩一天时图表自动切成饼图。
 *
 * - `from`: 起始日期，YYYY-MM-DD
 * - `to`: 结束日期，含当天
 */
function pickRange(from: string, to: string) {
  fromDate.value = from;
  toDate.value = to;
  customRange.value = true;
}

/**
 * 从下拉里选一个实例
 *
 * - `uuid`: 实例 uuid，空串表示全部实例
 */
function pickInstance(uuid: string) {
  selectedUuid.value = uuid;
  instOpen.value = false;
}


/**
 * 切换自定义日期范围
 *
 * - `on`: 是否启用自定义范围
 */
function setCustomRange(on: boolean) {
  customRange.value = on;
  // 第一次打开时用当前预设区间起手，用户不至于面对两个空日期框
  if (on && (!fromDate.value || !toDate.value)) {
    const end = maxDate.value ? parseLocalDate(maxDate.value) : new Date();
    const start = new Date(end);
    start.setDate(start.getDate() - (Number(range.value) - 1));
    let from = localDate(start);
    if (minDate.value && from < minDate.value) from = minDate.value;
    fromDate.value = from;
    toDate.value = localDate(end);
  }
  if (on) clampCustomRange();
}

/**
 * 把自定义日期收进当前实例的统计范围
 *
 * 下界是最早一条记录（实例创建 / 统计开始），上界是最近一次启动；
 * 换实例或数据刷新后调用，避免停在范围外的日期上。
 */
function clampCustomRange() {
  if (!customRange.value) return;
  if (fromDate.value && minDate.value && fromDate.value < minDate.value) {
    fromDate.value = minDate.value;
  }
  if (toDate.value && maxDate.value && toDate.value > maxDate.value) {
    toDate.value = maxDate.value;
  }
  if (fromDate.value && toDate.value && fromDate.value > toDate.value) {
    fromDate.value = toDate.value;
  }
}

/** 秒数折成小时数文本，保留一位小数；超过 100 小时不再留小数 */
function hoursNum(seconds: number): string {
  const hours = seconds / 3600;
  return hours >= 100 ? hours.toFixed(0) : hours.toFixed(1);
}

/** 时长列文本 */
function hoursOf(seconds: number): string {
  return t("winStats.hours", { h: hoursNum(seconds) });
}

/** 次数列文本 */
function timesOf(n: number): string {
  return t("winStats.times", { n });
}

/** 卡片 / 图表里的短时长，如 12.3h */
function hoursShort(seconds: number): string {
  return t("winStats.hourShort", { h: hoursNum(seconds) });
}

/** 纵轴刻度：整小时不留小数，半小时保留一位 */
function hourTick(hours: number): string {
  const text = Number.isInteger(hours) ? String(hours) : hours.toFixed(1);
  return t("winStats.hourShort", { h: text });
}

/** 总游戏时长 */
const totalPlay = computed(() => hoursShort(data.value?.totalSeconds ?? 0));

/** 最近游玩时间：epoch 毫秒转 YYYY-MM-DD HH:mm */
function fmtTime(ms: number | null) {
  if (!ms) return t("detail.none");
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, "0");
  return d.getFullYear() + "-" + p(d.getMonth() + 1) + "-" + p(d.getDate()) + " " + p(d.getHours()) + ":" + p(d.getMinutes());
}

/** 柱高：按纵轴顶格取百分比；有记录的最少 2%，免得看不见 */
function barHeight(seconds: number): string {
  if (seconds <= 0) return "0%";
  const pct = (seconds / (axisMaxHours.value * 3600)) * 100;
  return Math.max(2, pct).toFixed(2) + "%";
}

/** 横轴标签：只取月-日 */
function xLabel(date: string): string {
  return date.slice(5);
}

/** 横轴标签抽稀：天数多时隔几天标一个，首尾都标 */
function showXLabel(index: number): boolean {
  const n = barCount.value;
  if (n <= 10) return true;
  const step = Math.ceil(n / 6);
  return index % step === 0 || index === n - 1;
}

/** 柱子的悬停提示：日期（合并柱是区间）、这段时长与启动次数；有失败时补上失败次数 */
function barTip(bar: ChartBar): string {
  const date = bar.from === bar.to ? xLabel(bar.from) : xLabel(bar.from) + "–" + xLabel(bar.to);
  const args = { date, h: hoursNum(bar.seconds), count: bar.count };
  return bar.failed > 0
    ? t("winStats.barTipFail", { ...args, failed: bar.failed })
    : t("winStats.barTip", args);
}

/** 失败段占整根柱子的高度：按失败次数占比，太低时留 4px 保证看得见 */
function failHeight(bar: ChartBar): string {
  const pct = (bar.failed / Math.max(1, bar.count)) * 100;
  return "min(100%, max(4px, " + pct.toFixed(1) + "%))";
}

/** 实例下拉项文案：已删除的实例补一个标记，免得只剩 UUID 时分不清 */
function instanceLabel(inst: StatsInstanceDto): string {
  return inst.deleted ? t("winStats.deletedName", { name: inst.name }) : inst.name;
}
</script>

<template>
  <WindowFrame ref="frame" :title="t('features.stats')" body-gutter @close="$emit('close')">
    <div class="summary">
      <div class="summary-card">
        <span class="summary-num">{{ currentCount }}</span>
        <span class="summary-label">{{ t("winStats.instances") }}</span>
      </div>
      <div class="summary-card">
        <span class="summary-num">{{ deletedCount }}</span>
        <span class="summary-label">{{ t("winStats.deletedInstances") }}</span>
      </div>
      <div class="summary-card">
        <span class="summary-num">{{ data?.launchCount ?? 0 }}</span>
        <span class="summary-label">{{ t("winStats.totalLaunch") }}</span>
      </div>
      <div class="summary-card">
        <span class="summary-num" :class="{ danger: (data?.launchErrorCount ?? 0) > 0 }">{{
          data?.launchErrorCount ?? 0 }}</span>
        <span class="summary-label">{{ t("winStats.totalFailed") }}</span>
      </div>
      <div class="summary-card">
        <span class="summary-num">{{ totalPlay }}</span>
        <span class="summary-label">{{ t("winStats.totalPlay") }}</span>
      </div>
    </div>

    <!-- 游玩时间图：工具条里选实例与时间段，图表跟着两者变 -->
    <section class="play-panel">
      <!-- 点下拉外面收起：遮罩铺满本面板，实例列表再长也不会盖到汇总卡片上 -->
      <div v-if="instOpen" class="inst-backdrop" @click="instOpen = false"></div>
      <!-- 工具条固定两行：第一行标题 + 实例 + 时间区间，第二行三个区间统计。
           标题与两个控件组各占自己的格子，谁都不按内容去挤别人，换实例时版式不动 -->
      <div class="play-bar">
        <div class="play-top">
          <h2 class="play-title">{{ t("winStats.playTitle") }}</h2>
          <div class="inst-picker">
            <button type="button" class="field-select inst-select" :aria-label="t('winStats.colInstance')"
              :title="selectedInstance ? instanceLabel(selectedInstance) : undefined" @click="instOpen = !instOpen">
              <span class="inst-select-text">{{ selectedInstance ? instanceLabel(selectedInstance) : t("winStats.allInstances") }}</span>
            </button>
            <div v-if="instOpen" class="inst-menu">
              <button type="button" class="inst-option" :class="{ active: !selectedUuid }" @click="pickInstance('')">
                {{ t("winStats.allInstances") }}
              </button>
              <button v-for="inst in instanceOptions" :key="inst.uuid" type="button" class="inst-option"
                :class="{ active: inst.uuid === selectedUuid }" @click="pickInstance(inst.uuid)">
                {{ instanceLabel(inst) }}
              </button>
            </div>
          </div>
          <div class="range-controls">
            <label class="range-custom">
              <BaseSwitch :model-value="customRange" @update:model-value="setCustomRange" />
              <span>{{ t("winStats.customRange") }}</span>
            </label>
            <SegmentedTabs v-if="!customRange" v-model="range" :options="rangeOptions" />
            <template v-else>
              <BaseDatePicker v-model="fromDate" class="range-date" :min="minDate"
                :max="toDate || maxDate || todayDate" :aria-label="t('winStats.dateFrom')" />
              <span class="range-sep">–</span>
              <BaseDatePicker v-model="toDate" class="range-date" :min="fromDate || minDate"
                :max="maxDate || todayDate" :aria-label="t('winStats.dateTo')" />
            </template>
          </div>
          <!-- 形态切换：单日固定饼图，不给切 -->
          <SegmentedTabs v-if="!isSingleDay" v-model="chartMode" :options="chartModeOptions" />
        </div>

        <div class="play-stats">
          <div class="play-stat">
            <span class="play-num">{{ hoursShort(playtime?.totalSeconds ?? 0) }}</span>
            <span class="play-cap">{{ t("winStats.rangePlay") }}</span>
          </div>
          <div class="play-stat">
            <span class="play-num">{{ playtime?.activeDays ?? 0 }}</span>
            <span class="play-cap">{{ t("winStats.activeDays") }}</span>
          </div>
          <div class="play-stat">
            <span class="play-num">{{ hoursShort((playtime?.totalSeconds ?? 0) / Math.max(1, dayCount)) }}</span>
            <span class="play-cap">{{ t("winStats.dailyAverage") }}</span>
          </div>
          <!-- 累计口径贴在统计行右端：选中实例时是该实例的累计，全部实例时是全局合计 -->
          <p v-if="data" class="play-total" :title="playSub">{{ playSub }}</p>
        </div>
      </div>

      <!-- 图表区：只选一天时换成饼图看实例占比，多天时是柱状图。
           零记录与还没读到时各给一句说明，不留空框 -->
      <div v-if="showPie && dayRows.length > 0" class="day-share">
        <div class="ring-row">
          <DaySharePie :items="dayRows.map((r) => ({ uuid: r.uuid, name: r.name, value: r.seconds }))"
            :title="t('winStats.colHours')" kind="hours" />
          <DaySharePie :items="dayRows.map((r) => ({ uuid: r.uuid, name: r.name, value: r.count }))"
            :title="t('winStats.colCount')" kind="count" />
          <DaySharePie :items="dayRows.map((r) => ({ uuid: r.uuid, name: r.name, value: r.failed }))"
            :title="t('winStats.colFailed')" kind="count" />
        </div>
        <!-- 三个环共用一份图例：同一实例在三个环里是同一个颜色，图例列也对齐 -->
        <div class="day-legend">
          <div class="legend-head">
            <span>{{ t("winStats.colInstance") }}</span>
            <span>{{ t("winStats.colHours") }}</span>
            <span>{{ t("winStats.colCount") }}</span>
            <span>{{ t("winStats.colFailed") }}</span>
          </div>
          <div v-for="row in dayRows" :key="row.uuid" class="legend-item">
            <span class="legend-name">
              <span class="legend-dot" :style="{ background: instanceColor(row.uuid) }"></span>
              <span class="legend-text" :title="row.name">{{ row.name }}</span>
            </span>
            <span>{{ hoursOf(row.seconds) }}</span>
            <span>{{ timesOf(row.count) }}</span>
            <span :class="{ danger: row.failed > 0 }">{{ timesOf(row.failed) }}</span>
          </div>
        </div>
      </div>
      <div v-else-if="playtime && playtime.totalSeconds > 0" class="chart">
        <div class="chart-y">
          <span>{{ axisTop }}</span>
          <span>{{ axisMid }}</span>
          <span>{{ axisBottom }}</span>
        </div>
        <div class="chart-main">
          <div class="chart-body">
            <div class="chart-lines" aria-hidden="true">
              <span></span>
              <span></span>
              <span></span>
            </div>
            <div class="chart-bars" :style="{ gridTemplateColumns: chartColumns, gap: chartGap }">
              <div v-for="bar in chartBars" :key="bar.from" class="chart-slot">
                <div class="chart-bar" :class="{ empty: bar.seconds === 0 }"
                  :style="{ height: bar.seconds > 0 ? barHeight(bar.seconds) : undefined }" v-tip="barTip(bar)"
                  @dblclick="pickRange(bar.from, bar.to)">
                  <span v-if="bar.failed > 0" class="chart-bar-fail" :style="{ height: failHeight(bar) }"></span>
                </div>
              </div>
            </div>
          </div>
          <div class="chart-x" :style="{ gridTemplateColumns: chartColumns, gap: chartGap }">
            <span v-for="(bar, index) in chartBars" :key="bar.from">
              <i v-if="showXLabel(index)">{{ xLabel(bar.from) }}</i>
            </span>
          </div>
        </div>
      </div>
      <div v-else-if="!playtimeLoaded" class="chart-empty">{{ t("winStats.loading") }}</div>
      <div v-else class="chart-empty">{{ t("winStats.chartEmpty") }}</div>

      <!-- 双击是隐藏操作，给一句话提示；单日已经是饼图，不用再说 -->
      <p v-if="!showPie && playtime && playtime.totalSeconds > 0" class="chart-hint">
        {{ t("winStats.dblHint") }}
      </p>
    </section>

    <!-- 实例汇总：行虚拟化，几百个实例也只渲染视口附近那几行。
         滚动容器是 WindowFrame 的内容区（整页滚动不变），上下留白撑住总高度 -->
    <div class="stats-table" role="table">
      <div class="stats-head" role="row">
        <span role="columnheader" :aria-sort="ariaSort('name')">
          <button type="button" class="sort-btn" @click="toggleSort('name')">
            {{ t("winStats.colInstance") }}
            <span v-if="sortKey === 'name'" class="sort-arrow">{{ sortDesc ? "▼" : "▲" }}</span>
          </button>
        </span>
        <span role="columnheader" :aria-sort="ariaSort('count')">
          <button type="button" class="sort-btn" @click="toggleSort('count')">
            {{ t("winStats.colCount") }}
            <span v-if="sortKey === 'count'" class="sort-arrow">{{ sortDesc ? "▼" : "▲" }}</span>
          </button>
        </span>
        <span role="columnheader" :aria-sort="ariaSort('failed')">
          <button type="button" class="sort-btn" @click="toggleSort('failed')">
            {{ t("winStats.colFailed") }}
            <span v-if="sortKey === 'failed'" class="sort-arrow">{{ sortDesc ? "▼" : "▲" }}</span>
          </button>
        </span>
        <span role="columnheader" :aria-sort="ariaSort('seconds')">
          <button type="button" class="sort-btn" @click="toggleSort('seconds')">
            {{ t("winStats.colHours") }}
            <span v-if="sortKey === 'seconds'" class="sort-arrow">{{ sortDesc ? "▼" : "▲" }}</span>
          </button>
        </span>
        <span role="columnheader" :aria-sort="ariaSort('last')">
          <button type="button" class="sort-btn" @click="toggleSort('last')">
            {{ t("winStats.colLast") }}
            <span v-if="sortKey === 'last'" class="sort-arrow">{{ sortDesc ? "▼" : "▲" }}</span>
          </button>
        </span>
      </div>
      <div v-if="!data || data.instances.length === 0" class="empty-cell">{{ t("winStats.noData") }}</div>
      <div v-else ref="rowsEl" class="stats-rows"
        :style="{ paddingTop: padTop + 'px', paddingBottom: padBottom + 'px' }">
        <div v-for="inst in visibleInstances" :key="inst.uuid" class="stats-row" role="row">
          <div class="cell-inst inst-cell" role="cell">
            <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="30" />
            <span class="inst-name">{{ inst.name }}</span>
            <span v-if="inst.deleted" class="deleted-tag">{{ t("winStats.deleted") }}</span>
            <span v-else-if="inst.running" class="running-tag">{{ t("winStats.running") }}</span>
          </div>
          <span role="cell">{{ inst.count }}</span>
          <span :class="{ danger: inst.failed > 0 }" role="cell">{{ inst.failed }}</span>
          <span role="cell">{{ hoursOf(inst.seconds) }}</span>
          <span role="cell">{{ fmtTime(inst.last) }}</span>
        </div>
      </div>
    </div>

    <p class="foot-note">{{ t("winStats.foot") }}</p>
  </WindowFrame>
</template>

<style scoped>
.summary {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  margin-bottom: 16px;
}

.summary-card {
  flex: 1;
  min-width: 120px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
}

.summary-num {
  font-size: 18px;
  font-weight: 800;
  color: var(--accent);
  /* 数字每 5 秒刷新一次，等宽数字才不会让卡片跟着抖 */
  font-variant-numeric: tabular-nums;
}

/* 有失败时把这一个数字标红，扫一眼就知道统计里有失败 */
.summary-num.danger {
  color: var(--red);
}

.summary-label {
  font-size: 11.5px;
  color: var(--text-dim);
}

/* ---------- 游玩时间图 ---------- */

.play-panel {
  /* 图表区高度：纵轴、柱状区与空状态三处共用，改一处三处一起变 */
  --chart-h: 150px;
  position: relative;
  margin-bottom: 16px;
  padding: 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
}

/* 工具条固定两行：第一行标题 / 实例 / 时间区间，第二行三个区间统计 */
.play-bar {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-bottom: 12px;
}

/* 第一行三格：标题 / 实例下拉 / 时间区间。
   下拉吃掉标题与区间控件之间的剩余宽度，宽度只由窗口决定，不随选中项的长短变 */
.play-top {
  display: flex;
  align-items: center;
  gap: 12px;
}

.play-title {
  flex: 0 0 auto;
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}

/* 时间区间控件：开关 + 预设分段（或两个日期框）。不参与伸缩，宽度只由组内控件决定 */
.range-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  flex: 0 0 auto;
}

/* 自定义时间段：开关 + 文案；选预设时这一对保留，7 / 30 / 90 隐藏 */
.range-custom {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text);
  user-select: none;
  cursor: pointer;
  white-space: nowrap;
}

/* 日期选择器：宽度挂在组件根元素上，外观由组件自己管 */
.range-date {
  width: 140px;
}

.range-sep {
  color: var(--text-dim);
}

/* 实例下拉是自绘的：原生 <select> 的宽度由最长选项文字决定，实例名一长就把
   后面的控件挤走。这里让它吃掉第一行的剩余宽度，文字在按钮里省略 */
.inst-picker {
  position: relative;
  /* 剩余宽度归它；下限保证窄窗时还认得出是哪个实例 */
  flex: 1 1 auto;
  min-width: 160px;
}

.inst-select {
  display: flex;
  align-items: center;
  width: 100%;
  /* 借用 .field-select 的外观（底色 / 边框 / 箭头 / 高度），按钮要把文字对齐与省略补上 */
  text-align: left;
  overflow: hidden;
}

.inst-select-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 停在面板图标层之上：下拉开着时它压在面板其它内容上，点哪儿都是收起，
   而不是让首次点击穿透去改开关 / 日期 */
.inst-backdrop {
  position: absolute;
  inset: 0;
  z-index: 20;
}

.inst-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 40;
  min-width: 100%;
  max-width: 360px;
  max-height: 260px;
  overflow-y: auto;
  padding: 5px;
  background: var(--bg-solid, var(--bg-card));
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-lg);
}

.inst-option {
  display: block;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: var(--ctl-font);
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.inst-option:hover {
  background: var(--bg-hover);
}

.inst-option.active {
  color: var(--accent);
  font-weight: 600;
}

.play-stats {
  display: flex;
  align-items: flex-end;
  gap: 24px;
}

/* 选中实例的累计口径：贴统计行右端，太长省略，完整值在悬停提示里 */
.play-total {
  margin: 0 0 1px auto;
  font-size: 11.5px;
  color: var(--text-dim);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.play-stat {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.play-num {
  font-size: 18px;
  font-weight: 800;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.play-cap {
  font-size: 11.5px;
  color: var(--text-dim);
}

.chart {
  display: flex;
  gap: 8px;
}

/* 纵轴：与柱状区同高，三档刻度分别贴着上 / 中 / 下 */
.chart-y {
  width: 40px;
  height: var(--chart-h);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  align-items: flex-end;
  font-size: 10.5px;
  color: var(--text-dim);
  white-space: nowrap;
}

.chart-main {
  flex: 1;
  /* 不横向滚动：天数再多也只压缩柱宽，一屏看完 */
  min-width: 0;
  overflow: hidden;
}

.chart-body {
  position: relative;
  height: var(--chart-h);
}

/* 背景网格：只在柱状区画，不挡鼠标（提示挂在柱子上） */
.chart-lines {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  pointer-events: none;
}

.chart-lines span {
  border-top: 1px dashed var(--border);
}

.chart-bars {
  position: absolute;
  inset: 0;
  display: grid;
  align-items: end;
  /* 间距按天数给，见 chartGap */
}

.chart-slot {
  height: 100%;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}

.chart-bar {
  position: relative;
  width: 100%;
  max-width: 34px;
  border-radius: 3px 3px 0 0;
  background: var(--accent);
  transition: height 0.3s, background-color 0.12s;
  overflow: hidden;
}

/* 启动失败段：贴在当天柱子的底部，高度按失败次数占比给 */
.chart-bar-fail {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--red);
}

/* 双击柱子能钻到那一天，给个手型提示它可点 */
.chart-bar:not(.empty) {
  cursor: pointer;
}

/* 悬停当天柱子提亮一档，与提示一起说明"这根柱子是哪天" */
.chart-bar:not(.empty):hover {
  background: var(--accent-hover);
}

/* 没记录的那天留一条基线，图里不至于看不出格子 */
.chart-bar.empty {
  height: 2px;
  background: var(--border);
  border-radius: 1px;
}

.chart-x {
  display: grid;
  margin-top: 6px;
}

/* 标签比格子宽，参与布局就会把容器撑出横向滚动、最后一天还被裁掉，
   所以让它绝对定位居中，不占网格宽度 */
.chart-x span {
  position: relative;
  height: 15px;
}

.chart-x i {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  font-style: normal;
  font-size: 10.5px;
  color: var(--text-dim);
  white-space: nowrap;
}

/* 首尾贴边：居中会让第一个字探出左边、最后一个被右边界裁掉 */
.chart-x span:first-child i {
  left: 0;
  transform: none;
}

.chart-x span:last-child i {
  left: auto;
  right: 0;
  transform: none;
}

/* 单日：三个环并排，下面一份共享图例 */
.day-share {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.ring-row {
  display: flex;
  justify-content: space-around;
  gap: 16px;
  padding: 4px 0;
}

.day-legend {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 表头与数据行共用同一套列宽：实例名占满剩余，后三列定宽 */
.legend-head,
.legend-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 92px 72px 72px;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
}

.legend-head {
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.legend-head > span:not(:first-child),
.legend-item > span:not(:first-child) {
  text-align: right;
}

.legend-item {
  font-variant-numeric: tabular-nums;
}

.legend-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.legend-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.legend-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.legend-item .danger {
  color: var(--red);
}

.chart-hint {
  margin: 8px 0 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

.chart-empty {
  height: var(--chart-h);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--border);
  border-radius: 10px;
  color: var(--text-dim);
  font-size: 12.5px;
}

/* ---------- 每实例汇总表（行虚拟化） ---------- */

.stats-table {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

/* 表头与行共用同一套列宽：两边分开写，改一处要改两处 */
.stats-head,
.stats-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 88px 64px 104px 132px;
  align-items: center;
  gap: 16px;
  padding: 0 16px;
}

.stats-head {
  height: 40px;
  font-size: 11.5px;
  color: var(--text-dim);
  font-weight: 600;
  background: var(--bg-side);
  border-bottom: 1px solid var(--border);
}

/* 表头可点击排序：按钮只借表格的排版，外观仍是一行文字 */
.sort-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-weight: inherit;
  cursor: pointer;
  text-align: left;
  transition: color 0.12s;
}

.sort-btn:hover {
  color: var(--text);
}

/* 当前排序列的箭头：只标方向，不改变列宽（固定宽度，翻转时不抖） */
.sort-arrow {
  display: inline-block;
  width: 10px;
  font-size: 9px;
  color: var(--accent);
}

/* 行高必须固定：隐藏的行按它估着撑高，见 composables/useVirtualRows.ts。
   分隔线用 box-shadow 画，不占高度；用 border 会随虚拟化窗口边界忽有忽无 */
.stats-row {
  height: 48px;
  font-size: 12.5px;
  font-variant-numeric: tabular-nums;
  box-shadow: inset 0 -1px 0 var(--border);
}

.inst-cell {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  font-weight: 600;
}

/* 实例名过长时省略：列宽被 grid 定死，不省略会顶破相邻列 */
.inst-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.running-tag,
.deleted-tag {
  flex-shrink: 0;
  white-space: nowrap;
  font-size: 10.5px;
  font-weight: 600;
  border-radius: 999px;
  padding: 2px 6px;
}

.running-tag {
  color: var(--accent);
  border: 1px solid var(--accent);
}

.deleted-tag {
  color: var(--text-dim);
  border: 1px solid var(--border);
}

.empty-cell {
  text-align: center;
  color: var(--text-dim);
  padding: 24px;
  font-size: 12.5px;
}

.foot-note {
  font-size: 11.5px;
  color: var(--text-dim);
  margin-top: 10px;
}
</style>