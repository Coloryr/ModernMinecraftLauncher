<script setup lang="ts">
// 下载任务：标题栏上的动态图标 + 细进度条（点击展开下载管理）
//
// 原来是右下角的浮层卡片（DownloadChip），会一直占着页面右下角那块地方；
// 改成标题栏指示器后不占内容区，位置也和整合包那个 ModpackTitleIndicator 并排。
//
// 挂载范围：主窗口顶栏（MainTopbar）+ 会发起 / 观看下载的那几个功能窗口的标题栏
// （WindowFrame 的 showTaskIndicators：下载整合包 / 资源管理 / 实例日志）。
// 没有任务时整块不存在，所以挂多了也不占位。
// 点击走 openWindow("download")：单窗口模式它会被特判成弹窗，多窗口模式开真实窗口。
//
// "动态"的落点：下载时图标**不转**，靠强调色 + 下方细条表达进度，
// 与整合包那个同一套口径 —— 两个并排都不转，不至于晃眼。
import { computed } from "vue";
import { t } from "../lib/i18n";
import { useDownloadProgress } from "../composables/useDownloadProgress";
import { openWindow } from "../windows/windowManager";
import type { DownloadTaskDto } from "../lib/bindings";

const { tasks, percent } = useDownloadProgress();

const count = computed(() => tasks.value.length);

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
const busy = computed(() => tasks.value.some(isDownloading));

/** 进度条显示与否：只要还有没下完的（含暂停）就显示，别让暂停时进度凭空消失 */
const showBar = computed(() => tasks.value.some(isIncomplete));

/** 有失败的任务（图标变警示色） */
const hasFailed = computed(() => tasks.value.some((task) => task.failed > 0));

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
  /* 尺寸与圆角见 styles/titlebar.css 的方形图标键，这里只留外观 */
  padding: 0;
  border: 1px solid transparent;
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
