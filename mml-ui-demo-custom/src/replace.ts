// 窗口页面回调：只替换主窗口，其它窗口返回 null 继续用内置页面
//
// ReplaceWindow 里面直接渲染内置 MainWindow，再额外加一个右下角按钮；
// 所以这里是“复用原始窗口 + 追加内容”，而不是重写整页。

import type { WindowPageRegistry } from "mml-ipc";
import ReplaceWindow from "./ReplaceWindow.vue";

/**
 * 按窗口 kind 返回要替换的页面
 *
 * - `kind`: 窗口标识
 */
export function replace(kind: string) {
  return kind === "main" ? ReplaceWindow : null;
}

(window as unknown as { __mmlWindowPages: WindowPageRegistry }).__mmlWindowPages.push(replace);
