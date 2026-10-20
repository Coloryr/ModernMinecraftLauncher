<script setup lang="ts">
// 整页重写：把自定义页面铺满整个主窗口
//
// iframe 故意不给 allow-same-origin，origin 保持 opaque；页面里的 window.mml 由 Rust 注入的
// custom_home_bridge.js 提供，父窗口侧处理复用 lib/customHomeBridge.ts。
import { onMounted, onUnmounted, ref } from "vue";
import { attachCustomHomeBridge } from "../lib/customHomeBridge";
import { exitCustomUi } from "../lib/customUi";
import { t } from "../lib/i18n";

defineProps<{
  /** 自定义页面的入口地址：导入的整页包由 mml-ui 协议伺服，开发时也可是 http */
  url: string;
}>();

const frame = ref<HTMLIFrameElement | null>(null);
let detach: (() => void) | null = null;

onMounted(() => {
  if (frame.value) detach = attachCustomHomeBridge(frame.value);
});

onUnmounted(() => {
  detach?.();
  detach = null;
});

/** 开发兜底：页面地址填错时还能退回内置界面，生产构建不显示 */
const devEscape = import.meta.env.DEV;
</script>

<template>
  <iframe ref="frame" class="custom-ui-frame" :src="url"
    sandbox="allow-scripts allow-forms allow-popups allow-modals"></iframe>
  <!-- 开发期逃生口：生产构建里没有这一颗 -->
  <button v-if="devEscape" class="ui-btn s-sm dev-escape" @click="exitCustomUi">
    {{ t("winSettings.customUiExit") }}
  </button>
</template>

<style scoped>
/* 铺满整个 webview，盖住内置界面；后端把页面地址换成自定义界面协议后行为不变 */
.custom-ui-frame {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
  border: 0;
  background: var(--bg);
}

/* 只给开发期用：浮在 iframe 之上，页面自己画了什么都能点回来 */
.dev-escape {
  position: fixed;
  right: 8px;
  bottom: 8px;
  z-index: 300;
}
</style>
