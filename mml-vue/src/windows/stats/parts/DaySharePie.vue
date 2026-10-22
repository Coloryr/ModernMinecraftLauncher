<script lang="ts">
/** 一个实例在这一项指标上的值 */
export interface RingItem {
  /** 实例标识，用来取色 */
  uuid: string;
  /** 实例名，悬停提示里用 */
  name: string;
  /** 这一项指标的数值 */
  value: number;
}
</script>

<script setup lang="ts">
// 单日环形图：把某一项指标按实例拆分后画成占比环。
// 不引图表库，用 SVG 圆环 + stroke-dasharray 分段；颜色取 lib/instanceColor，
// 与实例图标的兜底渐变同源，几个环摆在一起时同一实例颜色一致。
//
// 只画环与中心数值，图例由调用方统一出一份 —— 一排几个环各带一份图例会重复同一批实例名。
import { computed } from "vue";
import { instanceColor } from "../../../lib/instanceColor";
import { t } from "../../../lib/i18n";

const props = defineProps<{
  /** 要画的各个实例，顺序即分段顺序 */
  items: RingItem[];
  /** 卡片标题 */
  title: string;
  /**
   * 数值口径，决定中心与提示里的文本
   * - `hours`: 时长，按小时
   * - `count`: 次数
   */
  kind: "hours" | "count";
}>();

/** 圆环半径与线宽，单位是 SVG 用户单位（viewBox 100×100） */
const R = 38;
const W = 16;
/** 周长：dasharray 按它分段 */
const CIRC = 2 * Math.PI * R;
/** 段与段之间留的空隙，单位同周长 */
const GAP = 3;

/** 这项指标的总和 */
const total = computed(() => props.items.reduce((acc, it) => acc + it.value, 0));

/**
 * 各段的 dasharray 与偏移
 *
 * 从 12 点开始顺时针排：第 i 段绕过的长度是前面所有段之和。
 * 段长扣掉 GAP 才有空隙；占比太小的段扣完会变负，按 0 处理。
 */
const segments = computed(() => {
  const sum = total.value;
  if (sum <= 0) return [];
  let offset = 0;
  return props.items.map((item) => {
    const len = Math.max(0, (item.value / sum) * CIRC - GAP);
    const seg = {
      uuid: item.uuid,
      dash: `${len} ${CIRC - len}`,
      offset: -offset,
      tip: t("winStats.pieTip", {
        name: item.name,
        value: textOf(item.value),
        pct: Math.round((item.value / sum) * 100),
      }),
    };
    offset += (item.value / sum) * CIRC;
    return seg;
  });
});

/** 秒数折成小时数文本，保留一位小数；超过 100 小时不留小数 */
function hoursNum(seconds: number): string {
  const hours = seconds / 3600;
  return hours >= 100 ? hours.toFixed(0) : hours.toFixed(1);
}

/**
 * 一项指标的文本
 *
 * - `value`: 数值；`kind` 为 hours 时是秒，为 count 时是次数
 *
 * # 返回值
 *
 * 已本地化的文本
 */
function textOf(value: number): string {
  return props.kind === "hours"
    ? t("winStats.hours", { h: hoursNum(value) })
    : t("winStats.times", { n: value });
}

/** 中心显示的总数 */
const totalText = computed(() => textOf(total.value));

</script>

<template>
  <div class="ring-card">
    <svg class="ring" viewBox="0 0 100 100" role="img" :aria-label="title">
      <circle :cx="50" :cy="50" :r="R" fill="none" stroke="var(--border)" :stroke-width="W" />
      <circle v-for="seg in segments" :key="seg.uuid" :cx="50" :cy="50" :r="R" fill="none"
        :stroke="instanceColor(seg.uuid)" :stroke-width="W" :stroke-dasharray="seg.dash"
        :stroke-dashoffset="seg.offset" transform="rotate(-90 50 50)" v-tip="seg.tip" />
      <!-- 中心数值：环中间空着，正好放这一项的总数 -->
      <text x="50" y="50" class="ring-total" text-anchor="middle" dominant-baseline="central">{{ totalText }}</text>
    </svg>
    <p class="ring-title">{{ title }}</p>
  </div>
</template>

<style scoped>
.ring-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.ring {
  width: 96px;
  height: 96px;
  flex-shrink: 0;
}

/* 悬停提亮这一段；不改 stroke-width —— 环宽是相邻段共用的，加粗会互相挤 */
.ring circle {
  transition: filter 0.12s;
}

.ring circle:hover {
  filter: brightness(1.18);
}

/* 中心数值：10.5px 在 96px 的环里略显糊，这里按 SVG 用户单位给 13 */
.ring-total {
  font-size: 13px;
  font-weight: 700;
  fill: var(--text);
  font-variant-numeric: tabular-nums;
}

.ring-title {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  white-space: nowrap;
}
</style>
