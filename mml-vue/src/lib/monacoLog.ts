// Monaco 日志视图环境：worker 配置 + 随主题切换的日志控制台主题
//
// 只加载 editor.api，不带各语言贡献包，日志是纯文本；worker 只需要
// 基础 editor worker。vite 的 ?worker 导入在 dev / build 下都能正确打包。

import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import * as monaco from "monaco-editor/editor/editor.api";
import { resolvedTheme } from "./theme";

declare global {
  interface Window {
    MonacoEnvironment?: monaco.Environment;
  }
}

self.MonacoEnvironment = {
  getWorker() {
    return new EditorWorker();
  },
};

/** 暗色日志控制台主题名 */
const THEME_DARK = "mml-log-dark";
/** 亮色日志控制台主题名 */
const THEME_LIGHT = "mml-log-light";

let themeDefined = false;

/** 注册两套日志控制台主题，分别在 `editor.create` 之前调一次 */
export function ensureLogTheme() {
  if (themeDefined) return;
  themeDefined = true;

  // 暗色：比卡片底色再深一档的控制台观感
  monaco.editor.defineTheme(THEME_DARK, {
    base: "vs-dark",
    inherit: true,
    rules: [],
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
  monaco.editor.defineTheme(THEME_LIGHT, {
    base: "vs",
    inherit: true,
    rules: [],
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

/** 按当前主题给日志控制台上色 */
export function applyLogTheme() {
  ensureLogTheme();
  monaco.editor.setTheme(resolvedTheme() === "Light" ? THEME_LIGHT : THEME_DARK);
}

export { monaco };
