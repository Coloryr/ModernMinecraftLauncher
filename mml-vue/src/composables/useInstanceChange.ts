// 实例在别处被增删改时，各窗口的实例列表要跟着变
//
// 「删除实例」发生在主窗口，受影响的却是所有正显示实例下拉的窗口：那份列表是打开时拉的
// 快照，不清掉的话下拉里一直留着已经删掉的实例，选它只会报"实例不存在"。
// 后端每次增删改都发 instance-change，这里统一订阅，窗口各自重拉列表并回退选中项。
//
// 用法（窗口根组件里一行，与 useWindowRefresh 并列）：
// ```ts
// useInstanceChange(syncInstances);
// ```
import { onMounted } from "vue";
import { onInstanceChange } from "../lib/api";
import { useUnlisteners } from "./useUnlisteners";

/**
 * 订阅实例变更事件
 *
 * 只负责订阅 / 退订的时机，重拉什么由调用方决定：各窗口的列表结构不同，
 * 有的还要顺带回退当前选中项。
 *
 * - `refresh`: 实例被添加 / 删除 / 改名 / 换分组 / 换图标时调用，参数是变更类型
 */
export function useInstanceChange(refresh: (type: string) => void | Promise<void>) {
  const { track } = useUnlisteners();

  // 订阅是异步的，卸载早于订阅完成时就地退订，别把监听留到窗口关掉之后
  onMounted(() => {
    track(onInstanceChange((type) => void refresh(type)));
  });
}
