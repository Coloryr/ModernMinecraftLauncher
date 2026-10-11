<script setup lang="ts">
// 下载任务：标题栏上的动态图标 + 细进度条（点击展开下载管理）
//
// 原来是右下角的浮层卡片（DownloadChip），会一直占着页面右下角那块地方；
// 改成标题栏指示器后不占内容区，位置也和整合包那个 ModpackTitleIndicator 并排。
//
// 挂载范围与原来的卡片一致：**只挂单窗口模式的主窗口顶栏**。多窗口模式下下载管理
// 是独立窗口、自己有整页进度，标题栏再来一个就是重复。
// 点击走 openWindow("download")：单窗口模式它会被特判成弹窗，多窗口模式开真实窗口。
//
// "动态"的落点：下载时图标**不转**，靠强调色 + 下方细条表达进度，
// 与整合包那个同一套口径 —— 两个并排都不转，不至于晃眼。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "../lib/i18n";
import { api, onDownloadItem, onDownloadTask } from "../lib/api";
import { downloadOverallPercent } from "../lib/progress";
import { openWindow } from "../windows/windowManager";
import type { DownloadStatusDto, DownloadTaskDto } from "../lib/bindings";

const status = ref<DownloadStatusDto>({ tasks: [], threads: [], speed: 0, paused: false });

/** 拉一次任务快照；失败时保留上次数据，入口闪一下比不更新更糟 */
async function refresh() {
  try {
    status.value = await api.getDownloadStatus();
  } catch {
    /* 忽略：后端不可用时保留上次显示 */
  }
}

const count = computed(() => status.value.tasks.length);

/**
 * 任务是否还在下，与下载窗口的 taskState 同一口径
 *
 * 暂停不算、已下完不算、有失败也不算 —— 那几种都不该让图标一直转。
 */
function isDownloading(task: DownloadTaskDto): boolean {
  if (task.paused) return false;
  if (task.total > 0 && task.completed + task.failed >= task.total) return false;
  return task.failed === 0;
}

/** 还没下完；暂停也算，进度条照显示，只是图标不转 */
function isIncomplete(task: DownloadTaskDto): boolean {
  return task.total === 0 || task.completed + task.failed < task.total;
}

/** 是否仍在下（决定图标转不转） */
const busy = computed(() => status.value.tasks.some(isDownloading));

/** 进度条显示与否：只要还有没下完的（含暂停）就显示，别让暂停时进度凭空消失 */
const showBar = computed(() => status.value.tasks.some(isIncomplete));

/** 有失败的任务（图标变警示色） */
const hasFailed = computed(() => status.value.tasks.some((task) => task.failed > 0));

/** 总体进度，与下载窗口同一口径；见 lib/progress.ts，多文件按项目数、单文件按字节 */
const percent = computed(() => downloadOverallPercent(status.value.tasks));

const tipText = computed(() =>
  busy.value
    ? `${t("winDownload.taskCount", { n: count.value })} · ${Math.round(percent.value)}%`
    : t("winDownload.taskCount", { n: count.value }),
);

/** 图标状态：下载中转、失败警示、结束常态 */
const iconState = computed(() => {
  if (busy.value) return "busy";
  return hasFailed.value ? "failed" : "done";
});

let unsubs: Array<() => void> = [];

onMounted(async () => {
  await refresh();
  // 任务 / 文件状态变化都会广播，跟着刷新即可（进度本身也是事件驱动的）
  unsubs.push(await onDownloadTask(() => void refresh()));
  unsubs.push(await onDownloadItem(() => void refresh()));
});

onUnmounted(() => {
  unsubs.forEach((fn) => fn());
  unsubs = [];
});
</script>

<template>
  <!-- 没有任务时整个入口不存在；任务全部结束后后端会清表，它自己就消失了 -->
  <button v-if="count > 0" class="dl-indicator" :class="iconState" v-tip="tipText" :aria-label="t('features.download')"
    @click="openWindow('download')">
    <span class="dl-ind-icon">
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.9"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3v12" />
        <path d="m7 12 5 5 5-5" />
        <path d="M4 21h16" />
      </svg>
    </span>
    <!-- 进度：细条贴在图标下沿。还有没下完的就显示（结束是满格，没信息量） -->
    <span v-if="showBar" class="dl-ind-track">
      <span class="dl-ind-fill" :style="{ width: percent + '%' }" />
    </span>
  </button>
</template>

<style scoped>
.dl-indicator {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  /* 与顶栏其它入口按钮 .topbar-icon-btn 同规格：并排时行高与间距才对得上 */
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 10px;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

.dl-indicator:hover {
  background: var(--bg-card);
  border-color: var(--border);
}

/* 下载中：不旋转，靠颜色（强调色）+ 下方进度条表达"在动"。
   两个指示器并排，都不转 —— 旋转容易晃眼，而且容易让人以为点了会有别的动作 */
.dl-indicator.busy {
  color: var(--accent);
}

.dl-indicator.failed {
  color: var(--red);
}

.dl-indicator.done {
  color: var(--text-dim);
}

.dl-ind-icon {
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 进度条：压在图标下方那条 2px 的细线 */
.dl-ind-track {
  position: absolute;
  left: 6px;
  right: 6px;
  bottom: 4px;
  height: 2px;
  border-radius: 1px;
  background: var(--bg-hover);
  overflow: hidden;
}

.dl-ind-fill {
  display: block;
  height: 100%;
  border-radius: 1px;
  background: currentColor;
  transition: width 0.3s ease;
}
</style>
