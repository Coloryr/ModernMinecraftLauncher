<script setup lang="ts">
// 实例日志面板：线程 / 级别 / 分类筛选 + 彩色控制台 + 自动滚动
import { computed, ref, watch, nextTick } from "vue";
import { t } from "../lib/i18n";
import type { LogLine } from "../lib/bindings";
import { uniq } from "../lib/util";

const props = defineProps<{
  /** 日志行，按时间顺序 */
  logs: LogLine[];
  /** 撑满父容器；独立日志窗口用，默认固定高度内嵌在主窗口里 */
  fill?: boolean;
}>();

const consoleEl = ref<HTMLElement | null>(null);

watch(
  () => props.logs.length,
  () => {
    nextTick(() => {
      if (consoleEl.value) {
        consoleEl.value.scrollTop = consoleEl.value.scrollHeight;
      }
    });
  },
);

// ================= 筛选器（线程 / 级别 / 分类） =================

const ALL = "";
const threadFilter = ref(ALL);
const levelFilter = ref(ALL);
const categoryFilter = ref(ALL);

const threadOptions = computed(() => uniq(props.logs.map((l) => l.thread)));
const categoryOptions = computed(() => uniq(props.logs.map((l) => l.category)));

/** 级别按严重度排序 */
const levelOptions = computed(() => {
  const set = new Set(props.logs.map((l) => l.level).filter(Boolean));
  return ["Error", "Warn", "Info", "Debug"].filter((l) => set.delete(l)).concat([...set]);
});

const filteredLogs = computed(() =>
  props.logs.filter(
    (l) =>
      (threadFilter.value === ALL || l.thread === threadFilter.value) &&
      (levelFilter.value === ALL || l.level === levelFilter.value) &&
      (categoryFilter.value === ALL || l.category === categoryFilter.value),
  ),
);

// 日志被清空时重置筛选，清空来自 clear 事件或切换实例
watch(
  () => props.logs.length === 0,
  (empty) => {
    if (empty) {
      threadFilter.value = ALL;
      levelFilter.value = ALL;
      categoryFilter.value = ALL;
    }
  },
);
</script>

<template>
  <div class="inst-log" :class="{ fill: props.fill }">
    <div class="filter-bar">
      <select v-model="threadFilter" class="field-select filter-select" v-tip="t('launch.filterThread')">
        <option value="">{{ t("launch.filterThread") }} · {{ t("launch.filterAll") }}</option>
        <option v-for="th in threadOptions" :key="th" :value="th">{{ th }}</option>
      </select>
      <select v-model="levelFilter" class="field-select filter-select" v-tip="t('launch.filterLevel')">
        <option value="">{{ t("launch.filterLevel") }} · {{ t("launch.filterAll") }}</option>
        <option v-for="lv in levelOptions" :key="lv" :value="lv">{{ lv }}</option>
      </select>
      <select v-model="categoryFilter" class="field-select filter-select" v-tip="t('launch.filterCategory')">
        <option value="">{{ t("launch.filterCategory") }} · {{ t("launch.filterAll") }}</option>
        <option v-for="c in categoryOptions" :key="c" :value="c">{{ c }}</option>
      </select>
    </div>

    <div ref="consoleEl" class="console">
      <div v-for="(line, i) in filteredLogs" :key="i" class="log-line"
        :class="{ 'log-error': line.level === 'Error', 'log-warn': line.level === 'Warn' }">
        {{ line.text }}
      </div>
      <div v-if="filteredLogs.length === 0" class="log-empty">{{ t("launch.waitLog") }}</div>
    </div>
  </div>
</template>

<style scoped>
.inst-log {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
}

/* 撑满父容器（独立日志窗口）：控制台随窗口伸缩，不固定高 */
.inst-log.fill {
  flex: 1;
}

.inst-log.fill .console {
  height: auto;
  flex: 1;
  max-height: none;
}

.filter-bar {
  display: flex;
  gap: 8px;
}

.console {
  height: 46vh;
  max-height: 480px;
  background: #0d0f12;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 14px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  font-family: "Cascadia Code", Consolas, "Courier New", monospace;
  font-size: 12.5px;
  line-height: 1.65;
  user-select: text;
  text-align: left;
}

.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  color: #c8d0da;
}

.log-line.log-error {
  color: #ff7b72;
}

.log-line.log-warn {
  color: #e3b341;
}

.log-empty {
  color: #4d5560;
  text-align: center;
  margin-top: 80px;
}
</style>
