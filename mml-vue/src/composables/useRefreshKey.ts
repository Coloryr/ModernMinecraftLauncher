// F5 刷新入口：全局快捷键已经移除，保留空实现兼容现有调用点。

/**
 * 给当前窗口绑上 F5 刷新
 *
 * - `refresh`: 刷新回调，与窗口切回时的刷新同一份即可
 */
export function useRefreshKey(_refresh: () => void | Promise<void>) {
  // 全局快捷键已移除：F5 不再刷新页面，保留函数签名让现有调用点不用改
}
