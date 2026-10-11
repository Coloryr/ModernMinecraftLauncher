// 极小的通用工具：**不依赖任何其它业务模块**
//
// 单独一个文件是为了打破循环引用：`lib/storage.ts` 需要 `isTauri()`，
// 而 `windows/windowManager.ts` 又要用 `lib/storage.ts` 读写窗口模式 ——
// 两边互相 import 会形成一个环，靠打包器的求值顺序兜底。能跑，但脆弱：
// 谁先被求值就决定了另一边拿到的是不是 undefined。
// 把 `isTauri` 这种"零依赖判定"放这里，两边都只依赖它。

/**
 * 是否运行在 Tauri 环境
 *
 * # 返回值
 *
 * 浏览器预览时为 false
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
