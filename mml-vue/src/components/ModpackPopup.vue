<script setup lang="ts">
// 整合包安装进度弹窗：右下角 chip 点开后浮在当前页面之上
//
// 与 Java 安装、资源下载共用一个 TaskProgressPopup；整合包任务带阶段与子进度，
// 这里把它们归一化成同一份任务行。
import { computed } from "vue";
import TaskProgressPopup from "./TaskProgressPopup.vue";
import { api } from "../lib/api";
import { t, tErr } from "../lib/i18n";
import { isRunning } from "../lib/modpackTasks";
import { progressPercent } from "../lib/packProgress";
import { showToast } from "../lib/toast";
import type { ModPackStatusDto, ModPackTaskDto } from "../lib/bindings";
import type { TaskProgressRow, TaskStatusKind } from "../lib/taskProgress";

const props = defineProps<{
  /** 整合包安装任务状态，来自 App.vue 那份全局订阅 */
  status: ModPackStatusDto;
}>();

const emit = defineEmits<{
  /** 关闭弹窗；安装任务不受影响 */
  (e: "close"): void;
}>();

/** 进行中的任务 */
const running = computed(() => props.status.tasks.filter((task) => isRunning(task)));

/** 已结束的任务 */
const finished = computed(() => props.status.tasks.filter((task) => !isRunning(task)));

/**
 * 任务状态
 *
 * - `task`: 整合包安装任务
 */
function statusKindOf(task: ModPackTaskDto): TaskStatusKind {
  if (task.done) return "done";
  if (task.failed) return "failed";
  if (task.cancelled) return "cancelled";
  return "running";
}

/**
 * 右侧状态文案
 *
 * - `task`: 整合包安装任务
 */
function statusTextOf(task: ModPackTaskDto): string {
  const kind = statusKindOf(task);
  if (kind === "done") return t("add.packState.done");
  if (kind === "failed") return t("modpack.bar.failed");
  if (kind === "cancelled") return t("modpack.bar.cancelled");
  const state = t(`add.packState.${task.state}`);
  return task.total ? `${state} ${task.now}/${task.total}` : state;
}

/** 归一化成共享弹窗的行 */
const rows = computed<TaskProgressRow[]>(() =>
  props.status.tasks.map((task) => {
    const active = isRunning(task);
    return {
      key: `${task.pid}:${task.fid}`,
      name: task.name,
      icon: task.icon,
      percent: task.done ? 100 : Math.round(progressPercent(task) ?? 0),
      unknown: active && !task.total && !task.subTotal,
      statusText: statusTextOf(task),
      statusKind: statusKindOf(task),
      error: task.error,
      subText: active ? task.subText : null,
      subPercent: active && task.subTotal > 0 ? (task.subNow / task.subTotal) * 100 : null,
      subUnknown: active && !!task.subText && !task.subTotal,
      cancelable: active,
    };
  }),
);

/** 标题下的任务数 */
const headText = computed(() =>
  running.value.length
    ? t("modpack.bar.running", { count: running.value.length })
    : t("modpack.bar.finished", { count: props.status.tasks.length }),
);

/** 总进度：多个任务同时装时取各任务进度的均值 */
const overallPercent = computed(() => {
  if (running.value.length <= 1) return null;
  const sum = running.value.reduce((acc, task) => acc + (progressPercent(task) ?? 0), 0);
  return sum / running.value.length;
});

/** 所有进行中的任务都还没有总数时，总进度走无限滚动 */
const overallUnknown = computed(
  () => running.value.length > 0 && running.value.every((task) => !task.total && !task.subTotal),
);

/** 有终态任务时才给「清除已完成」 */
const clearLabel = computed(() =>
  finished.value.length ? t("modpack.bar.clearDone") : null,
);

/**
 * 取消一条安装
 *
 * - `key`: `pid:fid` 形式的任务键
 */
async function cancel(key: string) {
  const task = props.status.tasks.find((item) => `${item.pid}:${item.fid}` === key);
  if (!task) return;
  try {
    await api.cancelModpackInstall(task.pid, task.fid);
  } catch (e) {
    showToast(t("modpack.bar.cancelFail", { msg: tErr(e) }));
  }
}

/** 清掉已结束的任务 */
async function clear() {
  try {
    await api.clearModpackDone();
  } catch (e) {
    showToast(tErr(e));
  }
}
</script>

<template>
  <TaskProgressPopup :title="t('modpack.bar.title')" :head-text="headText" :rows="rows"
    :overall-percent="overallPercent" :overall-unknown="overallUnknown" :clear-label="clearLabel"
    @close="emit('close')" @cancel="cancel" @clear="clear" />
</template>
