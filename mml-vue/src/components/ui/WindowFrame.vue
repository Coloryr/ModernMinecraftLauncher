<script setup lang="ts">
// 子窗口通用框架：标题（兼作自绘标题栏）+ 内容区
// 头部整条可拖动，两端按平台样式放窗口按钮；「返回主页面」按钮只在单窗口模式下显示。
import { computed, ref } from "vue";
import { t } from "../../lib/i18n";
import { currentKind, multiWindow } from "../../windows/windowManager";
import WindowControls from "./WindowControls.vue";
import ModpackTitleIndicator from "../ModpackTitleIndicator.vue";
import DownloadTitleIndicator from "../DownloadTitleIndicator.vue";
import ResourceTitleIndicator from "../ResourceTitleIndicator.vue";
import MusicTitleIndicator from "../MusicTitleIndicator.vue";
import { onTitleBarPointerDown, titleBarStyle, useWindowTitle } from "../../lib/titlebar";
import { useWindowDecoration } from "../../lib/useWindowDecoration";

const props = defineProps<{
  /** 标题栏文案，同时作为原生窗口标题 */
  title: string;
  /** 内容区不整页滚动，即 overflow hidden + flex 列，滚动交给视图内部的容器；
   *  详情表格这类"工具栏固定、表格自己滚"的布局用 */
  bodyFill?: boolean;
  /** 内容区满铺：内边距归零，排版与留白交给窗口自己。
   *  设置窗口那种"左侧导航顶到窗口边缘 + 右侧内容自己滚"的两栏布局用，
   *  与 `bodyFill` 叠加时不用再拿负外边距去抵消 `.frame-body` 的内边距 */
  bodyFlush?: boolean;
  /** 内容区**整页滚动**、且内容长短会变时开启：给内容区预留纵向滚动条的位置，
   *  免得滚动条一冒出来整页内容就左移 9px，见 styles/scrollbar.css 的 scrollbar-gutter。
   *  只有"滚动容器就是 `.frame-body` 本身"的窗口才需要开 —— 资源 / 添加资源 /
   *  下载 Java 这几个内容区是 `height: 100%` 布局、滚动交给内部容器，开了只会白窄 9px */
  bodyGutter?: boolean;
  /** 窗口内子页面的返回键文案，如"返回"；不传则用单窗口模式那枚「返回上一页」。
   *  按钮本身只画箭头，这段文案作为悬停提示 v-tip 与无障碍名称存在；
   *  两种用法共用同一个 `.back-btn`，外观完全一致 */
  back?: string;
  /** 是否在标题栏显示整合包安装进度指示（默认显示）。
   *  整合包窗口整页就是进度，给它关掉，免得同一件事在标题栏里再出现一次 */
  hideModpackIndicator?: boolean;
  /** 是否显示下载进度指示（默认显示）。下载窗口整页就是下载管理，同样关掉 */
  hideDownloadIndicator?: boolean;
}>();

/**
 * 下载 / 整合包两个进度指示器挂在下载整合包、添加实例、资源管理与**实例日志**四个窗口
 *
 * 它们自己就会发起下载：装整合包、导入在线实例（服务器包内容）、更新模组；日志窗口是
 * **启动游戏**的地方（标题栏那颗测试启动键），启动前的缺失文件补齐走的是同一个下载器。
 * 其它窗口没必要挂。下载整合包窗口关掉之后它们回主页面，见 MainTopbar 的 modpackWindowOpen。
 */
const showTaskIndicators = computed(() => {
  const kind = currentKind.value;
  return kind === "add" || kind === "add_modpack" || kind === "resource" || kind === "log";
});

const emit = defineEmits<{
  /** 返回按钮：单窗口模式下回上一层 */
  (e: "close"): void;
  /** 窗口内子页面的返回键 */
  (e: "back"): void;
}>();

/** 单窗口模式才显示返回按钮，浏览器与 Tauri 都一样：
 *  这时功能页都在同一个窗口里切换，"返回"＝回上一层，返回栈见 windowManager；
 *  多窗口模式下每个功能是独立窗口，关掉它自然露出下面那个，不需要返回键 */
const showBack = computed(() => !multiWindow.value);

// 原生窗口标题（任务栏 / Alt+Tab）跟随自绘标题栏文案。
// 必须走 useWindowTitle：它**每次激活都重设**，而不是只在挂载时设一次 ——
// 单窗口模式下本页被 KeepAlive 缓存，切走再回来不重新挂载，只设一次的话
// 从别的页面切回来时，任务栏上还挂着上一个页面的标题
useWindowTitle(() => props.title);

// 窗口装饰：激活插件装饰（原生 frame → 自绘）+ 把贴靠热区交给插件。
// 每个功能窗口都要做，放这里统一处理，见 lib/useWindowDecoration.ts
const rootEl = ref<HTMLElement | null>(null);
const { decorated } = useWindowDecoration(rootEl);

// 内容区元素：整页滚动的窗口（如统计窗口）把它交给行虚拟化当滚动容器，
// 表格只渲染视口附近那几行，页面整体滚动的行为不变
const bodyEl = ref<HTMLElement | null>(null);
defineExpose({ bodyEl });
</script>

<template>
  <div class="window-frame" ref="rootEl">
    <header class="frame-head" :class="titleBarStyle" @pointerdown="onTitleBarPointerDown">
      <!-- macos 样式：红黄绿在左端。插件接管时由它画原生圆点，这里不重复渲染 -->
      <WindowControls v-if="titleBarStyle === 'macos' && !decorated" :style="titleBarStyle" />

      <!-- 窗口内子页面的返回键，如整合包详情：与下面的单窗口返回按钮共用一套外观。
           按钮只画箭头，文案改由悬停提示给出，见 .back-btn 的说明 -->
      <button v-if="props.back" class="back-btn" v-tip="props.back" :aria-label="props.back" @click="emit('back')">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.2"
          stroke-linecap="round">
          <path d="m15 18-6-6 6-6" />
        </svg>
      </button>
      <!-- 单窗口模式才显示返回按钮；子页面自带返回键时让位 -->
      <button v-else-if="showBack" class="back-btn" v-tip="t('winCommon.back')" :aria-label="t('winCommon.back')"
        @click="emit('close')">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.2"
          stroke-linecap="round">
          <path d="m15 18-6-6 6-6" />
        </svg>
      </button>
      <h1>{{ title }}</h1>
      <span class="spacer"></span>
      <!-- 标题栏右侧扩展区（如查询进度指示） -->
      <slot name="head-right" />
      <!-- 进度指示器：跟着**当时显示的那条标题栏**走。
           多窗口模式下各功能是独立窗口，自然挂各自的；单窗口模式下功能页也各有自己的
           标题栏，主页面才是 MainTopbar，所以这里照样得挂 —— 之前只在 multiWindow 时挂，
           结果从"下载整合包"返回"添加实例"后两个指示器都不见了。
           没有任务时各自整块不存在 -->
      <ModpackTitleIndicator v-if="showTaskIndicators && !hideModpackIndicator" />
      <DownloadTitleIndicator v-if="showTaskIndicators && !hideDownloadIndicator" />
      <ResourceTitleIndicator />
      <!-- 自定义音乐：只有持有播放器的那个窗口会画出来（多窗口模式 = 主窗口） -->
      <MusicTitleIndicator />

      <!-- windows 样式：最小化 / 最大化 / 关闭在右端。
           插件不再画自己的按钮（vendor patch），热区按这三颗的位置摆，
           所以这里照常渲染，悬停反馈由插件转发的事件驱动 -->
      <WindowControls v-if="titleBarStyle === 'windows'" :style="titleBarStyle" />
    </header>
    <div ref="bodyEl" class="frame-body" :class="{ fill: props.bodyFill, gutter: props.bodyGutter, flush: props.bodyFlush }">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.window-frame {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
}

.frame-head {
  height: var(--titlebar-h);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  /* 标题栏内元素间距统一 8px：各窗口自己在槽里再包一层时用的也是 8px，见 styles/titlebar.css */
  gap: 8px;
  padding: 0 18px;
  background: var(--bg-side);
  border-bottom: 1px solid var(--border);
}

/* windows 样式的窗口按钮是小方块、不贴边，右侧留一点呼吸空间；
   macos 样式的圆点留在左内边距之后 */
.frame-head.windows {
  padding-right: 10px;
}

/* 幽灵图标按钮：只有箭头，无边框、无底色，悬停才浮出一层淡底并提亮箭头——
   与右侧窗口按钮"平时透明、悬停才出底"同一套逻辑。
   文案"返回"不再画在按钮上，改由模板里的 v-tip 悬停提示给出。
   单窗口的返回与窗口内子页面的返回共用这一套外观 */
.back-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ctl-h);
  height: var(--ctl-h);
  padding: 0;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.back-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.frame-head h1 {
  font-size: 16px;
  font-weight: 700;
  /* 标题不被挤掉：窗口窄时先压右边的上下文与按钮，资源窗口那个实例 chip 就是这么
     挤掉过 "资源管理" 的，标题本身不缩、不换行 */
  flex-shrink: 0;
  white-space: nowrap;
}

.spacer {
  flex: 1;
}

.frame-body {
  flex: 1;
  overflow-y: auto;
  padding: 22px 26px;
}

/* 整页滚动的窗口（`body-gutter`）：预留那条 9px，滚动条出现时不挤窄内容；
   右边距同时减掉这 9px，内容右边缘因此仍落在 26px 上（与左边、与不滚动的标题栏对称），
   滚动条被画进这条外边距里，不再从内容盒里吃掉一条。
   两句必须成对，理由与踩过的坑见 styles/scrollbar.css。
   不加在 .frame-body 上无条件生效 —— 多数窗口的内容区是 height: 100% 布局、
   滚动交给内部容器，那些窗口的 .frame-body 根本不滚，预留只会白窄 9px */
.frame-body.gutter {
  scrollbar-gutter: stable;
  padding-right: calc(26px - 9px);
}

/* 不整页滚动的窗口 `body-fill`：显式取消预留。
   **必须写**：scrollbar-gutter 不看"这会儿有没有滚动条"，overflow: hidden 一样占位 9px，
   实测如此。账户窗口还会在列表 / 详情之间切换 fill，不取消就会随视图切换忽宽忽窄。
   上面那条边距补偿要一起取消：这里没有槽位，减掉的 9px 就真少了。
   这条要放在 .gutter 之后：两者特异性相同，靠顺序让 .fill 赢 */
.frame-body.fill {
  overflow: hidden;
  display: flex;
  flex-direction: column;
  scrollbar-gutter: auto;
  padding-right: 26px;
}

/* 满铺内容区 `body-flush`：内边距全部归零，留白与滚动都由窗口自己安排
   （设置窗口的左侧导航要顶到窗口边缘，右侧内容自己滚）。
   放上面两条之后：特异性相同，靠顺序把左右（含 fill 补回的 26px）一起清掉 */
.frame-body.flush {
  padding: 0;
}
</style>
