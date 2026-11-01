// F5：重新拉这一页的数据
//
// F5 在发布版里被 WebView2 的"浏览器快捷键"关掉了，debug 下则会被拿去重载页面 ——
// 两种情况都指望不上，所以自己接：拦下默认行为，再跑这一页的刷新回调。
//
// **只对当前活动的页面生效**：单窗口模式里窗口组件被 KeepAlive 缓存，
// 同时挂着的可能有五六个，不能按一次 F5 让所有窗口都去拉一遍。
import { onMounted, onUnmounted } from "vue";
import { usePageActive } from "../lib/pageActive";

/**
 * 给当前窗口绑上 F5 刷新
 *
 * - `refresh`: 刷新回调，与窗口切回时的刷新同一份即可
 */
export function useRefreshKey(refresh: () => void | Promise<void>) {
  /** 本页是否正在显示：KeepAlive 缓存里非活动的窗口不响应 */
  const active = usePageActive();

  /** F5 按下：拦住浏览器的重载，跑这一页的刷新 */
  function onKeyDown(e: KeyboardEvent) {
    if (e.key !== "F5" || e.ctrlKey || e.altKey || e.metaKey) return;
    e.preventDefault();
    if (!active.value) return;
    void refresh();
  }

  onMounted(() => window.addEventListener("keydown", onKeyDown));
  onUnmounted(() => window.removeEventListener("keydown", onKeyDown));
}
