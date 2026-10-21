<script setup lang="ts">
// 自绘标题栏的窗口按钮
//
// - windows 样式：右端三个小图标按钮（最小化 / 最大化 / 关闭），参照网易云音乐
// - macos 样式：左端三个圆点（关闭 / 最小化 / 最大化），符号只在悬停整组时显示
//
// 最大化按钮右键弹「全屏 / 占满屏幕」菜单：两者都是铺满屏幕，区别在任务栏 ——
// 全屏连任务栏一起盖住，占满屏幕是系统最大化、留出任务栏。
//
// 自包含：命令与 quitWindow() 都在内部调用，宿主只需传 style。
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
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

const props = defineProps<{
  /** 标题栏样式；windows 是右端三个图标按钮，macos 是左端三个圆点 */
  style: TitleBarStyle;
}>();

/** 是否最大化：决定第二个按钮是「最大化」还是「还原」 */
const maximized = ref(false);
/** 是否全屏：全屏下第二个按钮同样显示「还原」，点击退出全屏 */
const fullscreen = ref(false);

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

/** 按钮顺序：windows 是 最小化 / 最大化 / 关闭，macos 是 关闭 / 最小化 / 最大化 */
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
/** 菜单左上角，单位 px；渲染后按真实尺寸收进窗口 */
const menuPos = ref({ x: 0, y: 0 });
const menuEl = ref<HTMLElement | null>(null);
/** 菜单与窗口边缘的最小距离，单位 px */
const MENU_MARGIN = 8;

/**
 * 在最大化按钮上右键：菜单贴着按钮右下角弹出
 *
 * 原生贴靠热区会吃掉热区内的鼠标消息，那种情况下由 lib/decoration.ts 派发一个
 * 合成 contextmenu 事件到这里，`currentTarget` 仍是按钮。
 */
async function onMaximizeContextMenu(e: MouseEvent) {
  const btn = e.currentTarget as HTMLElement | null;
  if (!btn) return;
  // 启动画面的标题栏里最大化按钮是 visibility: hidden 的占位，别在那个位置弹菜单
  if (window.getComputedStyle(btn).visibility === "hidden") return;
  const rect = btn.getBoundingClientRect();
  // 先按"右缘对齐按钮右缘"摆，量到真实宽高后再收进窗口
  menuPos.value = { x: rect.right, y: rect.bottom + 4 };
  menuOpen.value = true;
  await nextTick();
  const menuRect = menuEl.value?.getBoundingClientRect();
  if (!menuRect) return;
  menuPos.value = {
    x: Math.max(
      MENU_MARGIN,
      Math.min(rect.right - menuRect.width, window.innerWidth - menuRect.width - MENU_MARGIN),
    ),
    y: Math.min(rect.bottom + 4, window.innerHeight - menuRect.height - MENU_MARGIN),
  };
}

/** 只有最大化按钮开菜单，其它按钮的右键照旧什么都不弹 */
function onButtonContextMenu(e: MouseEvent, id: string) {
  if (id === "maximize") {
    void onMaximizeContextMenu(e);
  }
}

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

function onMenuKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    closeMenu();
  }
}

// 菜单开着时挂两个窗口级监听：Esc 收起、窗口尺寸变了位置就不对了
watch(menuOpen, (open) => {
  if (open) {
    window.addEventListener("keydown", onMenuKeydown, true);
    window.addEventListener("resize", closeMenu);
  } else {
    window.removeEventListener("keydown", onMenuKeydown, true);
    window.removeEventListener("resize", closeMenu);
  }
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
  window.removeEventListener("keydown", onMenuKeydown, true);
  window.removeEventListener("resize", closeMenu);
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

  <!-- 最大化按钮的右键菜单：全屏 / 占满屏幕两种铺满方式。
       Teleport 到 body，免得被标题栏那一层的层叠上下文裁掉 -->
  <Teleport to="body">
    <div v-if="menuOpen" class="wc-menu-layer" @pointerdown.self="closeMenu" @contextmenu.prevent>
      <div ref="menuEl" class="wc-menu" :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }" role="menu">
        <button class="wc-menu-item" :class="{ 'is-active': fullscreen }" role="menuitem" @click="onMenuFullscreen">
          <svg class="wc-menu-check" viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor"
            stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="m2.5 6.2 2.4 2.4 4.6-5.2" />
          </svg>
          {{ t("titlebar.fullscreen") }}
        </button>
        <button class="wc-menu-item" role="menuitem" @click="onMenuFillScreen">
          <svg class="wc-menu-check" viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor"
            stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="m2.5 6.2 2.4 2.4 4.6-5.2" />
          </svg>
          {{ t("titlebar.fillScreen") }}
        </button>
      </div>
    </div>
  </Teleport>
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

/* ---------- 最大化按钮的右键菜单 ---------- */

/* 整屏透明层：接住菜单外的按下与右键，点哪儿都先收起菜单 */
.wc-menu-layer {
  position: fixed;
  inset: 0;
  z-index: 40;
}

.wc-menu {
  position: fixed;
  min-width: 160px;
  padding: 6px;
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  animation: wc-menu-in 0.18s ease;
}

@keyframes wc-menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px) scale(0.98);
  }

  to {
    opacity: 1;
    transform: none;
  }
}

.wc-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s;
}

.wc-menu-item:hover {
  background: var(--bg-hover);
}

/* 勾选位常驻占位：切换时文字不左右跳 */
.wc-menu-check {
  flex-shrink: 0;
  opacity: 0;
  color: var(--accent);
}

.wc-menu-item.is-active .wc-menu-check {
  opacity: 1;
}

.wc-menu-item.is-active {
  color: var(--accent);
}
</style>
