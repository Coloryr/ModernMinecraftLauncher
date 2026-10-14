<script setup lang="ts">
// 资源下载进度条：每个下载项目（pid）一条进度，终态带标签。
// 完成后后端保留一段时间再自动移除，这里只做展示。
// 添加资源窗口常显；主窗口仅在添加资源窗口关闭时显示（由调用方 v-if 控制）。
//
// 口径见 lib/progress.ts：同一个项目下选了多个文件时按「项目数」推进，
// 只有一个文件时用该文件自己的字节进度（单独下一个模组就是这一种）。
import { computed } from "vue";
import { api } from "../lib/api";
import { resourceGroups, resourceOverallPercent, type ResourceGroupProgress } from "../lib/progress";
import type { ResourceStatusDto } from "../lib/bindings";
import { t, tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";

const props = withDefaults(
  defineProps<{
    /** 当前资源下载状态，含各项目的进度 */
    status: ResourceStatusDto;
    /** 弹窗形态：撑满弹窗剩余高度，任务多了由任务区自己滚（弹窗那侧锁死高度时用） */
    fillHeight?: boolean;
  }>(),
  { fillHeight: false },
);

/** 按下载项目分组的进度 */
const groups = computed(() => resourceGroups(props.status.tasks));

/** 还在下的项目；标题与总进度条都按它算，全结束了就别再报"正在下载" */
const running = computed(() =>
  groups.value.filter((group) => !group.done && !group.failed && !group.cancelled),
);

/** 标题：还有在下的说数量，全结束了改说"已结束"，否则取消完还写着"正在下载" */
const headText = computed(() =>
  running.value.length
    ? t("addResource.bar.running", { count: running.value.length })
    : t("addResource.bar.finished", { count: groups.value.length }),
);

/** 总进度，与标题栏指示器同一口径，见 lib/progress.ts */
const percent = computed(() => resourceOverallPercent(props.status.tasks));

/** 终态标签；未结束时返回 `null` */
function stateTag(group: ResourceGroupProgress): string | null {
  if (group.done) return t("addResource.bar.done");
  if (group.failed) return t("addResource.bar.failed");
  if (group.cancelled) return t("addResource.bar.cancelled");
  return null;
}

/** 已结束的项目（完成 / 失败 / 取消），决定"清除已完成"显不显示 */
const finished = computed(() =>
  groups.value.filter((group) => group.done || group.failed || group.cancelled),
);

/** 清掉已结束的条目；终态条目不再自动移除，只能手动清 */
async function clearDone() {
  try {
    await api.clearResourceDone();
  } catch (e) {
    showToast(tErr(e));
  }
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
  <div class="res-bar" :class="{ 'fill-h': props.fillHeight }">
    <div class="res-bar-head">
      <span class="res-bar-title">{{ headText }}</span>
    </div>
    <!-- 总进度条只在还有在下的时候画：全结束时它是条满格蓝杠，什么信息都没表达 -->
    <div v-if="running.length" class="progress-track">
      <div class="progress-fill" :style="{ width: percent + '%' }" />
    </div>

    <!-- 每个下载项目的进度。**只有一个项目时也列**：名字是用户唯一能确认"下的是哪个"的东西；
         那种情况下总进度条就是它的进度，不再画一条重复的子条 -->
    <div v-if="groups.length" class="res-task-list">
      <div v-for="group in groups" :key="group.pid" class="res-task">
        <div class="res-task-head">
          <span class="res-task-name">{{ group.name }}</span>
          <span v-if="stateTag(group)" class="res-tag"
            :class="{ done: group.done, failed: group.failed, cancelled: group.cancelled }">
            {{ stateTag(group) }}
          </span>
          <span v-else class="res-task-percent">{{ progressText(group) }}</span>
        </div>
        <div v-if="groups.length > 1" class="progress-track sub">
          <div class="progress-fill" :style="{ width: group.percent + '%' }" />
        </div>
      </div>
    </div>

    <!-- 清掉已结束的条目：放列表底部，与整合包那条进度卡同一位置口径。
         终态条目不再自动移除，不点它就一直在，方便回头看下过什么 -->
    <div v-if="finished.length" class="res-footer">
      <button class="res-clear-btn" @click="clearDone">
        {{ t("addResource.bar.clearDone") }}
      </button>
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

/* 弹窗形态：卡片撑满弹窗剩余高度，任务多了由任务列表自己滚，不再把弹窗顶高。
   高度由弹窗那侧传 fillHeight 打开，窗口里那处保持自适应 */
.res-bar.fill-h {
  flex: 1;
  min-height: 0;
}

.res-bar.fill-h .res-task-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
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

/* 取消不是失败：用弱化色，与整合包那条进度卡的 .mp-tag.cancelled 同一套 */
.res-tag.cancelled {
  color: var(--text-dim);
  background: var(--bg-hover);
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

/* 清除已完成：列表底部一个次级按钮，靠右放（与弹窗、表单底部的按钮同一位置口径） */
.res-footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}

/* 尺寸与外观照整合包那条进度卡的 .mp-clear-btn：32px 高、左右 14px、细描边次级按钮 */
.res-clear-btn {
  height: var(--ctl-h);
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12.5px;
  font-family: inherit;
  cursor: pointer;
  transition: color 0.12s, border-color 0.12s;
}

.res-clear-btn:hover {
  color: var(--text);
  border-color: var(--accent);
}
</style>
