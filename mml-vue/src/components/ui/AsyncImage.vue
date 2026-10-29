<script setup lang="ts">
// 带加载占位的图片：加载中显示流光动画，失败显示灰底图标占位。
// 父级把尺寸类挂在组件上即可。
//
// 失败兜底与全局那套 lib/imageFallback.ts 是一回事，区别在这里能画得更讲究：
// 全局兜底只是把 <img> 换成一张灰底占位图，够用且覆盖所有直接写 <img> 的地方；
// 走本组件的地方加载中有流光、失败还能抛出 error 让调用方重试，也不留破图痕迹。
import { ref, watch } from "vue";

const props = defineProps<{
  /** 图片地址 */
  src: string;
  /** 替代文本；缺省空串 */
  alt?: string;
  /** 悬停提示；不传则不提示 */
  title?: string;
}>();

const emit = defineEmits<{
  /** 加载失败时抛出去；调用方可据此重试 / 换地址，不接就是纯展示 */
  (e: "error"): void;
}>();

/** 图片是否已经加载出来，用来收掉流光占位 */
const loaded = ref(false);
/** 是否加载失败，失败后换成灰底图标占位 */
const failed = ref(false);

/** 图片加载失败：切到占位并通知调用方 */
function onError() {
  failed.value = true;
  emit("error");
}

// 换图（如版本筛选刷新截图）时回到占位状态
watch(
  () => props.src,
  () => {
    loaded.value = false;
    failed.value = false;
  },
);
</script>

<template>
  <span class="async-img">
    <img v-if="!failed" :src="src" :alt="alt ?? ''" v-tip="title" loading="lazy" :class="{ show: loaded }"
      @load="loaded = true" @error="onError" />
    <span v-if="!loaded && !failed" class="async-shimmer"></span>
    <!-- 加载失败：换成"灰底 + 图片图标"的占位，别留破图图标 -->
    <span v-else-if="failed" class="async-fail" aria-hidden="true">
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.6"
        stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="3" />
        <circle cx="8.5" cy="9" r="1.5" />
        <path d="m21 15-5-5L5 21" />
      </svg>
    </span>
  </span>
</template>

<style scoped>
.async-img {
  position: relative;
  display: block;
  overflow: hidden;
}

.async-img img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.async-img img.show {
  opacity: 1;
}

/* 流光占位：图盖上来之前一直在动 */
.async-shimmer {
  position: absolute;
  inset: 0;
  background: linear-gradient(100deg, var(--bg-hover) 40%, var(--bg-card) 50%, var(--bg-hover) 60%);
  background-size: 200% 100%;
  animation: async-shine 1.2s linear infinite;
}

@keyframes async-shine {
  to {
    background-position: -200% 0;
  }
}

/* 失败占位：灰底 + 居中图标；不放文字，各种尺寸下图标都稳，文字在小图标里会挤 */
.async-fail {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8%;
  box-sizing: border-box;
  background: var(--bg-hover);
  color: var(--text-dim);
}
</style>
