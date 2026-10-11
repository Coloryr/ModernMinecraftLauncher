<script setup lang="ts">
// 资源下载进度条：每个下载项目（pid）一条进度，终态带标签。
// 完成后后端保留一段时间再自动移除，这里只做展示。
// 添加资源窗口常显；主窗口仅在添加资源窗口关闭时显示（由调用方 v-if 控制）。
//
// 口径见 lib/progress.ts：同一个项目下选了多个文件时按「项目数」推进，
// 只有一个文件时用该文件自己的字节进度（单独下一个模组就是这一种）。
import { computed } from "vue";
import { resourceGroups, type ResourceGroupProgress } from "../lib/progress";
import type { ResourceStatusDto } from "../lib/bindings";
import { t } from "../lib/i18n";

const props = defineProps<{
  /** 当前资源下载状态，含各项目的进度 */
  status: ResourceStatusDto;
}>();

/** 按下载项目分组的进度 */
const groups = computed(() => resourceGroups(props.status.tasks));

/** 总进度 = 还没下完的各项目进度的均值；已结束的不参与平均，与原口径一致 */
const percent = computed(() => {
  const active = groups.value.filter((group) => group.finished < group.total);
  if (!active.length) return 100;
  return active.reduce((acc, group) => acc + group.percent, 0) / active.length;
});

/** 终态标签；未结束时返回 `null` */
function stateTag(group: ResourceGroupProgress): string | null {
  if (group.done) return t("addResource.bar.done");
  if (group.failed) return t("addResource.bar.failed");
  return null;
}

/** 进度文案：多个文件的项目说"项目 x/y"，单文件用字节百分比 */
function progressText(group: ResourceGroupProgress): string {
  if (group.byCount) {
    return t("addResource.bar.projects", { done: group.finished, total: group.total });
  }
  return `${group.percent.toFixed(0)}%`;
}
</script>

<template>
  <div class="res-bar">
    <div class="res-bar-head">
      <span class="res-bar-title">{{ t("addResource.bar.running", { count: groups.length }) }}</span>
    </div>
    <div class="progress-track">
      <div class="progress-fill" :style="{ width: percent + '%' }" />
    </div>

    <!-- 每个下载项目的进度 -->
    <div v-if="groups.length > 1" class="res-task-list">
      <div v-for="group in groups" :key="group.pid" class="res-task">
        <div class="res-task-head">
          <span class="res-task-name">{{ group.name }}</span>
          <span v-if="stateTag(group)" class="res-tag" :class="{ done: group.done, failed: group.failed }">
            {{ stateTag(group) }}
          </span>
          <span v-else class="res-task-percent">{{ progressText(group) }}</span>
        </div>
        <div class="progress-track sub">
          <div class="progress-fill" :style="{ width: group.percent + '%' }" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.res-bar {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
}

.res-bar-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.res-bar-title {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.res-task-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 2px;
}

.res-task {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.res-task-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.res-task-name {
  flex: 1;
  font-size: 12px;
  color: var(--text);
  word-break: break-all;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.res-task-percent {
  font-size: 11px;
  color: var(--text-dim);
  flex-shrink: 0;
}

.res-tag {
  font-size: 11px;
  padding: 1px 8px;
  border-radius: 999px;
  flex-shrink: 0;
}

.res-tag.done {
  color: var(--green);
  background: color-mix(in srgb, var(--green) 14%, transparent);
}

.res-tag.failed {
  color: var(--red);
  background: color-mix(in srgb, var(--red) 14%, transparent);
}

.progress-track {
  height: 8px;
  border-radius: 4px;
  background: var(--bg-hover);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 4px;
  background: var(--accent-grad);
  transition: width 0.3s;
}

.progress-track.sub {
  height: 6px;
}
</style>
