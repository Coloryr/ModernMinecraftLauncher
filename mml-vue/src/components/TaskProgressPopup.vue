<script setup lang="ts">
// 安装 / 下载进度弹窗：Java 安装、资源下载、整合包安装共用一套壳与任务行
//
// 三处任务的 DTO 字段不同，调用方先归一化成 TaskProgressRow[]；这里只负责
// 标题、任务数、总进度、逐行进度条、取消与底部按钮，避免三个弹窗各写一份模板。
import { computed } from "vue";
import AsyncImage from "./ui/AsyncImage.vue";
import BaseModal from "./ui/BaseModal.vue";
import { t, tErr } from "../lib/i18n";
import { shortPath } from "../lib/format";
import type { TaskProgressRow } from "../lib/taskProgress";

const props = withDefaults(
  defineProps<{
    /** 弹窗标题 */
    title: string;
    /** 标题下的任务数文案，如「正在安装 2 个」 */
    headText: string;
    /** 逐行任务 */
    rows: TaskProgressRow[];
    /** 总进度，0–100；null 不画 */
    overallPercent?: number | null;
    /** 总进度总数未知时走无限滚动 */
    overallUnknown?: boolean;
    /** 「清除已完成」文案；null 不显示 */
    clearLabel?: string | null;
  }>(),
  { overallPercent: null, overallUnknown: false, clearLabel: null },
);

const emit = defineEmits<{
  /** 关闭弹窗；任务不受影响 */
  (e: "close"): void;
  /** 清掉已结束的任务 */
  (e: "clear"): void;
  /** 取消某条进行中的任务 */
  (e: "cancel", key: string): void;
}>();

/** 进行中的行数：只有多个任务时总进度条才有意义 */
const runningCount = computed(
  () => props.rows.filter((row) => row.statusKind === "running").length,
);

/** 是否画总进度条 */
const showOverall = computed(
  () => props.overallPercent !== null && runningCount.value > 1,
);

/**
 * 把进度夹到 0–100
 *
 * - `value`: 原始百分比
 */
function clampPercent(value: number): number {
  return Math.min(100, Math.max(0, value));
}

/**
 * 行内进度条的宽度样式；总数未知时返回 undefined，交给 .is-roll
 *
 * - `value`: 百分比
 * - `unknown`: 总数是否未知
 */
function barStyle(value: number, unknown: boolean | undefined): { width: string } | undefined {
  return unknown ? undefined : { width: `${clampPercent(value)}%` };
}
</script>

<template>
  <BaseModal :title="title" :width="560" fixed-height="420px" below-titlebar :closable="false"
    @close="emit('close')">
    <div class="tp">
      <div class="tp-head">
        <span class="tp-head-text">{{ headText }}</span>
      </div>
      <div v-if="showOverall" class="progress-track">
        <div class="progress-fill" :class="{ 'is-roll': overallUnknown }"
          :style="barStyle(overallPercent ?? 0, overallUnknown)" />
      </div>

      <div class="tp-list">
        <div v-for="row in rows" :key="row.key" class="tp-row">
          <div class="tp-row-head">
            <AsyncImage v-if="row.icon" class="tp-icon" :src="row.icon" alt="" />
            <span v-else class="tp-icon tp-icon-fallback">{{ row.name.slice(0, 1).toUpperCase() }}</span>
            <span class="tp-name" :title="row.name">{{ row.name }}</span>
            <button v-if="row.cancelable" class="tp-cancel" @click="emit('cancel', row.key)">
              {{ t("actions.cancel") }}
            </button>
          </div>

          <!-- 状态文案贴在进度条右侧，固定一列宽度：多条任务的进度条与数字才能对齐 -->
          <div class="tp-progress">
            <div class="progress-track">
              <div class="progress-fill" :class="{
                'is-roll': row.unknown,
                'is-failed': row.statusKind === 'failed',
                'is-cancelled': row.statusKind === 'cancelled',
              }" :style="barStyle(row.percent, row.unknown)" />
            </div>
            <span class="tp-status-cell">
              <span v-if="row.statusKind === 'running'" class="tp-status" :title="row.statusText">{{ row.statusText }}</span>
              <span v-else class="tp-tag" :class="row.statusKind">{{ row.statusText }}</span>
            </span>
          </div>

          <template v-if="row.statusKind === 'running' && (row.subText || row.subPercent !== null && row.subPercent !== undefined)">
            <div v-if="row.subText" class="tp-sub" :title="row.subText">{{ shortPath(row.subText) }}</div>
            <div v-if="row.subPercent !== null && row.subPercent !== undefined" class="tp-progress sub">
              <div class="progress-track sub">
                <div class="progress-fill" :class="{ 'is-roll': row.subUnknown }"
                  :style="barStyle(row.subPercent ?? 0, row.subUnknown)" />
              </div>
              <span class="tp-status-cell" aria-hidden="true"></span>
            </div>
          </template>

          <p v-if="row.error" class="tp-error">{{ tErr(row.error) }}</p>
        </div>
      </div>

      <div class="tp-actions">
        <button v-if="clearLabel" class="tp-btn" @click="emit('clear')">{{ clearLabel }}</button>
        <button class="tp-btn" @click="emit('close')">{{ t("actions.close") }}</button>
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 顶到底：列表占满剩余高度，任务多了内部滚，弹窗高度不跟着任务数跳 */
.tp {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.tp-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tp-head-text {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

/* 总进度条只在有多个进行中任务时画 */
.tp > .progress-track {
  margin-top: 6px;
}

.tp-list {
  flex: 1;
  min-height: 0;
  margin-top: 6px;
  overflow-y: auto;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回，与项目列表同一做法 */
  margin-right: -8px;
  padding-right: 8px;
  scrollbar-gutter: stable;
}

.tp-row {
  padding: 10px 2px;
  border-bottom: 1px solid var(--border);
}

.tp-row:last-child {
  border-bottom: none;
}

.tp-row-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tp-icon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  object-fit: cover;
  flex-shrink: 0;
}

/* 没有图标时用首字母占位 */
.tp-icon-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 12.5px;
  font-weight: 600;
}

.tp-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.tp-status {
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11.5px;
  color: var(--text-dim);
}

.tp-tag {
  flex-shrink: 0;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 11.5px;
}

.tp-tag.done {
  color: var(--green);
  background: color-mix(in srgb, var(--green) 14%, transparent);
}

.tp-tag.failed {
  color: var(--red);
  background: color-mix(in srgb, var(--red) 14%, transparent);
}

.tp-tag.cancelled {
  color: var(--text-dim);
  background: var(--bg-hover);
}

.tp-cancel {
  flex-shrink: 0;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--text-dim);
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
}

.tp-cancel:hover {
  color: var(--red);
}

/* 进度条 + 右侧固定宽度的状态列：状态统一贴条右边，多条任务的进度条才能对齐 */
.tp-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
}

.tp-progress.sub {
  margin-top: 4px;
}

.tp-progress .progress-track {
  flex: 1;
  min-width: 0;
  margin-top: 0;
}

/* 状态列宽度固定：下载中 / 项目数 / 终态标签都从这里对齐 */
.tp-status-cell {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  width: 104px;
  flex-shrink: 0;
  min-width: 0;
}

/* 8px 主进度、6px 子进度：与其它进度条同档，见 AGENTS.md §16.4 */
.progress-track {
  height: 8px;
  margin-top: 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.progress-track.sub {
  height: 6px;
  margin-top: 4px;
}

/* 子进度条也放进 .tp-progress 时，间距由外层统一给 */
.tp-progress .progress-track.sub {
  margin-top: 0;
}

.progress-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.3s;
}

/* 失败 / 已取消也留出进度条，状态列才能和上面几行对齐；条本身按实际情况着色 */
.progress-fill.is-failed {
  background: var(--red);
}

.progress-fill.is-cancelled {
  background: var(--text-dim);
}

/* 总数未知：无限滚动，见 styles/controls.css */
.progress-fill.is-roll {
  width: 40%;
  animation: progress-roll 1.2s linear infinite;
}

.tp-sub {
  margin-top: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11.5px;
  color: var(--text-dim);
}

.tp-error {
  margin: 6px 0 0;
  font-size: 11.5px;
  color: var(--red);
  word-break: break-all;
}

.tp-actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
  margin-top: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}

.tp-btn {
  height: var(--ctl-h);
  padding: 0 16px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text);
  font-size: var(--ctl-font);
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}

.tp-btn:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}
</style>
