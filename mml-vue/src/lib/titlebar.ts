// 自绘标题栏：平台判定 + 拖动 / 最大化助手
//
// 两套样式：`windows` 右端三个方按钮，`macos` 左端红黄绿圆点。
// 4 月 1 日对调两套样式，愚人节彩蛋。
//
// 窗口动作都走应用自己的 Rust 命令，见 src-tauri 的 windows/mod.rs 末尾，不走 JS 的
// `getCurrentWindow()`，省得往 capabilities/default.json 加窗口权限。
// 唯一的例外是「关闭」——它复用 windowManager 的 quitWindow()，那条链路才会跑
// 关闭保护（下载中 / 查询中拒关）与几何保存。收起当前页是 closeWindow()，别混。

import { onActivated, onMounted, watch } from "vue";
import { commands } from "./bindings";
import { isTauri } from "../windows/windowManager";

export type OsKind = "windows" | "linux" | "macos";
export type TitleBarStyle = "windows" | "macos";

/**
 * 让原生窗口标题（任务栏 / Alt+Tab）跟着自绘标题栏走
 *
 * 关键是**每次激活都要重设**，不能只在挂载时设一次：单窗口模式下窗口组件被
 * `<KeepAlive>` 缓存，切走再回来只是 deactivate / activate，**不会重新挂载** ——
 * 只在 `onMounted` 里设标题的写法再也不会执行，于是从子窗口切回主页面时，
 * 任务栏上仍挂着子窗口的标题。
 *
 * - `title`: 取值函数，每次现取，语言切换后拿到的是新文案
 */
export function useWindowTitle(title: () => string) {
  function apply() {
    if (!isTauri()) return;
    commands.windows.setTitle(title()).catch(() => { });
  }

  // 标题本身变化（语言切换 / 窗口内标题变化）
  watch(title, apply);
  // KeepAlive 内：挂载与每次切回都会触发。两类都挂上，
  // 不依赖调用方是否恰好被缓存（重复设一次同样的标题无副作用）
  onMounted(apply);
  onActivated(apply);
}

/**
 * 运行平台
 *
 * 用 UA 判定，Tauri 与浏览器预览都可用，不必引入 tauri-plugin-os 依赖，
 * 也不必加一个异步命令。
 *
 * # 返回值
 *
 * `windows` / `linux` / `macos`
 */
export function detectOs(): OsKind {
  const ua = navigator.userAgent;
  if (/Macintosh|Mac OS X/.test(ua)) {
    return "macos";
  }
  if (/Linux|X11/.test(ua)) {
    return "linux";
  }
  return "windows";
}

/**
 * 是否 4 月 1 日
 *
 * - `now`: 判定的时刻，缺省为当前时间
 *
 * # 返回值
 *
 * 是 4 月 1 日时为 true
 */
export function isAprilFools(now = new Date()): boolean {
  return now.getMonth() === 3 && now.getDate() === 1;
}

/** 本次运行使用的标题栏样式：启动时定一次，会话中不随其它状态变化 */
export const titleBarStyle: TitleBarStyle = (() => {
  const isMac = detectOs() === "macos";
  return isAprilFools() ? (isMac ? "windows" : "macos") : isMac ? "macos" : "windows";
})();

/** 标题栏上按下时不应触发拖动 / 最大化的元素 */
const INTERACTIVE = "button, a, input, select, textarea, label, [data-no-drag]";

/** 判定双击的最大间隔 */
const DOUBLE_CLICK_MS = 400;

function isInteractive(target: EventTarget | null): boolean {
  // 注意用 Element 而非 HTMLElement：按钮内是 SVG 图标，
  // 点击目标是 SVGElement，instanceof HTMLElement 会判定为非交互元素，
  // 导致 startDragging 吞掉按钮的 click，这是关闭 / 最小化按钮失灵的直接原因
  return target instanceof Element && target.closest(INTERACTIVE) !== null;
}

let lastDownAt = 0;
let lastDownTarget: EventTarget | null = null;

/**
 * 标题栏按下：单击拖动窗口，双击最大化 / 还原
 *
 * 双击在这里自己判：`start_dragging()` 会把指针交给系统，之后的 `dblclick`
 * 事件不一定能到，所以不能依赖 `@dblclick`。
 *
 * - `e`: 标题栏上的指针按下事件
 */
export function onTitleBarPointerDown(e: PointerEvent) {
  if (!isTauri() || e.button !== 0 || isInteractive(e.target)) {
    return;
  }

  const now = Date.now();
  if (now - lastDownAt < DOUBLE_CLICK_MS && e.target === lastDownTarget) {
    // 双击：只在这一次里最大化，且不再起拖拽
    lastDownAt = 0;
    lastDownTarget = null;
    commands.windows.toggleMaximize().catch(() => { });
    return;
  }

  lastDownAt = now;
  lastDownTarget = e.target;
  commands.windows.startDragging().catch(() => { });
}

/** 最小化当前窗口 */
export function minimizeWindow() {
  if (!isTauri()) {
    return;
  }
  commands.windows.minimize().catch(() => { });
}

/**
 * 最大化 / 还原当前窗口
 *
 * # 返回值
 *
 * 切换后的最大化状态；非 Tauri 环境或调用失败时为 `null`
 */
export async function toggleMaximizeWindow(): Promise<boolean | null> {
  if (!isTauri()) {
    return null;
  }
  try {
    return await commands.windows.toggleMaximize();
  } catch {
    return null;
  }
}

/**
 * 当前是否最大化
 *
 * # 返回值
 *
 * 非 Tauri 环境或调用失败时为 false
 */
export async function isWindowMaximized(): Promise<boolean> {
  if (!isTauri()) {
    return false;
  }
  try {
    return await commands.windows.isMaximized();
  } catch {
    return false;
  }
}

/**
 * 设置窗口全屏：盖住任务栏，退出时由系统恢复进入前的摆放
 *
 * - `fullscreen`: 目标状态，true 进入全屏，false 退出
 */
export async function setWindowFullscreen(fullscreen: boolean): Promise<void> {
  if (!isTauri()) {
    return;
  }
  try {
    await commands.windows.setFullscreen(fullscreen);
  } catch {
    // 失败时保持原状，调用方随后的 refresh 会把图标纠正回来
  }
}

/**
 * 当前是否全屏
 *
 * # 返回值
 *
 * 非 Tauri 环境或调用失败时为 false
 */
export async function isWindowFullscreen(): Promise<boolean> {
  if (!isTauri()) {
    return false;
  }
  try {
    return await commands.windows.isFullscreen();
  } catch {
    return false;
  }
}

/**
 * 把窗口铺满当前显示器的工作区：左上角对齐，客户区取工作区尺寸
 *
 * 不是最大化 —— 不进入系统最大化状态，摆好之后照常能拖动 / 缩放；
 * 原本全屏或最大化时后端会先退出来。
 */
export async function fillScreenWindow(): Promise<void> {
  if (!isTauri()) {
    return;
  }
  try {
    await commands.windows.fillScreen();
  } catch {
    // 失败时保持原状
  }
}
