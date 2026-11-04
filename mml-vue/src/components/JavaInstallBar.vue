<script setup lang="ts">
// Java 安装任务列表：和整合包 / 资源下载的进度条同一套外观
//
// 一条任务三行信息：名字 + 阶段、进度条、失败原因。批量安装时多条并排。
import { computed } from "vue";
import { t, tErr } from "../lib/i18n";
import { isRunning } from "../lib/javaTasks";
import type { JavaInstallTaskDto } from "../lib/bindings";

const props = defineProps<{
  /** 安装任务，来自 java-install-status 事件 */
  tasks: JavaInstallTaskDto[];
}>();

/** 进行中的数量 */
const running = computed(() => props.tasks.filter(isRunning).length);

/** 标题行文案 */
const headText = computed(() =>
  running.value
    ? t("winJavaDownload.running", { count: running.value })
    : t("winJavaDownload.finished", { count: props.tasks.length }),
);

/** 阶段文字 */
const STAGE_KEYS: Record<JavaInstallTaskDto["stage"], string> = {
  download: "winJavaDownload.stageDownload",
  extract: "winJavaDownload.stageExtract",
  probe: "winJavaDownload.stageProbe",
  done: "winJavaDownload.stageDone",
  failed: "winJavaDownload.stageFailed",
};

/**
 * 阶段文字
 *
 * - `stage`: 后端阶段
 */
function stageLabel(stage: JavaInstallTaskDto["stage"]): string {
  return t(STAGE_KEYS[stage]);
}

/**
 * 进度百分比，取整
 *
 * - `value`: 0.0 到 1.0
 */
function percent(value: number): number {
  return Math.round(Math.min(1, Math.max(0, value)) * 100);
}
</script>

<template>
  <div class="jb">
    <div class="jb-head">{{ headText }}</div>
    <ul class="jb-list">
      <li v-for="task in tasks" :key="task.id" class="jb-item" :class="task.stage">
        <div class="jb-top">
          <span class="jb-name" :title="task.name">{{ task.name }}</span>
          <span class="jb-stage">{{ stageLabel(task.stage) }}</span>
          <span class="jb-pct">{{ percent(task.progress) }}%</span>
        </div>
        <div class="jb-track">
          <div class="jb-fill" :style="{ width: percent(task.progress) + '%' }" />
        </div>
        <p v-if="task.error" class="jb-error">{{ tErr(task.error) }}</p>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.jb {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}

.jb-head {
  font-size: 12.5px;
  font-weight: 600;
}

.jb-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
  overflow-y: auto;
}

.jb-item {
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-card);
}

.jb-top {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.jb-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.jb-stage,
.jb-pct {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}

.jb-track {
  margin-top: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.jb-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.15s;
}

.jb-item.done .jb-fill {
  background: var(--green);
}

.jb-item.failed .jb-fill {
  background: var(--red);
}

.jb-error {
  margin: 6px 0 0;
  color: var(--red);
  font-size: 11.5px;
}
</style>
