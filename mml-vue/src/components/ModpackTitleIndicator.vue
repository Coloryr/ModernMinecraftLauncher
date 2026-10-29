<script setup lang="ts">
// 整合包安装进度：标题栏上的动态图标 + 细进度条（点击展开进度弹窗）
//
// 为什么放标题栏而不是右下角浮层：它是**跟着当前窗口走的常驻指示**，
// 放标题栏既不占内容区，又不会像角落浮层那样在窄窗口里压住列表。
//
// 挂载位置**两种窗口模式不一样**（由调用方决定）：
// - 单窗口模式：整个应用只有一个真实窗口，任何页面共用主窗口的顶栏 → 挂在 MainTopbar 上，
//   这样切到哪一页都在；右下角那种全局浮层也能做到，但占地方；
// - 多窗口模式：整合包安装可能发生在"下载整合包"那个独立窗口里，
//   → 挂在 WindowFrame 的 head-right 插槽（各窗口自己那条标题栏）。
//
// 自包含状态：自己订阅安装任务，不依赖父组件传参（两处挂载点都能直接放）。
import { computed, onMounted, onUnmounted } from "vue";
import { t } from "../lib/i18n";
import { isRunning, useModpackStatus } from "../lib/modpackTasks";
import { openModpackPopup } from "../windows/windowManager";

const props = withDefaults(
  defineProps<{
    /** 点击图标时是否展开进度弹窗。多窗口模式下整合包窗口自己就是进度页，不需要弹窗 */
    popup?: boolean;
  }>(),
  { popup: true },
);

const { status, init } = useModpackStatus(false);

/**
 * 状态订阅的退订函数
 *
 * 赋值与注册分开：`onUnmounted` 只能在 setup 的**同步阶段**注册 —— 写在 `init()`
 * 的 `await` 之后就注册不上了，订阅会一直挂着 —— 所以退订函数存这里、由同步钩子调用。
 */
let unlistenStatus: (() => void) | null = null;
let disposed = false;

onMounted(async () => {
  const off = await init();
  // 订阅期间组件已卸载：立刻退订，别把监听留到窗口关闭之后
  if (disposed) {
    off();
    return;
  }
  unlistenStatus = off;
});

onUnmounted(() => {
  disposed = true;
  unlistenStatus?.();
  unlistenStatus = null;
});

const tasks = computed(() => status.value?.tasks ?? []);

const running = computed(() => tasks.value.filter(isRunning));

/** 有没有失败的任务（图标据此变成警示色） */
const hasFailed = computed(() => tasks.value.some((task) => task.failed));

/** 总进度 = 各进行中任务的主进度均值；全结束时 100 */
const percent = computed(() => {
  if (!running.value.length) return 100;
  const sum = running.value.reduce(
    (acc, task) => acc + (task.total ? (task.now / task.total) * 100 : 0),
    0,
  );
  return sum / running.value.length;
});

/** 悬停提示：进行中报进度，结束后报结果 */
const tipText = computed(() => {
  if (running.value.length) {
    return `${t("modpack.bar.running", { count: running.value.length })} · ${Math.round(percent.value)}%`;
  }
  return t("modpack.bar.finished", { count: tasks.value.length });
});

/** 图标状态：安装中、失败警示、全部完成常态 */
const iconState = computed(() => {
  if (running.value.length) return "busy";
  return hasFailed.value ? "failed" : "done";
});

/** 点一下：展开进度弹窗（关掉弹窗只收起，安装照跑） */
function onClick() {
  if (props.popup) openModpackPopup();
}
</script>

<template>
  <!-- 没有任务时整块不存在 -->
  <button v-if="tasks.length" class="mp-indicator" :class="iconState" :disabled="!popup" v-tip="tipText"
    :aria-label="tipText" @click="onClick">
    <span class="mp-ind-icon">
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.9"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 8v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8" />
        <path d="m3 8 2.5-4h13L21 8" />
        <path d="M12 3v5" />
        <path d="M9.5 11.5h5" />
      </svg>
    </span>
    <!-- 进度：细条贴在图标下沿，进行中才显示（结束后它是满格，没信息量） -->
    <span v-if="running.length" class="mp-ind-track">
      <span class="mp-ind-fill" :style="{ width: percent + '%' }" />
    </span>
  </button>
</template>

<style scoped>
.mp-indicator {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  /* 尺寸与圆角见 styles/titlebar.css 的方形图标键，这里只留外观 */
  padding: 0;
  border: 1px solid transparent;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

.mp-indicator:disabled {
  cursor: default;
}

.mp-indicator:not(:disabled):hover {
  background: var(--bg-card);
  border-color: var(--border);
}

/* 安装中：不旋转，靠颜色（强调色）+ 下方进度条表达"在动"。
   这个与下载那个都不转 —— 旋转容易晃眼，也容易让人以为点了会有别的动作 */
.mp-indicator.busy {
  color: var(--accent);
}

.mp-indicator.failed {
  color: var(--red);
}

.mp-indicator.done {
  color: var(--text-dim);
}

.mp-ind-icon {
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 进度条：压在图标下方那条 2px 的细线 */
.mp-ind-track {
  position: absolute;
  left: 6px;
  right: 6px;
  bottom: 4px;
  height: 2px;
  border-radius: 1px;
  background: var(--bg-hover);
  overflow: hidden;
}

.mp-ind-fill {
  display: block;
  height: 100%;
  border-radius: 1px;
  background: currentColor;
  transition: width 0.3s ease;
}
</style>
