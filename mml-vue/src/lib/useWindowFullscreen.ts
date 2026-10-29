// 全屏状态：macOS 全屏后窗口铺满整屏，顶栏会顶到刘海 / 菜单栏下方，
// 得给顶部留一条安全区，否则内容被挡住一截。
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "./util";
import { isWindowFullscreen } from "./titlebar";

/**
 * 当前窗口是否全屏
 *
 * Tauri 没有单独的全屏事件，靠 resize 兜底再查一次。
 *
 * # 返回值
 *
 * 全屏状态，`ref<boolean>`
 */
export function useWindowFullscreen() {
  const fullscreen = ref(false);
  let unlisten: (() => void) | null = null;
  let disposed = false;

  const refresh = async () => {
    const now = await isWindowFullscreen();
    if (!disposed) fullscreen.value = now;
  };

  onMounted(async () => {
    if (!isTauri()) return;
    await refresh();
    if (disposed) return;
    const stop = await getCurrentWindow().onResized(() => void refresh());
    // 等订阅期间组件可能已经卸载，这时监听要立刻退掉，否则会留到窗口销毁
    if (disposed) {
      stop();
      return;
    }
    unlisten = stop;
  });

  onUnmounted(() => {
    disposed = true;
    unlisten?.();
    unlisten = null;
  });

  return fullscreen;
}
