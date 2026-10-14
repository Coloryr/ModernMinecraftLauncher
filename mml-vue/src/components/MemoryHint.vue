<script setup lang="ts">
// 本机内存参考值：一行文字 + 一条"可用占比"的细条。
//
// 两处共用（实例设置的启动参数、设置窗口的「游戏启动」），所以做成组件：
// 读数是整机一份、5s 刷新，见 lib/systemMemory。
import { useSystemMemory } from "../lib/systemMemory";

const { text, freePercent } = useSystemMemory();
</script>

<template>
  <div class="mem-hint">
    <span class="mem-text">{{ text }}</span>
    <!-- 条上画的是"可用"（满条 = 空闲多）；读不到物理内存时整条不画，只留上面的文字。
         数值已经写在上面那行里，这条对读屏是重复信息 -->
    <div v-if="freePercent > 0" class="mem-bar" aria-hidden="true">
      <div class="mem-bar-fill" :style="{ width: freePercent + '%' }" />
    </div>
  </div>
</template>

<style scoped>
.mem-hint {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.mem-text {
  font-size: 12px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.mem-bar {
  height: 6px;
  border-radius: 999px;
  /* 轨道不能用 --bg-raised：亮色主题里它和 --bg-card 都是纯白，条到哪结束看不出来。
     用文字色的半透明，深色 / 亮色 / 各种强调色主题都保证和卡片底色分得开 */
  background: color-mix(in srgb, var(--text-dim) 30%, transparent);
  overflow: hidden;
}

.mem-bar-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  /* 5s 一刷：走个短过渡，数值跳动不至于像闪一下 */
  transition: width 0.3s ease;
}
</style>
