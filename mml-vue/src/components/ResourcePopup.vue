<script setup lang="ts">
// 资源下载进度弹窗：标题栏指示器点开后浮在当前页面之上
//
// 与 Java 安装、整合包安装共用一个 TaskProgressPopup；任务按项目（pid）分组，
// 多文件项目按项目数推进，单文件按字节，口径见 lib/progress.ts。
import { computed } from "vue";
import TaskProgressPopup from "./TaskProgressPopup.vue";
import { api } from "../lib/api";
import { t, tErr } from "../lib/i18n";
import { resourceGroups, resourceOverallPercent, type ResourceGroupProgress } from "../lib/progress";
import { showToast } from "../lib/toast";
import type { ResourceStatusDto } from "../lib/bindings";
import type { TaskProgressRow, TaskStatusKind } from "../lib/taskProgress";

const props = defineProps<{
  /** 资源下载任务状态，来自 App.vue 那份全局订阅 */
  status: ResourceStatusDto;
}>();

const emit = defineEmits<{
  /** 关闭弹窗；下载任务不受影响 */
  (e: "close"): void;
}>();

/** 按下载项目分组 */
const groups = computed(() => resourceGroups(props.status.tasks));

/** 还在下载的项目 */
const running = computed(() =>
  groups.value.filter((group) => !group.done && !group.failed && !group.cancelled),
);

/** 已结束的项目 */
const finished = computed(() =>
  groups.value.filter((group) => group.done || group.failed || group.cancelled),
);

/**
 * 项目状态
 *
 * - `group`: 下载项目
 */
function statusKindOf(group: ResourceGroupProgress): TaskStatusKind {
  if (group.done) return "done";
  if (group.failed) return "failed";
  if (group.cancelled) return "cancelled";
  return "running";
}

/**
 * 终态文案
 *
 * - `group`: 下载项目
 */
function stateLabel(group: ResourceGroupProgress): string {
  if (group.done) return t("addResource.bar.done");
  if (group.failed) return t("addResource.bar.failed");
  return t("addResource.bar.cancelled");
}

/**
 * 进行中的右侧文案：多文件说项目数，单文件说百分比
 *
 * - `group`: 下载项目
 */
function progressText(group: ResourceGroupProgress): string {
  if (group.byCount) {
    return t("addResource.bar.projects", { done: group.finished, total: group.total });
  }
  return `${Math.round(group.percent)}%`;
}

/** 归一化成共享弹窗的行 */
const rows = computed<TaskProgressRow[]>(() =>
  groups.value.map((group) => {
    const kind = statusKindOf(group);
    return {
      key: group.pid,
      name: group.name,
      icon: group.icon || null,
      percent: Math.round(group.percent),
      statusText: kind === "running" ? progressText(group) : stateLabel(group),
      statusKind: kind,
      subPercent: null,
    };
  }),
);

/** 标题下的任务数 */
const headText = computed(() =>
  running.value.length
    ? t("addResource.bar.running", { count: running.value.length })
    : t("addResource.bar.finished", { count: groups.value.length }),
);

/** 总进度：与标题栏指示器同一口径 */
const overallPercent = computed(() =>
  running.value.length ? resourceOverallPercent(props.status.tasks) : null,
);

/** 有终态项目时才给「清除已完成」 */
const clearLabel = computed(() =>
  finished.value.length ? t("addResource.bar.clearDone") : null,
);

/** 清掉已结束的项目 */
async function clear() {
  try {
    await api.clearResourceDone();
  } catch (e) {
    showToast(tErr(e));
  }
}
</script>

<template>
  <TaskProgressPopup :title="t('addResource.bar.title')" :head-text="headText" :rows="rows"
    :overall-percent="overallPercent" :clear-label="clearLabel" @close="emit('close')" @clear="clear" />
</template>
