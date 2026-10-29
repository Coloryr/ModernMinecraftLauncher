// 窗口装饰生命周期：先注册贴靠热区，再激活插件装饰
//
// 顺序不能反：插件在激活流程里就要量热区矩形，量不到会以 `disabled` 完成这次激活，
// 之后再补报也改不回，表现为原生 frame 没摘、系统标题栏与自绘按钮并存。
// 窗口按 `visible(false)` 建出来，激活同时负责显示，所以前端挂载后才显示。
// 主窗口多一层启动画面：标题栏要等核心 load 完成才渲染，由 `waitForSnapTarget` 兜住。
import { nextTick, onMounted, onUnmounted, ref, type Ref } from "vue";
import { commands } from "./bindings";
import { isTauri } from "./util";
import { detectOs } from "./titlebar";
import { pluginDecorated, registerSnapTarget, waitForSnapTarget } from "./decoration";

/**
 * 给当前窗口接上插件装饰
 *
 * - `rootEl`: 该窗口的根元素，贴靠热区在它范围内查找最大化按钮
 *
 * # 返回值
 *
 * - `decorated`: 装饰是否已由插件接管
 */
export function useWindowDecoration(rootEl: Ref<HTMLElement | null>) {
  const decorated = ref(false);
  let unregister: (() => void) | null = null;
  let disposed = false;

  onMounted(async () => {
    if (!isTauri()) return;

    const root = rootEl.value;
    unregister = registerSnapTarget(root);

    // 等一帧让标题栏（含窗口按钮）渲染出来，否则量不到矩形
    await nextTick();
    // 窗口关得快时不必再等按钮：激活已经没意义，白等 15 秒
    if (disposed) return;
    // 只有 Windows 需要等最大化按钮：贴靠热区按它量。macOS / Linux 上插件摆的是
    // 原生按钮，这里量不到，白等 15 秒还会让窗口迟迟不显示
    if (detectOs() === "windows") {
      await waitForSnapTarget(root);
    }
    if (disposed) return;

    try {
      const mode = await commands.windows.activateDecoration();
      // 只有插件真正接管了才置 true；返回 native 说明激活没成，标题栏的类也不会挂上
      const custom = mode === "custom";
      pluginDecorated.value = custom;
      decorated.value = custom;
      if (!custom) {
        console.warn("[decoration] 退回原生 frame:", mode);
      }
    } catch (e) {
      pluginDecorated.value = false;
      console.warn("[decoration] 激活失败:", e);
    }
  });

  onUnmounted(() => {
    disposed = true;
    unregister?.();
    unregister = null;
  });

  return { decorated };
}
