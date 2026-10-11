// Monaco 日志视图环境：worker 配置 + 日志专用暗色主题
//
// 只加载 editor.api，不带各语言贡献包，日志是纯文本；worker 只需要
// 基础 editor worker。vite 的 ?worker 导入在 dev / build 下都能正确打包。

import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import * as monaco from "monaco-editor/editor/editor.api";

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

let themeDefined = false;

/** 日志控制台主题，与 InstanceLogPanel 的控制台同底色，始终暗色 */
export function ensureLogTheme() {
  if (themeDefined) return;
  themeDefined = true;
  monaco.editor.defineTheme("mml-log", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#0d0f12",
      "editorLineNumber.foreground": "#3a4048",
      "editorGutter.background": "#0d0f12",
      "scrollbarSlider.background": "#2a2f3880",
      "scrollbarSlider.hover.background": "#3a404880",
      "scrollbarSlider.active.background": "#4a515c80",
    },
  });
}

export { monaco };
