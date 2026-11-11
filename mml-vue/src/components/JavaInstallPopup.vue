<script setup lang="ts">
// Java 安装进度弹窗：入口是标题栏上的 JavaTitleIndicator，关掉只收起、安装照跑
//
// 与资源下载、整合包安装共用一个 TaskProgressPopup；Java 没有项目封面，
// 任务行统一用 Java 图标，进度与状态照常按阶段显示。
import { computed } from "vue";
import TaskProgressPopup from "./TaskProgressPopup.vue";
import javaIcon from "../assets/java.svg";
import { commands, type JavaInstallTaskDto } from "../lib/bindings";
import { t, tErr } from "../lib/i18n";
import { isRunning } from "../lib/javaTasks";
import { showToast } from "../lib/toast";
import type { TaskProgressRow, TaskStatusKind } from "../lib/taskProgress";

const props = defineProps<{
  /** 安装任务，来自 App.vue 那份全局订阅 */
  tasks: JavaInstallTaskDto[];
}>();

const emit = defineEmits<{
  /** 关闭弹窗；安装不受影响 */
  (e: "close"): void;
}>();

/** 各阶段文案 */
const STAGE_KEYS: Record<JavaInstallTaskDto["stage"], string> = {
  download: "winJavaDownload.stageDownload",
  extract: "winJavaDownload.stageExtract",
  probe: "winJavaDownload.stageProbe",
  done: "winJavaDownload.stageDone",
  failed: "winJavaDownload.stageFailed",
  cancelled: "winJavaDownload.stageCancelled",
};

/**
 * 阶段文案
 *
 * - `stage`: 安装阶段
 */
function stageLabel(stage: JavaInstallTaskDto["stage"]): string {
  return t(STAGE_KEYS[stage]);
}

/**
 * 阶段对应的行状态
 *
 * - `stage`: 安装阶段
 *
 * # 返回值
 *
 * 进行中的三个阶段为 running，其余原样作为终态
 */
function statusKindOf(stage: JavaInstallTaskDto["stage"]): TaskStatusKind {
  if (stage === "done" || stage === "failed" || stage === "cancelled") {
    return stage;
  }
  return "running";
}

/** 进行中的任务 */
const runningTasks = computed(() => props.tasks.filter((task) => isRunning(task)));

/** 归一化成共享弹窗的行 */
const rows = computed<TaskProgressRow[]>(() =>
  props.tasks.map((task) => {
    const active = isRunning(task);
    const percent = task.stage === "done" ? 100 : Math.round(task.progress * 100);
    return {
      key: task.id,
      name: task.name,
      icon: javaIcon,
      percent,
      statusText: active
        ? `${stageLabel(task.stage)} ${percent}%`
        : stageLabel(task.stage),
      statusKind: statusKindOf(task.stage),
      error: task.error,
      subPercent: null,
      cancelable: active,
    };
  }),
);

/** 标题下的任务数 */
const headText = computed(() =>
  runningTasks.value.length
    ? t("winJavaDownload.running", { count: runningTasks.value.length })
    : t("winJavaDownload.finished", { count: props.tasks.length }),
);

/** 总进度：多个任务同时装时取各任务进度的均值 */
const overallPercent = computed(() =>
  runningTasks.value.length > 1
    ? runningTasks.value.reduce((sum, task) => sum + task.progress * 100, 0) /
    runningTasks.value.length
    : null,
);

/** 有终态任务时才给「清除已完成」 */
const clearLabel = computed(() =>
  props.tasks.some((task) => !isRunning(task)) ? t("winJavaDownload.clearDone") : null,
);

/**
 * 取消一条安装
 *
 * - `key`: 任务 id
 */
async function cancel(key: string) {
  try {
    await commands.javaDownload.cancel(key);
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 清掉已结束的任务 */
async function clear() {
  try {
    await commands.javaDownload.clear();
  } catch (e) {
    showToast(tErr(e));
  }
}
</script>

<template>
  <TaskProgressPopup :title="t('winJavaDownload.tasks')" :head-text="headText" :rows="rows"
    :overall-percent="overallPercent" :clear-label="clearLabel" @close="emit('close')"
    @cancel="cancel" @clear="clear" />
</template>
