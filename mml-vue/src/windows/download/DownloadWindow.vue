<script setup lang="ts">
// 下载管理窗口：对接 mml_downloader
// - 状态快照来自 download_get_status（任务 + 线程 + 总体速度），按固定间隔轮询
// - 任务 / 文件状态事件触发立即刷新（进度本身靠轮询，避免高频重绘）
// - 每个任务可暂停 / 继续 / 停止
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { api, onDownloadItem, onDownloadTask } from "../../lib/api";
import {
  byProjectCount,
  downloadOverallPercent,
  downloadTaskPercent,
} from "../../lib/progress";
import { t } from "../../lib/i18n";
import { usePageActive } from "../../lib/pageActive";
import { isTauri, multiWindow } from "../windowManager";
import type { DownloadStatusDto, DownloadTaskDto } from "../../lib/bindings";

/** 请求关闭本窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

/** 本页面是否在前台：单窗口模式下切走只是被 KeepAlive 停用，见 lib/pageActive.ts */
const pageActive = usePageActive();

/** 轮询间隔，单位毫秒 */
const REFRESH_MS = 700;

/**
 * 外壳两用：多窗口模式下是独立窗口 `WindowFrame`；单窗口模式下是**悬浮弹窗** `BaseModal`，
 * 后者带遮罩、标题与右上角 ✕，浮在当前页面之上，不把页面换掉。
 * 两者同形，title + close 事件 + 默认插槽，所以下面那整块下载内容两种形态共用一份。
 * 四周留白随外壳走：独立窗口是 `.frame-body` 的 22/26，弹窗是 `BaseModal` 的 `.modal`
 * （上下 22、左右 15 再加两侧滚动条槽）—— 弹窗那一档是它自己的口径，不按页面外壳对齐。
 */
const shell = computed(() => (multiWindow.value ? WindowFrame : BaseModal));
const shellProps = computed(() =>
  multiWindow.value
    ? {
      title: t("features.download"),
      // 本窗口整页就是下载管理，标题栏不用再放一个下载指示器（整合包那个照留）
      hideDownloadIndicator: true,
      // 多窗口模式下两段列表都没有自己的高度约束，滚的是内容区本身，样式见 styles/scrollbar.css
      bodyGutter: true,
    }
    : {
      title: t("features.download"),
      // 线程行是固定列宽的网格，26 + 文件名 + 116 + 124 + 78 + 88，加间距与内边距约 612，
      // 700 刚好还给文件名留一截；再窄就被压成省略号了（独立窗口最小宽是 670）
      width: 700,
      // 锁死高度：线程表每 700ms 都在增删行，不锁的话弹窗会跟着一直长高变矮
      fixedHeight: "520px",
      // 贴着自绘标题栏下沿，别盖住窗口按钮
      belowTitlebar: true,
    },
);

const status = ref<DownloadStatusDto>({ tasks: [], threads: [], speed: 0, paused: false });
const loading = ref(true);

/** 停止下载确认弹窗 */
const confirmStop = ref(false);

let timer: number | null = null;
let unsubs: Array<() => void> = [];

const tasks = computed(() => [...status.value.tasks].sort((a, b) => a.id - b.id));
/** 线程列表：已完成的线程不占位 */
const threads = computed(() => status.value.threads.filter((x) => x.state !== "done"));

// ================= 进度计算 =================
// 口径见 lib/progress.ts：多文件按项目数、单文件按字节。总览与单任务、标题栏指示器共用它，
// 三处必须同源，否则同一个任务在列表和顶栏上会显示成两个进度

/** 总体进度：与单任务同一口径，汇总全部任务 */
const overall = computed(() => downloadOverallPercent(tasks.value));

/** 总体字节：已下载 / 总大小 */
const overallBytes = computed(() => ({
  now: tasks.value.reduce((n, x) => n + x.nowBytes, 0),
  all: tasks.value.reduce((n, x) => n + x.allBytes, 0),
}));

/** 文件计数汇总 */
const overallFiles = computed(() => ({
  done: tasks.value.reduce((n, x) => n + x.completed, 0),
  total: tasks.value.reduce((n, x) => n + x.total, 0),
}));

/** 计数文案：多文件任务数「项目」，单文件才数「文件」（与进度条同一口径） */
const countKey = computed(() =>
  byProjectCount(overallFiles.value.total) ? "winDownload.projects" : "winDownload.files",
);

/** 单个任务的计数文案（同 `countKey`） */
function taskCountKey(task: DownloadTaskDto): string {
  return byProjectCount(task.total) ? "winDownload.projects" : "winDownload.files";
}

// ================= 任务状态（标签文案 + 配色） =================

type TaskState = "running" | "paused" | "failed" | "done";

/** 任务状态：暂停优先，其次按是否全部结束、有无失败判定 */
function taskState(task: DownloadTaskDto): TaskState {
  if (task.paused) return "paused";
  if (task.total > 0 && task.completed + task.failed >= task.total) {
    return task.failed > 0 ? "failed" : "done";
  }
  return task.failed > 0 ? "failed" : "running";
}

// ================= 格式化 =================

/** 字节数转可读文本，100 以上不带小数 */
function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  const text = index === 0 || value >= 100 ? value.toFixed(0) : value.toFixed(1);
  return `${text} ${units[index]}`;
}

/** 速度：无速度时显示占位符 */
function formatSpeed(speed: number): string {
  return speed > 0 ? `${formatBytes(speed)}/s` : "—";
}

/** 已进行时间：不足 1 小时为 mm:ss，否则 h:mm:ss */
function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const hour = Math.floor(total / 3600);
  const minute = Math.floor((total % 3600) / 60);
  const second = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return hour > 0 ? `${hour}:${pad(minute)}:${pad(second)}` : `${pad(minute)}:${pad(second)}`;
}

/** 大小文本：总大小未知时只显示已下载 */
function sizeText(now: number, all: number): string {
  return all > 0 ? `${formatBytes(now)} / ${formatBytes(all)}` : formatBytes(now);
}

/** 线程状态 → 标签文案，键为 winDownload.state.* */
function stateLabel(state: string): string {
  return t(`winDownload.state.${state}`);
}

// ================= 数据刷新 =================

/** 是否出现过任务：任务清空时据此判断下载已结束并自动关窗 */
let hadTasks = false;
/** 用户主动停止后本次不自动关窗（停完可能还想看状态） */
let keepOpen = false;
/** 上一轮任务里是否有失败：有失败就不自动关窗，留着让用户看到 */
let lastFailed = false;

/** 上一轮刷新是否报过 IPC 错误：每段故障只报一次，别让 700ms 的轮询刷屏 */
let ipcErrorReported = false;

/** 拉一次状态快照，任务全部结束时按需自动关窗 */
async function refresh() {
  try {
    status.value = await api.getDownloadStatus();
    // 拉到了说明这段故障过去了，下次再坏还值得报一条
    ipcErrorReported = false;
  } catch (err) {
    // 纯浏览器模式下本来就没有 IPC，静默保持空状态；真接上后端还失败就得留下痕迹，
    // 否则界面一直显示"所有下载线程空闲"，日志里什么都没有
    if (isTauri() && !ipcErrorReported) {
      ipcErrorReported = true;
      console.warn("download_get_status 失败，下载状态无法刷新", err);
    }
  } finally {
    loading.value = false;
  }

  if (status.value.tasks.length > 0) {
    hadTasks = true;
    keepOpen = false;
    lastFailed = status.value.tasks.some((x) => x.failed > 0);
    return;
  }
  // 任务全部结束（下载完成后清空）：自动关窗，不用手动点关闭；
  // 有失败或用户主动停止时保留窗口。
  // 页面已切走时不发这条关闭，见 lib/pageActive.ts：此刻"关窗"会被当成关掉当前页面，
  // 而在主页面就是退出启动器 —— 轮询虽已停，收尾那一发仍可能落到这里
  if (hadTasks && !keepOpen && !lastFailed) {
    hadTasks = false;
    if (pageActive.value) emit("close");
  }
}

/** 是否已全部暂停（后端全局状态） */
const allPaused = computed(() => status.value.paused);
/** 是否有任务可操作（决定全局按钮是否可用） */
const hasTasks = computed(() => tasks.value.length > 0);

async function pauseAll() {
  await api.pauseAllDownloads();
  await refresh();
}

async function resumeAll() {
  await api.resumeAllDownloads();
  await refresh();
}

/** 停止全部下载，并抑制这次的自动关窗 */
async function stopAll() {
  keepOpen = true;
  await api.stopAllDownloads();
  await refresh();
}

/** 点停止：先二次确认 */
function requestStop() {
  confirmStop.value = true;
}

/** 确认停止所有下载 */
async function confirmStopAll() {
  confirmStop.value = false;
  await stopAll();
}

/**
 * 点关闭：直接收起弹窗 / 关掉窗口，不打断下载
 *
 * 单窗口模式下它只是浮在当前页上的一层壳，收起 ≠ 停止下载：任务照跑，
 * 顶栏的入口 DownloadTitleIndicator 还在。真正会连带停掉下载的是"关下载窗口"和
 * "退出启动器"，那两种情况由 Rust 侧的关闭保护拦下，再由根组件弹确认框，
 * 见 src-tauri 的 close_guarded 与 App.vue
 */
function requestClose() {
  emit("close");
}

/** 轮询开关：单窗口模式下窗口会被 KeepAlive 缓存，切走必须停掉，否则 700ms 一直空转 */
function startTimer() {
  if (timer === null) {
    timer = window.setInterval(refresh, REFRESH_MS);
  }
}

function stopTimer() {
  if (timer !== null) {
    window.clearInterval(timer);
    timer = null;
  }
}

onMounted(async () => {
  await refresh();

  unsubs.push(await onDownloadTask(() => void refresh()));
  unsubs.push(await onDownloadItem(() => void refresh()));
  // 关闭被 Rust 拦下（close-blocked）不在这里处理：关下载窗口 / 退出应用
  // 共用根组件那一个确认框（App.vue）

  startTimer();
});

// 切回本窗口先补一次数据，再继续轮询
useWindowRefresh(refresh);
onActivated(startTimer);
onDeactivated(stopTimer);

onUnmounted(() => {
  stopTimer();
  unsubs.forEach((fn) => fn());
  unsubs = [];
});

</script>

<template>
  <component :is="shell" v-bind="shellProps" @close="requestClose">
    <div class="download-body" :class="{ fill: !multiWindow }">
      <!-- 总览：任务总量 / 文件数 / 大小 / 总体速度 + 总体进度条 -->
      <div class="overview">
        <div class="overview-head">
          <span class="ov-item">{{ t("winDownload.taskCount", { n: tasks.length }) }}</span>
          <span class="ov-item">
            {{ t(countKey, { done: overallFiles.done, total: overallFiles.total }) }}
          </span>
          <span v-if="overallBytes.all > 0" class="ov-item">
            {{ sizeText(overallBytes.now, overallBytes.all) }}
          </span>
          <span class="ov-speed">
            <span class="ov-speed-label">{{ t("winDownload.totalSpeed") }}</span>
            <span class="ov-speed-value" :class="{ idle: status.speed <= 0 }">
              {{ formatSpeed(status.speed) }}
            </span>
          </span>
          <!-- 全局控制：暂停 / 继续 / 停止（作用于所有下载任务） -->
          <span class="ov-actions">
            <button v-if="!allPaused" class="mini-btn" :disabled="!hasTasks" @click="pauseAll">
              {{ t("winDownload.pause") }}
            </button>
            <button v-else class="mini-btn primary" :disabled="!hasTasks" @click="resumeAll">
              {{ t("winDownload.resume") }}
            </button>
            <button class="mini-btn danger" :disabled="!hasTasks" @click="requestStop">
              {{ t("winDownload.stop") }}
            </button>
          </span>
        </div>
        <div class="ov-progress">
          <span class="progress-track ov-track">
            <span class="progress-fill" :style="{ width: overall + '%' }" />
          </span>
          <span class="ov-percent">{{ overall.toFixed(1) }}%</span>
        </div>
      </div>

      <!-- 下载任务：只有一个任务时隐藏（信息与下方线程进度重复） -->
      <div v-if="tasks.length !== 1" class="section tasks">
        <div class="section-head">
          <span class="section-title">{{ t("winDownload.tasks") }}</span>
        </div>
        <div v-if="loading" class="empty">{{ t("winDownload.loading") }}</div>
        <div v-else-if="tasks.length === 0" class="empty">{{ t("winDownload.empty") }}</div>
        <div v-else class="task-list">
          <div v-for="task in tasks" :key="task.id" class="task-item">
            <div class="task-head">
              <span class="task-id">#{{ task.id }}</span>
              <span class="state-tag" :class="'ts-' + taskState(task)">
                {{ t(`winDownload.taskState.${taskState(task)}`) }}
              </span>
              <span class="task-meta">
                {{ t(taskCountKey(task), { done: task.completed, total: task.total }) }}
              </span>
              <span class="task-meta right">
                <template v-if="task.failed > 0">{{ t("winDownload.failed", { n: task.failed }) }}</template>
              </span>
              <span class="task-meta right">{{ sizeText(task.nowBytes, task.allBytes) }}</span>
              <span class="task-meta right">
                {{ t("winDownload.elapsed", { time: formatDuration(task.elapsedMs) }) }}
              </span>
            </div>
            <div class="task-foot">
              <span class="progress-track">
                <span class="progress-fill" :class="{ failed: task.failed > 0, paused: task.paused }"
                  :style="{ width: downloadTaskPercent(task) + '%' }" />
              </span>
              <span class="task-percent">{{ downloadTaskPercent(task).toFixed(1) }}%</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 下载线程：当前文件进度 + 速度 -->
      <div class="section threads">
        <div class="section-head">
          <span class="section-title">{{ t("winDownload.threads") }}</span>
        </div>
        <div v-if="threads.length === 0" class="empty">{{ t("winDownload.idle") }}</div>
        <div v-else class="thread-list">
          <div v-for="item in threads" :key="item.thread" class="thread-item">
            <span class="thread-id">#{{ item.thread + 1 }}</span>
            <span class="thread-name" v-tip="item.name">{{ item.name }}</span>
            <span class="thread-progress">
              <span class="progress-track">
                <span class="progress-fill" :style="{ width: item.progress + '%' }" />
              </span>
              <span class="thread-percent">{{ item.progress.toFixed(1) }}%</span>
            </span>
            <span class="thread-size">{{ sizeText(item.nowBytes, item.allBytes) }}</span>
            <span class="thread-speed" :class="{ idle: item.speed <= 0 }">
              {{ formatSpeed(item.speed) }}
            </span>
            <span class="state-tag" :class="'state-' + item.state">{{ stateLabel(item.state) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 停止下载确认 -->
    <BaseModal v-if="confirmStop" :title="t('winDownload.stopTitle')" :closable="false" @close="confirmStop = false">
      <p class="delete-tip">{{ t("winDownload.stopConfirm") }}</p>
      <div class="modal-actions">
        <BaseButton @click="confirmStop = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="confirmStopAll">
          {{ t("winDownload.stop") }}
        </BaseButton>
      </div>
    </BaseModal>

  </component>
</template>

<style scoped>
.download-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* ---------- 弹窗形态：把锁死的高度分下去 ----------
   面板高度由 shellProps 的 fixedHeight 定死，这里让两段列表各自滚。
   不这么切的话，线程行的增删会一路顶到面板上，弹窗高度就跟着抽动 */
.download-body.fill {
  flex: 1;
  min-height: 0;
}

/* 任务区：最多这么高，多了自己滚（任务数变化不再撑高面板） */
.download-body.fill .section.tasks {
  min-height: 0;
}

.download-body.fill .task-list {
  max-height: 168px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行不往左缩（槽位由上面的 scrollbar-gutter 常驻），滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

/* 线程区：吃掉剩余高度，行数变化只影响它自己的滚动条 */
.download-body.fill .section.threads {
  flex: 1;
  min-height: 0;
}

.download-body.fill .thread-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行不往左缩（槽位由上面的 scrollbar-gutter 常驻），滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

/* ---------- 总览 ---------- */
.overview {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 10px 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
}

.overview-head {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 12.5px;
  color: var(--text-dim);
}

.ov-item:first-child {
  color: var(--text);
  font-weight: 600;
}

.ov-speed {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

/* 全局控制按钮（暂停 / 继续 / 停止） */
.ov-actions {
  display: flex;
  gap: 6px;
}

.ov-speed-label {
  font-size: 11.5px;
}

.ov-speed-value {
  font-size: 13px;
  font-weight: 700;
  color: var(--green, #4caf7d);
  font-variant-numeric: tabular-nums;
}

.ov-speed-value.idle {
  color: var(--text-dim);
  font-weight: 500;
}

/* 总体进度：进度条 + 右侧百分比 */
.ov-progress {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ov-percent {
  flex-shrink: 0;
  width: 48px;
  text-align: right;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.ov-track {
  height: 8px;
}

/* ---------- 分区 ---------- */
.section {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-title {
  font-size: 12.5px;
  font-weight: 700;
  color: var(--text-dim);
}

.empty {
  padding: 16px;
  background: var(--bg-card);
  border: 1px dashed var(--border);
  border-radius: 10px;
  text-align: center;
  font-size: 12.5px;
  color: var(--text-dim);
}

/* ---------- 任务 ---------- */
.task-list,
.thread-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.task-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
}

/* 任务行：数字/时间列贴右，多余宽度只留在「文件」列（表格常规做法） */
.task-head {
  display: grid;
  grid-template-columns: 26px 86px minmax(0, 1fr) 54px 122px 100px;
  gap: 8px;
  align-items: center;
  font-size: 12px;
}

.task-id {
  font-weight: 700;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}

.task-meta {
  color: var(--text-dim);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-variant-numeric: tabular-nums;
}

.task-meta.right {
  text-align: right;
}

.mini-btn {
  height: var(--ctl-h);
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: var(--bg-raised);
  color: var(--text);
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.15s;
}

.mini-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  pointer-events: none;
}

.mini-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.mini-btn.primary {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-soft);
}

.mini-btn.danger:hover {
  border-color: var(--red);
  color: var(--red);
}

.task-foot {
  display: flex;
  align-items: center;
  gap: 8px;
}

.task-percent {
  flex-shrink: 0;
  width: 46px;
  text-align: right;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ---------- 进度条 ---------- */
.progress-track {
  flex: 1;
  display: block;
  height: 6px;
  border-radius: 3px;
  background: var(--bg-hover);
  overflow: hidden;
}

.progress-fill {
  display: block;
  height: 100%;
  border-radius: 3px;
  background: var(--accent-grad);
  transition: width 0.3s;
}

.progress-fill.failed {
  background: var(--red);
}

.progress-fill.paused {
  background: var(--yellow, #f5b944);
}

/* ---------- 线程 ---------- */
/* 线程行：固定列宽，信息列紧邻；状态列放得下最长的标签 "获取信息" / "Fetching Info"，
   大小 / 速度列按最宽的 "41.4 MB / 41.4 MB"、"12.3 MB/s" 留宽，数字列一律不换行 */
.thread-item {
  display: grid;
  grid-template-columns: 26px minmax(0, 1fr) 116px 124px 78px 88px;
  gap: 8px;
  align-items: center;
  padding: 6px 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 9px;
  font-size: 12px;
}

.thread-id {
  font-weight: 600;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.thread-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thread-progress {
  display: flex;
  align-items: center;
  gap: 7px;
}

.thread-percent {
  flex-shrink: 0;
  width: 40px;
  text-align: right;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.thread-size {
  text-align: right;
  font-size: 11.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.thread-speed {
  text-align: right;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--green, #4caf7d);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.thread-speed.idle {
  color: var(--text-dim);
  font-weight: 500;
}

/* ---------- 状态标签（多色） ---------- */
.state-tag {
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 11px;
  white-space: nowrap;
  text-align: center;
  overflow: hidden;
  text-overflow: ellipsis;
  background: var(--bg-hover);
  color: var(--text-dim);
}

/* 任务状态 */
.ts-running {
  color: #6aa8ff;
  background: rgba(63, 140, 255, 0.16);
}

.ts-paused {
  color: var(--yellow, #f5b944);
  background: rgba(245, 185, 68, 0.16);
}

.ts-failed {
  color: #ff8080;
  background: rgba(255, 90, 90, 0.16);
}

.ts-done {
  color: #5ecf99;
  background: rgba(76, 175, 125, 0.16);
}

/* 线程状态 */
.state-download {
  color: #6aa8ff;
  background: rgba(63, 140, 255, 0.16);
}

.state-getinfo {
  color: #4fd1e0;
  background: rgba(6, 182, 212, 0.16);
}

.state-init {
  color: #b98cff;
  background: rgba(168, 85, 247, 0.16);
}

.state-action {
  color: #ffab5c;
  background: rgba(255, 150, 56, 0.16);
}

.state-pause {
  color: var(--yellow, #f5b944);
  background: rgba(245, 185, 68, 0.16);
}

.state-wait {
  color: var(--text-dim);
  background: var(--bg-hover);
}

.state-done {
  color: #5ecf99;
  background: rgba(76, 175, 125, 0.16);
}

.state-error {
  color: #ff8080;
  background: rgba(255, 90, 90, 0.16);
}

/* 取消不是出错：用与"等待中"同一套弱化色区分开 */
.state-cancel {
  color: var(--text-dim);
  background: var(--bg-hover);
}
</style>
