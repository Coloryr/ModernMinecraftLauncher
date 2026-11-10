<script setup lang="ts">
// 自绘标题栏的窗口按钮
//
// - windows 样式：右端三个小图标按钮（最小化 / 最大化 / 关闭），参照网易云音乐
// - macos 样式：左端三个圆点（关闭 / 最小化 / 最大化），符号只在悬停整组时显示
//
// 最大化按钮的右键菜单拆在 WindowMaximizeMenu：这里只管按钮、开关状态与动作，
// 定位 / Esc / 点外面收起都由菜单自己处理。
// 自包含：命令与 quitWindow() 都在内部调用，宿主只需传 style。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../../lib/i18n";
import { isTauri, quitWindow } from "../../windows/windowManager";
import {
  fillScreenWindow,
  isWindowFullscreen,
  isWindowMaximized,
  minimizeWindow,
  setWindowFullscreen,
  toggleMaximizeWindow,
  type TitleBarStyle,
} from "../../lib/titlebar";
import WindowMaximizeMenu from "./WindowMaximizeMenu.vue";

const props = defineProps<{
  /** 标题栏样式；windows 是右端三个图标按钮，macos 是左端三个圆点 */
  style: TitleBarStyle;
}>();

/** 是否最大化：决定第二个按钮是「最大化」还是「还原」 */
const maximized = ref(false);
/** 是否全屏：全屏下第二个按钮同样显示「还原」，点击退出全屏 */
const fullscreen = ref(false);

/** 重查最大化 / 全屏状态 */
async function refresh() {
  const [max, full] = await Promise.all([isWindowMaximized(), isWindowFullscreen()]);
  maximized.value = max;
  fullscreen.value = full;
}

/** 点第二个按钮：全屏时退出全屏，否则最大化 / 还原 */
async function onToggleMaximize() {
  closeMenu();
  if (fullscreen.value) {
    await setWindowFullscreen(false);
    await refresh();
    return;
  }
  const next = await toggleMaximizeWindow();
  if (next !== null) {
    maximized.value = next;
  }
}

/** 第二个按钮的悬停提示：全屏时说明点下去是退出全屏 */
const maximizeLabel = computed(() => {
  if (fullscreen.value) return t("titlebar.exitFullscreen");
  return t(maximized.value ? "titlebar.restore" : "titlebar.maximize");
});

/**
 * 按钮顺序与动作
 *
 * # 返回值
 *
 * windows 是 最小化 / 最大化 / 关闭，macos 是 关闭 / 最小化 / 最大化
 */
const buttons = computed(() => {
  const minimize = {
    id: "minimize",
    label: () => t("titlebar.minimize"),
    run: minimizeWindow,
  };
  const maximize = {
    id: "maximize",
    label: () => maximizeLabel.value,
    run: onToggleMaximize,
  };
  const close = {
    id: "close",
    label: () => t("titlebar.close"),
    run: quitWindow,
  };
  return props.style === "macos" ? [close, minimize, maximize] : [minimize, maximize, close];
});

// ---------- 最大化按钮的右键菜单 ----------

/** 菜单是否展开 */
const menuOpen = ref(false);
/** 菜单锚点：右键的那枚最大化按钮 */
const menuAnchor = ref<HTMLElement | null>(null);

/**
 * 在最大化按钮上右键：记下锚点并打开菜单
 *
 * 原生贴靠热区会吃掉热区内的鼠标消息，那种情况下由 lib/decoration.ts 派发一个
 * 合成 contextmenu 事件到这里，`currentTarget` 仍是按钮。
 */
function onMaximizeContextMenu(e: MouseEvent) {
  const btn = e.currentTarget as HTMLElement | null;
  if (!btn) return;
  // 启动画面的标题栏里最大化按钮是 visibility: hidden 的占位，别在那个位置弹菜单
  if (window.getComputedStyle(btn).visibility === "hidden") return;
  menuAnchor.value = btn;
  menuOpen.value = true;
}

/** 只有最大化按钮开菜单，其它按钮的右键照旧什么都不弹 */
function onButtonContextMenu(e: MouseEvent, id: string) {
  if (id === "maximize") {
    onMaximizeContextMenu(e);
  }
}

/** 收起最大化按钮的右键菜单 */
function closeMenu() {
  menuOpen.value = false;
}

/** 菜单「全屏」：当前是全屏就退出，否则进入全屏 */
async function onMenuFullscreen() {
  closeMenu();
  await setWindowFullscreen(!fullscreen.value);
  await refresh();
}

/** 菜单「占满屏幕」：窗口挪到工作区左上角并拉到工作区大小，不进入最大化状态 */
async function onMenuFillScreen() {
  closeMenu();
  await fillScreenWindow();
  await refresh();
}

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
  // 外部操作也会改最大化 / 全屏状态（Win+↑、系统贴靠），窗口尺寸变化时重新查一次
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
    <button v-for="btn in buttons" :key="btn.id" class="wc-btn" :class="btn.id" v-tip="btn.label()" @click="btn.run()"
      @contextmenu.prevent="onButtonContextMenu($event, btn.id)">
      <svg v-if="btn.id === 'minimize'" viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor"
        stroke-width="1.1" stroke-linecap="round">
        <path d="M2.5 6h7" />
      </svg>

      <svg v-else-if="btn.id === 'maximize'" viewBox="0 0 12 12" width="12" height="12" fill="none"
        stroke="currentColor" stroke-width="1.1" stroke-linejoin="round">
        <rect v-if="!maximized && !fullscreen" x="2.5" y="2.5" width="7" height="7" rx="1" />
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

  <!-- 最大化按钮右键菜单：全屏 / 占满屏幕；外观与收起都在组件里 -->
  <WindowMaximizeMenu v-if="menuOpen" :anchor="menuAnchor" :fullscreen="fullscreen" @close="closeMenu"
    @toggle-fullscreen="onMenuFullscreen" @fill-screen="onMenuFillScreen" />
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
  background: var(--window-close);
  color: var(--on-accent);
}

.window-controls.windows .wc-btn.close:hover {
  background: var(--window-close);
  color: var(--on-accent);
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
  color: var(--overlay);
}

.window-controls.macos .wc-btn.close {
  background: var(--mac-red);
}

.window-controls.macos .wc-btn.minimize {
  background: var(--mac-yellow);
}

.window-controls.macos .wc-btn.maximize {
  background: var(--mac-green);
}

/* 符号默认不显示，鼠标移到这一组上才出现（与 macOS 一致） */
.window-controls.macos .wc-btn svg {
  opacity: 0;
}

.window-controls.macos:hover .wc-btn svg {
  opacity: 1;
}
</style>
