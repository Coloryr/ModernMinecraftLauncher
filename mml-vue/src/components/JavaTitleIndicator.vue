<script setup lang="ts">
// Java 安装进度：标题栏上的图标 + 细进度条，点开弹出安装任务列表
//
// 和整合包 / 资源下载两个指示器同一套做法：任务状态由 App.vue 统一订阅，这里只读。
import { computed } from "vue";
import { t } from "../lib/i18n";
import { isRunning, javaTasks, overallPercent } from "../lib/javaTasks";
import { openJavaPopup } from "../windows/windowManager";

/** 进行中的任务 */
const running = computed(() => javaTasks.value.filter(isRunning));

/** 有没有失败的任务，图标据此变警示色 */
const hasFailed = computed(() => javaTasks.value.some((task) => task.stage === "failed"));

/** 总进度 */
const percent = computed(() => overallPercent(javaTasks.value));

/** 悬停提示：进行中报进度，结束后报结果 */
const tipText = computed(() =>
  running.value.length
    ? t("winJavaDownload.running", { count: running.value.length }) + " · " + Math.round(percent.value) + "%"
    : t("winJavaDownload.finished", { count: javaTasks.value.length }),
);

/** 图标状态 */
const iconState = computed(() => {
  if (running.value.length) return "busy";
  return hasFailed.value ? "failed" : "done";
});
</script>

<template>
  <button v-if="javaTasks.length" class="jv-indicator" :class="iconState" v-tip="tipText"
    :aria-label="tipText" @click="openJavaPopup()">
    <span class="jv-ind-icon">
      <!-- 咖啡杯：Java 的通用记号 -->
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.9"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M17 8h1a4 4 0 1 1 0 8h-1" />
        <path d="M3 8h14v6a5 5 0 0 1-5 5H8a5 5 0 0 1-5-5Z" />
        <path d="M9 2v3M13 2v3" />
      </svg>
    </span>
    <span v-if="running.length" class="jv-ind-track">
      <span class="jv-ind-fill" :style="{ width: percent + '%' }" />
    </span>
  </button>
</template>

<style scoped>
/* 尺寸与圆角沿用标题栏图标键的观感，这里只写字形与进度条 */
.jv-indicator {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.jv-indicator:hover {
  background: var(--bg-hover);
}

/* 失败：图标转警示色 */
.jv-indicator.failed {
  color: var(--red);
}

/* 进度细条贴在图标下沿，只在进行中显示 */
.jv-ind-track {
  position: absolute;
  left: 4px;
  right: 4px;
  bottom: 2px;
  height: 2px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.jv-ind-fill {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.15s;
}
</style>
