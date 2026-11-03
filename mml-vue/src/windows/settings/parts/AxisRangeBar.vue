<script setup lang="ts">
// 轴区间条：把 -1.0 到 1.0 画成一条，标出这条绑定覆盖的区间与摇杆当前值
import { computed } from "vue";

const props = defineProps<{
  /** 区间起点，-1.0 到 1.0 */
  from: number;
  /** 区间终点，-1.0 到 1.0 */
  to: number;
  /** 当前值，-1.0 到 1.0 */
  value: number;
}>();

/** 把 -1.0 到 1.0 映射成 0% 到 100% */
function percent(value: number): number {
  return Math.min(100, Math.max(0, ((value + 1) / 2) * 100));
}

/** 区间左端 */
const low = computed(() => percent(Math.min(props.from, props.to)));

/** 区间宽度 */
const width = computed(() => percent(Math.max(props.from, props.to)) - low.value);

/** 当前值的位置 */
const cursor = computed(() => percent(props.value));

/** 当前值是否落在区间里 */
const inside = computed(() =>
  props.value >= Math.min(props.from, props.to) && props.value <= Math.max(props.from, props.to),
);

/** 显示用的两位小数 */
const shown = computed(() => props.value.toFixed(2));
</script>

<template>
  <div class="axis-bar" :class="{ on: inside }">
    <div class="axis-track">
      <div class="axis-region" :style="{ left: low + '%', width: width + '%' }" />
      <div class="axis-tick" style="left: 50%" />
      <div class="axis-cursor" :style="{ left: cursor + '%' }" />
    </div>
    <span class="axis-value">{{ shown }}</span>
  </div>
</template>

<style scoped>
/* 区间条：左 -1、中 0、右 +1；命中时整条换成强调色 */
.axis-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 0 6px 8px;
}

.axis-track {
  position: relative;
  flex: 1;
  height: 6px;
  border-radius: 999px;
  background: var(--bg-hover);
}

/* 这条绑定覆盖的区间 */
.axis-region {
  position: absolute;
  top: 0;
  bottom: 0;
  border-radius: 999px;
  background: var(--accent-soft);
  transition: background 0.12s;
}

/* 中点刻度 */
.axis-tick {
  position: absolute;
  top: -2px;
  width: 1px;
  height: 10px;
  background: var(--border);
}

/* 当前值 */
.axis-cursor {
  position: absolute;
  top: -3px;
  width: 3px;
  height: 12px;
  margin-left: -1.5px;
  border-radius: 999px;
  background: var(--text-dim);
}

.axis-bar.on .axis-region {
  background: var(--accent);
}

.axis-bar.on .axis-cursor {
  background: var(--accent);
}

.axis-value {
  min-width: 34px;
  font-size: 11.5px;
  color: var(--text-dim);
  text-align: right;
}
</style>
