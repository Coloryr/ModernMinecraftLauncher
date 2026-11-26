// Monaco 界面文案（查找框 / 补全提示等）的中文表
//
// Monaco 的 `localize(index, "English", …)` 会先查 `globalThis._VSCODE_NLS_MESSAGES[index]`，
// 查不到才回落到英文原文（见 monaco-editor/esm/vs/nls.js 的 lookupMessage）。npm 包**不带
// 任何语言包**，所以默认恒为英文；这里就用它自己的机制，在开窗前把这张表挂到那个全局上。
// lookup 是每次现查的，所以语言一变重新挂一次即时生效，不需要重启窗口。
//
// 下标会随 Monaco 版本变，**不要手写**：表由 scripts/gen-monaco-nls.ts 按"英文原文"对上号
// 生成（src/lib/monacoNls.zh.ts），升级 monaco-editor 后重跑一次即可：
// `cd mml-vue && npm run gen:nls`。生成器还会报出"字典里有、源码里已找不到"
// 的条目，改名 / 删掉的文案不会一直留着。
import { ZH_MESSAGES } from "./monacoNls.zh";

declare global {
  /** Monaco 的文案表：nls.js 的 lookupMessage 按数字下标来这里查 */
  var _VSCODE_NLS_MESSAGES: Record<number, string> | undefined;
  /** 语言标记，Monaco 只用它判断 pseudo 调试模式 */
  var _VSCODE_NLS_LANGUAGE: string | undefined;
}

/**
 * 按当前语言挂上 Monaco 的文案表
 *
 * - `lang`: 界面语言；只有中文界面才挂表，英文界面挂空表走 Monaco 自己的英文原文
 *
 * 挂空表而不是删掉：lookup 每次都读这个全局，换一次即时生效，不用重开窗口
 */
export function applyMonacoNls(lang: "zh_cn" | "en_us") {
  globalThis._VSCODE_NLS_MESSAGES = lang === "zh_cn" ? ZH_MESSAGES : {};
  globalThis._VSCODE_NLS_LANGUAGE = lang === "zh_cn" ? "zh-cn" : "en";
}
