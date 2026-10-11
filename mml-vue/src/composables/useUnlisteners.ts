// Tauri 事件监听的安全登记
//
// 坑（实测确认）：`onMounted(async () => { const un = await onX(); onUnmounted(un); })` 是无效写法 ——
// await 之后 currentInstance 已被清空，`onUnmounted` 只会打一条 Vue 告警、什么都不注册，
// 于是窗口关掉后监听器仍留在事件总线上，回调继续往已卸载的组件里写状态。
//
// 正确做法：**卸载钩子在 setup 阶段就登记**，订阅完成后把 unlisten 交给列表：
//
// ```ts
// const { track } = useUnlisteners();
// onMounted(() => {
//   track(onCloseBlocked(() => showToast("...")));
// });
// ```
import { onUnmounted } from "vue";

/**
 * 收集 Tauri 事件订阅的退订函数，组件卸载时统一注销
 *
 * # 返回值
 *
 * - `track`: 登记一个订阅的 Promise；回调里拿到的 unlisten 会在卸载时调用
 */
export function useUnlisteners() {
  const unlisteners: Array<() => void> = [];
  let active = true;

  /** 登记一个异步订阅；Tauri 的 `listen` 返回 `Promise<UnlistenFn>` */
  function track(subscribing: Promise<() => void>) {
    subscribing
      .then((unlisten) => {
        // 订阅回来时组件已卸载：就地注销，别塞进列表
        if (active) unlisteners.push(unlisten);
        else unlisten();
      })
      .catch(() => {
        // 订阅失败（如窗口已销毁）无需处理
      });
  }

  onUnmounted(() => {
    active = false;
    for (const unlisten of unlisteners) unlisten();
    unlisteners.length = 0;
  });

  return { track };
}
