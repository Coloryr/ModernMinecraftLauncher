<script setup lang="ts">
// 游戏统计窗口：全局汇总 + 按实例与时间段查看每日游玩时间图 + 每实例汇总表
// 数据来自 Rust 侧 stats_get_data / stats_get_playtime，即内核 game_count::CountObj 快照；
// 运行中每 5 秒刷新，图表里的今天会跟着本次运行继续长
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import InstanceIcon from "../../components/InstanceIcon.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { api } from "../../lib/api";
import { t } from "../../lib/i18n";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import type { StatsDataDto, StatsInstanceDto, StatsPlayDayDto, StatsPlaytimeDto } from "../../lib/bindings";

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次返回会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

const data = ref<StatsDataDto | null>(null);

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
/** 今天的本地日期：结束日不能超过它；跨天后由刷新补上 */
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

/** 图表数据；null 表示还没读到 */
const playtime = ref<StatsPlaytimeDto | null>(null);
/** 至少成功读过一次，用来区分"正在读"与"这段时间真没有记录" */
const playtimeLoaded = ref(false);
/** 图表请求序号：换实例 / 换时间段后旧请求的结果直接丢掉 */
let playtimeSeq = 0;

/** 图表的实例下拉：全部实例 + 现有与有历史记录的实例 */
const instanceOptions = computed(() => data.value?.instances ?? []);

/** 图表天数：用作横轴格数 */
const dayCount = computed(() => playtime.value?.days.length ?? 0);

/** 横轴网格列，天数变化时重算 */
const chartColumns = computed(() => "repeat(" + Math.max(1, dayCount.value) + ", minmax(0, 1fr))");

/** 柱状区最小宽度：自定义范围超过 90 天时横向滚动，别把柱子压成一条线 */
const chartMinWidth = computed(() => (dayCount.value > 90 ? dayCount.value * 8 + "px" : "0px"));

/** 图表选中实例的副标题；全部实例时说明合计口径 */
const playSub = computed(() => {
  const inst = selectedInstance.value;
  if (!inst) return t("winStats.playSubAll");
  return t("winStats.playSubInstance", { h: hoursNum(inst.seconds), count: inst.count });
});

/** 纵轴顶格小时数，至少 1 小时，向上取整到整小时 */
const axisMaxHours = computed(() => {
  const max = playtime.value?.days.reduce((acc, day) => Math.max(acc, day.seconds), 0) ?? 0;
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
    const end = new Date();
    const start = new Date();
    start.setDate(start.getDate() - (Number(range.value) - 1));
    from = localDate(start);
    to = localDate(end);
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
}

function startTimer() {
  if (timer === null) {
    timer = window.setInterval(() => {
      void refreshAll();
    }, 5000);
  }
}

function stopTimer() {
  if (timer !== null) {
    window.clearInterval(timer);
    timer = null;
  }
}

onMounted(async () => {
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

// ---------------- 展示格式 ----------------

/** 本地日期文本，YYYY-MM-DD；日期输入框与后端都用这个口径 */
function localDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return d.getFullYear() + "-" + p(d.getMonth() + 1) + "-" + p(d.getDate());
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
    const end = new Date();
    const start = new Date();
    start.setDate(start.getDate() - (Number(range.value) - 1));
    fromDate.value = localDate(start);
    toDate.value = localDate(end);
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
  const n = dayCount.value;
  if (n <= 10) return true;
  const step = Math.ceil(n / 6);
  return index % step === 0 || index === n - 1;
}

/** 柱子的悬停提示：日期 + 当天时长 */
function barTip(day: StatsPlayDayDto): string {
  return t("winStats.barTip", { date: xLabel(day.date), h: hoursNum(day.seconds) });
}

/** 实例下拉项文案：已删除的实例补一个标记，免得只剩 UUID 时分不清 */
function instanceLabel(inst: StatsInstanceDto): string {
  return inst.deleted ? t("winStats.deletedName", { name: inst.name }) : inst.name;
}
</script>

<template>
  <WindowFrame :title="t('features.stats')" body-gutter @close="$emit('close')">
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

    <!-- 游玩时间图：实例与时间段都在这一行，图表跟着两者变 -->
    <section class="play-panel">
      <div class="play-head">
        <div class="play-heading">
          <h2 class="play-title">{{ t("winStats.playTitle") }}</h2>
          <p class="play-sub">
            {{ playSub }}
            <span v-if="selectedInstance?.deleted" class="deleted-tag">{{ t("winStats.deleted") }}</span>
          </p>
        </div>
        <div class="play-controls">
          <select v-model="selectedUuid" class="field-select inst-select" :aria-label="t('winStats.colInstance')">
            <option value="">{{ t("winStats.allInstances") }}</option>
            <option v-for="inst in instanceOptions" :key="inst.uuid" :value="inst.uuid">{{ instanceLabel(inst) }}</option>
          </select>
          <label class="range-custom">
            <BaseSwitch :model-value="customRange" @update:model-value="setCustomRange" />
            <span>{{ t("winStats.customRange") }}</span>
          </label>
          <SegmentedTabs v-if="!customRange" v-model="range" :options="rangeOptions" />
          <template v-else>
            <input v-model="fromDate" type="date" class="field-input range-date" :max="toDate || todayDate"
              :aria-label="t('winStats.dateFrom')" />
            <span class="range-sep">–</span>
            <input v-model="toDate" type="date" class="field-input range-date" :min="fromDate" :max="todayDate"
              :aria-label="t('winStats.dateTo')" />
          </template>
        </div>
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
      </div>

      <!-- 有记录才画图；零记录与还没读到时各给一句说明，不留空框 -->
      <div v-if="playtime && playtime.totalSeconds > 0" class="chart">
        <div class="chart-y">
          <span>{{ axisTop }}</span>
          <span>{{ axisMid }}</span>
          <span>{{ axisBottom }}</span>
        </div>
        <div class="chart-main">
          <div class="chart-body" :style="{ minWidth: chartMinWidth }">
            <div class="chart-lines" aria-hidden="true">
              <span></span>
              <span></span>
              <span></span>
            </div>
            <div class="chart-bars" :style="{ gridTemplateColumns: chartColumns }">
              <div v-for="day in playtime.days" :key="day.date" class="chart-slot">
                <div class="chart-bar" :class="{ empty: day.seconds === 0 }"
                  :style="{ height: day.seconds > 0 ? barHeight(day.seconds) : undefined }" v-tip="barTip(day)"></div>
              </div>
            </div>
          </div>
          <div class="chart-x" :style="{ gridTemplateColumns: chartColumns, minWidth: chartMinWidth }">
            <span v-for="(day, index) in playtime.days" :key="day.date">{{ showXLabel(index) ? xLabel(day.date) : ""
            }}</span>
          </div>
        </div>
      </div>
      <div v-else-if="!playtimeLoaded" class="chart-empty">{{ t("winStats.loading") }}</div>
      <div v-else class="chart-empty">{{ t("winStats.chartEmpty") }}</div>
    </section>

    <table class="stats-table">
      <thead>
        <tr>
          <th>{{ t("winStats.colInstance") }}</th>
          <th>{{ t("winStats.colCount") }}</th>
          <th>{{ t("winStats.colFailed") }}</th>
          <th>{{ t("winStats.colHours") }}</th>
          <th>{{ t("winStats.colLast") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="inst in data?.instances ?? []" :key="inst.uuid">
          <td class="inst-cell">
            <InstanceIcon :name="inst.name" :uuid="inst.uuid" :size="30" />
            <span>{{ inst.name }}</span>
            <span v-if="inst.deleted" class="deleted-tag">{{ t("winStats.deleted") }}</span>
            <span v-else-if="inst.running" class="running-tag">{{ t("winStats.running") }}</span>
          </td>
          <td>{{ inst.count }}</td>
          <td :class="{ danger: inst.failed > 0 }">{{ inst.failed }}</td>
          <td>{{ hoursOf(inst.seconds) }}</td>
          <td>{{ fmtTime(inst.last) }}</td>
        </tr>
        <tr v-if="!data || data.instances.length === 0">
          <td colspan="5" class="empty-cell">{{ t("winStats.noData") }}</td>
        </tr>
      </tbody>
    </table>

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
.summary-num.danger,
.stats-table td.danger {
  color: var(--red);
}

.summary-label {
  font-size: 11.5px;
  color: var(--text-dim);
}

/* ---------- 游玩时间图 ---------- */

.play-panel {
  margin-bottom: 16px;
  padding: 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
}

.play-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  /* 筛选与数据之间留一道分隔：标题 / 筛选在上，统计数字与图表在下 */
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}

.play-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}

.play-sub {
  margin: 4px 0 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

.play-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.inst-select {
  /* 实例名 +「已删除」后缀比 220px 长，单行显示也放得下；再窄时由 controls.css
     的单行裁切兜底，不会把控件撑高 */
  width: 260px;
  min-width: 180px;
  max-width: 100%;
}

/* 自定义时间段：开关 + 文案；选 preset 时这一对保留，7 / 30 / 90 隐藏 */
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

.range-date {
  width: 140px;
}

.range-sep {
  color: var(--text-dim);
}

.play-stats {
  display: flex;
  gap: 24px;
  margin: 14px 0 12px;
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
  height: 150px;
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
  min-width: 0;
  /* 自定义范围很长时柱状区横向滚动，纵轴仍固定在左侧 */
  overflow-x: auto;
}

.chart-body {
  position: relative;
  height: 150px;
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
  gap: 2px;
}

.chart-slot {
  height: 100%;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}

.chart-bar {
  width: 100%;
  max-width: 34px;
  border-radius: 3px 3px 0 0;
  background: var(--accent);
  transition: height 0.3s, background-color 0.12s;
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
  gap: 2px;
  margin-top: 6px;
}

.chart-x span {
  font-size: 10.5px;
  color: var(--text-dim);
  text-align: center;
  white-space: nowrap;
}

/* 标签比柱子宽，居中会让首尾两个探出容器——那部分算进滚动宽度，
   于是 30 天也冒出一条横向滚动条，最后一个日期还被切掉。首尾贴边即可 */
.chart-x span:first-child {
  text-align: left;
}

.chart-x span:last-child {
  text-align: right;
}

.chart-empty {
  height: 150px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--border);
  border-radius: 10px;
  color: var(--text-dim);
  font-size: 12.5px;
}

/* ---------- 每实例汇总表 ---------- */

.stats-table {
  width: 100%;
  border-collapse: collapse;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.stats-table th {
  text-align: left;
  font-size: 11.5px;
  color: var(--text-dim);
  font-weight: 600;
  padding: 12px 16px;
  background: var(--bg-side);
  border-bottom: 1px solid var(--border);
}

.stats-table td {
  padding: 10px 16px;
  font-size: 12.5px;
  border-bottom: 1px solid var(--border);
  font-variant-numeric: tabular-nums;
}

.stats-table tr:last-child td {
  border-bottom: none;
}

.inst-cell {
  display: flex;
  align-items: center;
  gap: 10px;
  font-weight: 600;
}

.running-tag,
.deleted-tag {
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
}

.foot-note {
  font-size: 11.5px;
  color: var(--text-dim);
  margin-top: 10px;
}
</style>
