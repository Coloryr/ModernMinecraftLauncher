// 主窗口顶栏的手柄入口按"有没有手柄"显示
//
// 手柄插拔没有事件，只能定时问一次后端。单窗口模式切到别的页面时主页面只是被 KeepAlive
// 停用（DOM 还在、组件不销毁），窗口最小化时页面也照样在跑，这两种情况都跳过这一拍。
import { onBeforeUnmount, onMounted, ref } from "vue";
import { commands } from "./bindings";
import { usePageActive } from "./pageActive";
import { isTauri } from "../windows/windowManager";

/** 采样间隔，单位毫秒 */
const TICK = 1000;

/**
 * 手柄是否已连接
 *
 * # 返回值
 *
 * - `present`: 连着手柄时为 `true`
 */
export function useGamepadPresent() {
  const present = ref(false);
  const active = usePageActive();
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  /** 拉一次连接状态 */
  async function poll() {
    if (disposed) return;
    if (active.value && !document.hidden && isTauri()) {
      try {
        present.value = (await commands.gamepad.list()).length > 0;
      } catch {
        present.value = false;
      }
    }
    if (!disposed) timer = setTimeout(() => void poll(), TICK);
  }

  onMounted(() => {
    disposed = false;
    void poll();
  });

  onBeforeUnmount(() => {
    disposed = true;
    if (timer) clearTimeout(timer);
  });

  return { present };
}
