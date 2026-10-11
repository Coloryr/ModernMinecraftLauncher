// 复制文本到剪贴板
//
// 优先用异步 Clipboard API，Tauri 的 webview 是安全上下文，可用；
// 失败时回退到 `execCommand("copy")`，某些输入法 / 焦点状态下 Clipboard API 会被拒。

/**
 * 复制文本到剪贴板
 *
 * - `text`: 要复制的文本
 *
 * # 返回值
 *
 * 两条路径都失败时为 false
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      return ok;
    } catch {
      return false;
    }
  }
}
