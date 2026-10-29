<script setup lang="ts">
// 资源下载进度：标题栏上的动态图标 + 细进度条（点击展开进度弹窗）
//
// 与 ModpackTitleIndicator 同一套做法：原来是内容区顶部的一张进度卡片（ResourceDownloadBar），
// 一直占着内容区高度、把列表顶下去；改成标题栏指示器后不占内容区，位置也与另外两个指示器并排。
//
// 挂载位置两种窗口模式都有：单窗口模式的主页面顶栏（MainTopbar）与各功能窗口的标题栏
// （WindowFrame 的 head-right 插槽）各挂一份 —— 切到哪一页都能看到进度。
//
// 自包含状态：自己订阅资源下载任务，不依赖父组件传参，两处挂载点都能直接放。
// 订阅用 own=true：那是"由我负责显示"的语义，不受"添加资源窗口是否开着"那条过滤影响，
// 否则从那个窗口里发起的下载在别处就看不到进度了。
import { computed, onMounted, onUnmounted } from "vue";
import { t } from "../lib/i18n";
import { resourceOverallPercent } from "../lib/progress";
import { useResourceStatus } from "../lib/resourceTasks";
import { openResourcePopup } from "../windows/windowManager";
import type { ResourceTaskDto } from "../lib/bindings";

const { status, init } = useResourceStatus(true);

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

/** 还没结束的任务 */
function isRunning(task: ResourceTaskDto): boolean {
  return !task.done && !task.failed;
}

const running = computed(() => tasks.value.filter(isRunning));

/** 有没有失败的任务（图标据此变成警示色） */
const hasFailed = computed(() => tasks.value.some((task) => task.failed));

/** 总进度，与进度卡片同一口径，见 lib/progress.ts */
const percent = computed(() => resourceOverallPercent(tasks.value));

/** 悬停提示：进行中报进度，结束后报结果 */
const tipText = computed(() => {
  if (running.value.length) {
    return `${t("addResource.bar.running", { count: running.value.length })} · ${Math.round(percent.value)}%`;
  }
  return t("addResource.bar.done");
});

/** 图标状态：下载中、失败警示、全部完成常态 */
const iconState = computed(() => {
  if (running.value.length) return "busy";
  return hasFailed.value ? "failed" : "done";
});
</script>

<template>
  <!-- 没有任务时整块不存在；任务全部结束后后端会清表，它自己就消失了 -->
  <button v-if="tasks.length" class="rs-indicator" :class="iconState" v-tip="tipText"
    :aria-label="t('addResource.bar.title')" @click="openResourcePopup()">
    <span class="rs-ind-icon">
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.9"
        stroke-linecap="round" stroke-linejoin="round">
        <!-- 墨迹范围与另外两个指示器一致（y 3–21、居中）：原来那版画到 y 18，看着偏高 -->
        <path d="m12 3 9 6-9 6-9-6 9-6Z" />
        <path d="m3 15 9 6 9-6" />
      </svg>
    </span>
    <!-- 进度：细条贴在图标下沿。进行中才显示（结束是满格，没信息量） -->
    <span v-if="running.length" class="rs-ind-track">
      <span class="rs-ind-fill" :style="{ width: percent + '%' }" />
    </span>
  </button>
</template>

<style scoped>
/* 尺寸与圆角见 styles/titlebar.css 的方形图标键，这里只留外观 */
.rs-indicator {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 1px solid transparent;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

.rs-indicator:hover {
  background: var(--bg-card);
  border-color: var(--border);
}

/* 下载中：不旋转，靠颜色（强调色）+ 下方进度条表达"在动" ——
   三个指示器并排，都不转，旋转容易晃眼 */
.rs-indicator.busy {
  color: var(--accent);
}

.rs-indicator.failed {
  color: var(--red);
}

.rs-indicator.done {
  color: var(--text-dim);
}

.rs-ind-icon {
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 进度条：压在图标下方那条 2px 的细线 */
.rs-ind-track {
  position: absolute;
  left: 6px;
  right: 6px;
  bottom: 4px;
  height: 2px;
  border-radius: 1px;
  background: var(--bg-hover);
  overflow: hidden;
}

.rs-ind-fill {
  display: block;
  height: 100%;
  border-radius: 1px;
  background: currentColor;
  transition: width 0.3s ease;
}
</style>
