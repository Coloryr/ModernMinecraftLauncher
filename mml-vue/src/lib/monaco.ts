// Monaco 环境：worker 分派 + 编辑器主题
//
// 主题两套来源：日志区用自己的控制台主题（applyLogTheme），代码编辑器用 Monaco 自带的
// vs / vs-dark（applyCodeTheme，与官网示例一致）。Monaco 的主题是**全局**的，
// 所以两个视图在挂载与激活时各刷一次自己的那套（单窗口模式下 KeepAlive 会同时留着两者）。
//
// 只加载 editor.api（不含 editor.main），语言包由用到的视图自己按需注册
// （配置窗口那份在 lib/monacoCode.ts；日志是纯文本，什么都不用注册）。
// vite 的 ?worker 导入在 dev / build 下都能正确打包。

import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import * as monaco from "monaco-editor/editor/editor.api";
// 查找框与剪贴板命令不在 editor.api 里，要按需引入：没引 find 时日志区按 Ctrl+F
// 会落到浏览器自带的页内查找，右键菜单里的「复制」也没有对应动作。
// codicon 字体单独引一份 —— 查找框的图标全靠它，缺了就显示成方框
import "monaco-editor/features/find/register.js";
import "monaco-editor/features/codicon/register.js";
import "monaco-editor/editor/contrib/find/browser/findController.js";
import "monaco-editor/editor/contrib/clipboard/browser/clipboard.js";
import { resolvedTheme } from "./theme";

declare global {
  interface Window {
    MonacoEnvironment?: monaco.Environment;
  }
}

// 查找框按钮自带的悬停提示在 WebView2 里会反复显示 / 隐藏：Monaco 的悬停浮层带
// IntersectionObserver，被判定不可见就立刻收起，鼠标还在按钮上又重新弹出。
// 在捕获阶段截掉 .find-widget 内的 mouseover，改用 aria-label 当原生 title ——
// 提示还在，但不会闪；CSS 的 :hover 反馈、按钮点击都不受影响。
document.addEventListener(
  "mouseover",
  (e) => {
    const el = e.target as Element | null;
    if (!el?.closest(".find-widget")) return;
    e.stopPropagation();
    const button = el.closest("[aria-label]") as HTMLElement | null;
    if (button && !button.title) {
      button.title = button.getAttribute("aria-label") ?? "";
    }
  },
  true,
);

self.MonacoEnvironment = {
  /**
   * 按语言给 worker
   *
   * - `_id`: Monaco 的 worker 分组 id，本仓库不用
   * - `label`: Monaco 传进来的语言 id
   *
   * # 返回值
   *
   * worker 实例；json 语言返回异步加载的 Promise
   *
   * json 的语言服务有独立 worker，按需异步加载：日志视图用不到它，
   * 静态 import 会把这份 worker 塞进日志那个包，白白多几百 KB。
   */
  getWorker(_id, label) {
    if (label === "json") {
      return import("monaco-editor/language/json/json.worker.js?worker").then(
        (m) => new m.default(),
      );
    }
    return new EditorWorker();
  },
};

/** 日志控制台的暗色主题名 */
const THEME_CONSOLE_DARK = "mml-console-dark";
/** 日志控制台亮色主题名 */
const THEME_CONSOLE_LIGHT = "mml-console-light";

let themeDefined = false;

/** 注册日志控制台的两套主题，在 `editor.create` 之前调一次 */
export function ensureMonacoThemes() {
  if (themeDefined) return;
  themeDefined = true;

  // 暗色：比卡片底色再深一档的控制台观感
  monaco.editor.defineTheme(THEME_CONSOLE_DARK, {
    base: "vs-dark",
    inherit: true,
    // 日志语法的 token 色，对应 lib/monacoLog 里注册的 log 语言；整行底色在 MonacoLogView
    rules: [
      { token: "log.date", foreground: "6b7280" },
      { token: "log.verbose", foreground: "6e7681" },
      { token: "log.debug", foreground: "8b949e" },
      { token: "log.info", foreground: "3fb950" },
      { token: "log.warning", foreground: "d29922" },
      { token: "log.error", foreground: "f85149" },
      { token: "log.constant", foreground: "79c0ff" },
      { token: "log.string", foreground: "a5d6ff" },
      { token: "log.exception", foreground: "f0883e" },
      { token: "log.exceptiontype", foreground: "f0883e" },
    ],
    colors: {
      "editor.background": "#0d0f12",
      // 行号比正文暗一档、比底色亮：原来给的是 #3a4048，在 #0d0f12 上几乎看不见
      "editorLineNumber.foreground": "#6b7280",
      "editorGutter.background": "#0d0f12",
      "scrollbarSlider.background": "#2a2f3880",
      "scrollbarSlider.hover.background": "#3a404880",
      "scrollbarSlider.active.background": "#4a515c80",
    },
  });

  // 亮色：白底，即 themes.css 亮色的卡片色 --bg-card（#ffffff）。
  // 日志区是一块带边框的面板，底色跟卡片走，不跟窗口底色 --bg（#eef1f5）
  monaco.editor.defineTheme(THEME_CONSOLE_LIGHT, {
    base: "vs",
    inherit: true,
    rules: [
      { token: "log.date", foreground: "8c959f" },
      { token: "log.verbose", foreground: "8c959f" },
      { token: "log.debug", foreground: "6e7781" },
      { token: "log.info", foreground: "1a7f37" },
      { token: "log.warning", foreground: "9a6700" },
      { token: "log.error", foreground: "cf222e" },
      { token: "log.constant", foreground: "0550ae" },
      { token: "log.string", foreground: "0a3069" },
      { token: "log.exception", foreground: "bc4c00" },
      { token: "log.exceptiontype", foreground: "bc4c00" },
    ],
    colors: {
      "editor.background": "#ffffff",
      // 行号：比正文浅、比白底深的一档灰，看得见但不抢正文
      "editorLineNumber.foreground": "#98a1ad",
      // 行号槽必须与正文同色，否则左边会多出一条色带
      "editorGutter.background": "#ffffff",
      "scrollbarSlider.background": "#c3c9d280",
      "scrollbarSlider.hover.background": "#aab1bb80",
      "scrollbarSlider.active.background": "#8f97a380",
    },
  });
}

/** 日志控制台上色：深色 / 白底那两套自定义主题 */
export function applyLogTheme() {
  ensureMonacoThemes();
  monaco.editor.setTheme(resolvedTheme() === "Light" ? THEME_CONSOLE_LIGHT : THEME_CONSOLE_DARK);
}

/**
 * 配置文件的代码编辑器上色
 *
 * 直接用 Monaco 自带的 `vs` / `vs-dark`，与官网示例（playground）同一套配色；
 * 不要换成上面那两套控制台主题 —— 那是给日志区的，编辑代码时底色与行号槽都对不上。
 */
export function applyCodeTheme() {
  monaco.editor.setTheme(resolvedTheme() === "Light" ? "vs" : "vs-dark");
}

export { monaco };
