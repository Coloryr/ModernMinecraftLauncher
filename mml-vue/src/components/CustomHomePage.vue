<script setup lang="ts">
// 自定义主页面：把服主导入的整页 HTML 放进 iframe 顶替内置主页
//
// 故意不给 allow-same-origin：iframe 的 origin 保持 opaque，页面拿不到启动器页面的
// localStorage / cookie；postMessage 在 opaque origin 下照常可用。
// 页面里的 window.mml 由 Rust 注入的 custom_home_bridge.js 提供，含 invoke / on / ready；
// 父窗口侧的处理在 lib/customHomeBridge.ts。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { attachCustomHomeBridge } from "../lib/customHomeBridge";
import { resolveHomePage } from "../lib/customHomeUi";

defineProps<{
  /** iframe 的 src，即服主导入的那份整页 HTML 的入口地址 */
  entryUrl: string;
}>();

const frame = ref<HTMLIFrameElement | null>(null);
let detach: (() => void) | null = null;

/** 包里 custom.js 注册的组件；有它就不开 iframe */
const page = computed(() => resolveHomePage());

/** 宿主主题；组件样式挂在 .mml-home 上，靠这个属性切深浅色 */
const theme = ref<"Dark" | "Light">(
  (window as unknown as { mml?: { theme?: string } }).mml?.theme === "Light" ? "Light" : "Dark",
);
let offTheme: (() => void) | null = null;

onMounted(() => {
  if (frame.value) detach = attachCustomHomeBridge(frame.value);
  const mml = (window as unknown as { mml?: { onTheme?: (cb: (v: "Dark" | "Light") => void) => () => void } }).mml;
  offTheme = mml?.onTheme?.((value) => {
    theme.value = value;
  }) ?? null;
});

onUnmounted(() => {
  detach?.();
  detach = null;
  offTheme?.();
  offTheme = null;
});
</script>

<template>
  <div v-if="page" class="mml-home" :data-mml-theme="theme">
    <component :is="page" />
  </div>
  <iframe v-else ref="frame" class="custom-home-frame" :src="entryUrl"
    sandbox="allow-scripts allow-forms allow-popups allow-modals"></iframe>
</template>

<style scoped>
/* 铺满内容区：父容器都是确定高度的纵向 flex，即 .custom-home-page / .custom-home-fill，
   高度靠 flex:1 吃满，height:100% 只是兜底 */
.custom-home-frame {
  flex: 1;
  width: 100%;
  height: 100%;
  min-height: 0;
  border: 0;
  display: block;
  background: var(--bg);
}

/* 组件自己滚动，避免长页面把主窗口撑开 */
.mml-home {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.mml-home :deep(> *) {
  min-height: 100%;
}
</style>
