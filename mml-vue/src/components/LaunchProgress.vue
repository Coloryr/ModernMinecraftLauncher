<script setup lang="ts">
// 启动流程：整个放在启动按钮**里面** —— 上一行是转圈 + 阶段文本 + 百分比，
// 进度条紧跟在这行文字下面（两行都在文档流里，不贴按钮底边）
//
// 只管画：阶段文案、百分比都由调用方给。颜色一律走 currentColor，任何按钮变体上都成立。
defineProps<{
  /** 阶段文案，调用方已 t() 好的成品 */
  text: string;
  /** 进度百分比；null 表示这一阶段没有确定进度，走滚动条 */
  percent: number | null;
}>();
</script>

<template>
  <span class="launch-in">
    <span class="launch-in-head">
      <span class="launch-spinner"></span>
      <span class="launch-stage">{{ text }}</span>
      <span v-if="percent !== null" class="launch-pct">{{ percent }}%</span>
    </span>
    <span class="launch-in-track">
      <span v-if="percent !== null" class="launch-in-bar" :style="{ width: percent + '%' }"></span>
      <span v-else class="launch-in-bar rolling"></span>
    </span>
  </span>
</template>

<style scoped>
/* 竖排两行：一行文字，进度条紧跟在文字下面。
   不贴按钮底边：那里落在圆角上会被裁掉两端，观感也像贴在框上。
   也不撑满按钮（不写 width: 100%）：整组按内容居中，进度条是**定宽**的（见 .launch-in-track）；
   文字太长时受按钮宽度约束、由 .launch-stage 省略号收尾 */
.launch-in {
  display: flex;
  flex-direction: column;
  /* 进度条定宽后不再是 stretch，整组要在按钮里居中 */
  align-items: center;
  justify-content: center;
  /* 2 + 3 正好压在按钮现有内容高度里，启动态不把按钮顶高 */
  gap: 2px;
  min-width: 0;
  /* 按钮本体可能大几号（主界面那颗是 15/16px），流程按普通控件那一档走 */
  font-size: var(--ctl-font);
}

.launch-in-head {
  display: flex;
  align-items: center;
  /* 转圈 + 文本 + 百分比作为一行居中 */
  justify-content: center;
  gap: 8px;
  min-width: 0;
}

.launch-in-track {
  /* 定宽：跟着文字宽度走的话，阶段名一换长度就跳（"检查游戏文件"与"下载缺失文件"不一样长）。
     要按使用处调整就覆盖 --launch-bar-w；定宽后容器宽度取两者的较大值，所以是居中不是拉伸 */
  width: var(--launch-bar-w, 110px);
  height: 3px;
  border-radius: 2px;
  background: color-mix(in srgb, currentColor 25%, transparent);
  overflow: hidden;
}

.launch-in-bar {
  display: block;
  height: 100%;
  background: currentColor;
  transition: width 0.3s ease;
}

.launch-in-bar.rolling {
  width: 40%;
  animation: launch-slide 1.1s ease-in-out infinite;
}

.launch-spinner {
  flex-shrink: 0;
  width: 12px;
  height: 12px;
  border: 2px solid color-mix(in srgb, currentColor 35%, transparent);
  border-top-color: currentColor;
  border-radius: 50%;
  animation: launch-spin 0.8s linear infinite;
}

@keyframes launch-spin {
  to {
    transform: rotate(360deg);
  }
}

.launch-stage {
  /* 不写 flex: 1：撑满剩余宽度会把文本顶到左边，整组就居中不了；太长靠下面三条裁掉 */
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.launch-pct {
  flex-shrink: 0;
  font-size: 11.5px;
  opacity: 0.85;
  font-variant-numeric: tabular-nums;
}

@keyframes launch-slide {
  0% {
    transform: translateX(-100%);
  }

  100% {
    transform: translateX(350%);
  }
}
</style>
