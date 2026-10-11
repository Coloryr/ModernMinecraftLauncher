<script setup lang="ts">
// 自绘标题栏的窗口按钮
//
// - windows 样式：右端三个小图标按钮（最小化 / 最大化 / 关闭），参照网易云音乐
// - macos 样式：左端三个圆点（关闭 / 最小化 / 最大化），符号只在悬停整组时显示
//
// 自包含：命令与 quitWindow() 都在内部调用，宿主只需传 style。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../../lib/i18n";
import { isTauri, quitWindow } from "../../windows/windowManager";
import {
  isWindowMaximized,
  minimizeWindow,
  toggleMaximizeWindow,
  type TitleBarStyle,
} from "../../lib/titlebar";

const props = defineProps<{
  /** 标题栏样式；windows 是右端三个图标按钮，macos 是左端三个圆点 */
  style: TitleBarStyle;
}>();

/** 是否最大化：决定第二个按钮是「最大化」还是「还原」 */
const maximized = ref(false);

async function refresh() {
  maximized.value = await isWindowMaximized();
}

async function onToggleMaximize() {
  const next = await toggleMaximizeWindow();
  if (next !== null) {
    maximized.value = next;
  }
}

/** 按钮顺序：windows 是 最小化 / 最大化 / 关闭，macos 是 关闭 / 最小化 / 最大化 */
const buttons = computed(() => {
  const minimize = {
    id: "minimize",
    label: () => t("titlebar.minimize"),
    run: minimizeWindow,
  };
  const maximize = {
    id: "maximize",
    label: () => t(maximized.value ? "titlebar.restore" : "titlebar.maximize"),
    run: onToggleMaximize,
  };
  const close = {
    id: "close",
    label: () => t("titlebar.close"),
    run: quitWindow,
  };
  return props.style === "macos" ? [close, minimize, maximize] : [minimize, maximize, close];
});

/**
 * 窗口尺寸监听的退订函数
 *
 * 赋值与注册分开：`onUnmounted` 只能在 setup 的**同步阶段**注册 —— 写在 `await`
 * 之后就注册不上了，监听会一直挂着摘不掉 —— 所以退订函数存这里、由同步钩子调用。
 */
let unlistenResize: (() => void) | null = null;

onMounted(async () => {
  if (!isTauri()) {
    return;
  }
  await refresh();
  // 外部操作也会改最大化状态（Win+↑、系统贴靠），窗口尺寸变化时重新查一次
  try {
    unlistenResize = await getCurrentWindow().onResized(() => {
      void refresh();
    });
  } catch {
    /* 拿不到窗口事件时只是图标不更新，不影响按钮本身 */
  }
});

onUnmounted(() => {
  unlistenResize?.();
  unlistenResize = null;
});
</script>

<template>
  <div class="window-controls" :class="style" data-no-drag>
    <button v-for="btn in buttons" :key="btn.id" class="wc-btn" :class="btn.id" v-tip="btn.label()" @click="btn.run()">
      <svg v-if="btn.id === 'minimize'" viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor"
        stroke-width="1.1" stroke-linecap="round">
        <path d="M2.5 6h7" />
      </svg>

      <svg v-else-if="btn.id === 'maximize'" viewBox="0 0 12 12" width="12" height="12" fill="none"
        stroke="currentColor" stroke-width="1.1" stroke-linejoin="round">
        <rect v-if="!maximized" x="2.5" y="2.5" width="7" height="7" rx="1" />
        <template v-else>
          <rect x="2" y="4" width="6" height="6" rx="1" />
          <path d="M4.5 4V3a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-1" />
        </template>
      </svg>

      <svg v-else viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.1"
        stroke-linecap="round">
        <path d="m3 3 6 6M9 3l-6 6" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.window-controls {
  display: flex;
  align-items: center;
  /* 不撑满标题栏高度：按钮是小方块，跟着内容垂直居中即可 */
  align-self: center;
  flex-shrink: 0;
}

.wc-btn {
  border: none;
  background: transparent;
  cursor: default;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ---------- windows 样式：右端三个小图标按钮（参照网易云音乐）----------
   纯图标、无边框无底色，悬停才出现淡圆角底，关闭悬停变红 */

.window-controls.windows {
  gap: 2px;
}

.window-controls.windows .wc-btn {
  width: 34px;
  height: 34px;
  border-radius: 6px;
  color: var(--text-dim);
  transition: background 0.12s, color 0.12s;
}

.window-controls.windows .wc-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}

/* 贴靠热区接管时的悬停态（主窗口）
 *
 * 原生命中区是个真实的 Win32 子窗口，盖在 webview 之上并吃掉 WM_NCMOUSEMOVE，
 * 所以鼠标进到最大化按钮上时 **DOM 的 :hover 不会触发**。插件把该事件转发给我们，
 * 由 decoration.ts 给按钮打上 .snap-hover —— 外观与 :hover 完全一致，两条并列写。
 * 关闭键的红色悬停同理。 */
.window-controls.windows .wc-btn.snap-hover {
  background: var(--bg-hover);
  color: var(--text);
}

.window-controls.windows .wc-btn.close.snap-hover {
  background: #e81123;
  color: #fff;
}

.window-controls.windows .wc-btn.close:hover {
  background: #e81123;
  color: #fff;
}

/* ---------- macos 样式：左端三个圆点 ---------- */

.window-controls.macos {
  align-items: center;
  gap: 8px;
}

.window-controls.macos .wc-btn {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  color: rgb(0 0 0 / 55%);
}

.window-controls.macos .wc-btn.close {
  background: #ff5f57;
}

.window-controls.macos .wc-btn.minimize {
  background: #febc2e;
}

.window-controls.macos .wc-btn.maximize {
  background: #28c840;
}

/* 符号默认不显示，鼠标移到这一组上才出现（与 macOS 一致） */
.window-controls.macos .wc-btn svg {
  opacity: 0;
}

.window-controls.macos:hover .wc-btn svg {
  opacity: 1;
}
</style>
