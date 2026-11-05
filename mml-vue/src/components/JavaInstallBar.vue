<script setup lang="ts">
// Java 安装任务列表：和整合包 / 资源下载的进度条同一套外观
//
// 一条任务三行信息：名字 + 阶段、进度条、失败原因。进行中的任务可以取消，
// 结束的任务可以一键清理；关掉弹窗不影响安装。
import { computed } from "vue";
import { t, tErr } from "../lib/i18n";
import { isRunning } from "../lib/javaTasks";
import { commands } from "../lib/bindings";
import { showToast } from "../lib/toast";
import BaseButton from "./ui/BaseButton.vue";
import type { JavaInstallTaskDto } from "../lib/bindings";

const props = defineProps<{
  /** 安装任务，来自 java-install-status 事件 */
  tasks: JavaInstallTaskDto[];
}>();

/** 进行中的数量 */
const running = computed(() => props.tasks.filter(isRunning));

/** 已经结束、可以清理的任务 */
const finished = computed(() => props.tasks.filter((task) => !isRunning(task)));

/** 标题行文案 */
const headText = computed(() =>
  running.value.length
    ? t("winJavaDownload.running", { count: running.value.length })
    : t("winJavaDownload.finished", { count: props.tasks.length }),
);

/** 阶段文字 */
const STAGE_KEYS: Record<JavaInstallTaskDto["stage"], string> = {
  download: "winJavaDownload.stageDownload",
  extract: "winJavaDownload.stageExtract",
  probe: "winJavaDownload.stageProbe",
  done: "winJavaDownload.stageDone",
  failed: "winJavaDownload.stageFailed",
  cancelled: "winJavaDownload.stageCancelled",
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

/**
 * 取消一条进行中的任务
 *
 * 安装流程自己收手，结果与阶段变化走 java-install-status 事件回来。
 *
 * - `id`: 任务的稳定包 id
 */
async function cancelTask(id: string) {
  try {
    await commands.javaDownload.cancel(id);
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 清掉已经结束的任务；后端广播新列表，弹窗会跟着收起 */
async function clearDone() {
  try {
    await commands.javaDownload.clear();
  } catch (e) {
    showToast(tErr(e));
  }
}
</script>

<template>
  <div class="jb">
    <div class="jb-head">
      <span>{{ headText }}</span>
      <BaseButton v-if="finished.length" size="sm" @click="clearDone">
        {{ t("winJavaDownload.clearDone") }}
      </BaseButton>
    </div>
    <ul class="jb-list">
      <li v-for="task in tasks" :key="task.id" class="jb-item" :class="task.stage">
        <div class="jb-top">
          <span class="jb-name" :title="task.name">{{ task.name }}</span>
          <span class="jb-stage">{{ stageLabel(task.stage) }}</span>
          <span class="jb-pct">{{ percent(task.progress) }}%</span>
          <BaseButton v-if="isRunning(task)" size="sm" @click="cancelTask(task.id)">
            {{ t("winJavaDownload.cancel") }}
          </BaseButton>
        </div>
        <div class="progress-track">
          <div class="progress-fill" :style="{ width: percent(task.progress) + '%' }" />
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
  flex: 1;
  gap: 8px;
  min-height: 0;
}

.jb-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 12.5px;
  font-weight: 600;
}

/* 按钮是子组件根部，scoped 规则碰不到；长任务名不能把按钮挤扁 */
.jb-head :deep(.ui-btn),
.jb-top :deep(.ui-btn) {
  flex-shrink: 0;
}

.jb-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  gap: 10px;
  min-height: 0;
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
  font-size: 12.5px;
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

/* 死板一条 6px 细条，与表格行里的进度条同档 */
.progress-track {
  margin-top: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.15s;
}

.jb-item.done .progress-fill {
  background: var(--green);
}

.jb-item.failed .progress-fill {
  background: var(--red);
}

/* 取消后进度条不再有推进意义，灰掉 */
.jb-item.cancelled .progress-fill {
  background: var(--text-dim);
}

.jb-item.cancelled .jb-name {
  color: var(--text-dim);
}

.jb-error {
  margin: 6px 0 0;
  color: var(--red);
  font-size: 11.5px;
}
</style>
