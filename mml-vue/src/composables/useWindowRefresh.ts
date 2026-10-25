// 单窗口模式：回到本窗口时重拉一次数据；同一份回调也接在 F5 上（见 useRefreshKey）
//
// 单窗口模式下 App.vue 用 <KeepAlive> 缓存窗口组件（见 App.vue），切走只是 deactivate、
// 切回**不会重新挂载** —— 于是 onMounted 里拉的那份数据永远是第一次打开时的快照，
// 统计 / 下载 / 资源 / 收藏 / 账户这类窗口尤其明显。
//
// 首次挂载不触发：onActivated 在挂载时也会跑一次，那一次交给 onMounted，免得拉两遍。
//
// 用法（窗口根组件里一行）：
// ```ts
// useWindowRefresh(load);
// ```
import { onActivated } from "vue";
import { useRefreshKey } from "./useRefreshKey";

/**
 * 单窗口模式下，窗口每次被切回时重拉一次数据
 *
 * - `refresh`: 重拉回调；首次挂载不调用，那一次交给 `onMounted`
 */
export function useWindowRefresh(refresh: () => void | Promise<void>) {
  let mounted = false;
  onActivated(() => {
    if (!mounted) {
      mounted = true;
      return;
    }
    void refresh();
  });
  // F5 也走同一个刷新入口：切回窗口与手动刷新拉的是同一份数据
  useRefreshKey(refresh);
}
