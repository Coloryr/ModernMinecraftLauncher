// 拖动时"跟手浮起副本"的公共状态
//
// 各拖动交互（实例 / 收藏 / 分组 / 分类栏）的落点语义不同，但"抓住的那一项跟着指针浮起来"
// 是同一件事：位置与显隐统一在这里管；外观与内容交给 components/ui/DragGhost.vue。
import { ref } from "vue";

/**
 * 拖动遮罩下的最上层元素
 *
 * 拖动时 DragGhost 会铺一层 `.drag-shield` 挡住底下的界面（不然按钮会跟着悬停高亮、
 * 松手还可能被点到）。命中判定因此不能用 elementFromPoint —— 它只会返回遮罩自己，
 * 得用 elementsFromPoint 自上而下找到遮罩之下的那个。
 *
 * - `x` / `y`: 视口坐标
 *
 * # 返回值
 *
 * 遮罩之下的最上层元素；指针不在窗口里时是 `null`
 */
export function elementBeneathShield(x: number, y: number): Element | null {
  return (
    document.elementsFromPoint(x, y).find((el) => !el.classList.contains("drag-shield")) ?? null
  );
}

/**
 * 跟手浮起副本
 *
 * # 返回值
 *
 * - `ghost`: 副本状态（被抓住的项 + 指针位置），`null` 表示没在拖
 * - `startGhost` / `moveGhost` / `stopGhost`: 开始 / 跟随 / 结束
 */
export function useDragGhost<T>() {
  const ghost = ref<{ item: T; x: number; y: number } | null>(null);

  /**
   * 开始拖：记下被抓住的项与指针位置
   *
   * - `item`: 被抓住的那一项，调用方自定义类型
   * - `e`: 触发的那次指针事件
   */
  function startGhost(item: T, e: PointerEvent) {
    ghost.value = { item, x: e.clientX, y: e.clientY };
  }

  /** 跟随指针；没在拖时什么都不做 */
  function moveGhost(e: PointerEvent) {
    const cur = ghost.value;
    if (cur) ghost.value = { item: cur.item, x: e.clientX, y: e.clientY };
  }

  /** 结束拖拽 */
  function stopGhost() {
    ghost.value = null;
  }

  return { ghost, startGhost, moveGhost, stopGhost };
}
