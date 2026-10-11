// 两类拖拽，共用一套"按下 → 超阈值 → 指针位置"的骨架（见文件末尾）：
//
// 1. **模组行** [`useModDrag`]：把模组拖进某个分组 / 状态分组，或拖到"未分组"里摘出来
// 2. **分组头** [`useReorderDrag`]：拖分组头调整分组的上下顺序，左侧分类栏也是这一套
//
// 都用指针事件模拟，不用 HTML5 draggable：WebView2 里会出现"禁止"光标且投放不可靠。
import { onMounted, onUnmounted, ref } from "vue";

/**
 * 开始拖拽的位移阈值（像素）
 *
 * 8 而不是 5：分组头是**整条都能拖**的大目标，点击时手抖几像素很常见 ——
 * 阈值太小会让"想点却变成拖"（用户报的问题）。8px 仍在"明显移动"的范围内，
 * 想拖的人不会觉得迟钝。
 */
const DRAG_THRESHOLD = 8;

/** 可拖拽的目标：带一个用于渲染身份的 key */
export interface DragItem {
  key: string;
}

/**
 * 通用的「按下 → 拖过阈值 → 按指针位置算落点 → 松手提交」
 *
 * 只处理骨架（阈值判定、抑制拖后点击），"落在第几项之前"由调用方的 `hitTest` 算 ——
 * 模组行是找投放容器，认两种 `data-*` 属性；分组头与分类栏是找"插到第几项之前"。
 *
 * - `deps.hitTest`: 拖动中每次指针移动调用，返回要提交的落点，`null` 表示不提交
 * - `deps.onDrop`: 松手时调用，落点非 `null` 才调。**第二个参数是被拖项的 key** ——
 *   提交前 `dragKey` 已经被 `reset()` 清掉了，调用方**不能**在回调里读 `dragKey`，
 *   读了必然是 `null`，于是"拖了却什么都没发生"。这正是分组头拖不动的根因：
 *   `useReorderDrag` 的回调读的就是 `dragKey`。
 * - `hitTest` 里也可以做副作用，例如内容区边缘自动滚动
 *
 * # 返回值
 *
 * - `dragKey`: 真正在拖的项，`null` 表示还没超过阈值
 * - `dropAt`: 指针当前落点，`null` 表示无落点
 * - `onPointerDown`: 按下某一项，开始一轮手势
 * - `consumeSuppressClick`: 取走"这次 click 要吞掉"的标记
 * - `reset`: 中止这一轮手势
 */
export function useDragGesture<T>(deps: {
  hitTest: (e: PointerEvent) => T | null;
  onDrop: (target: T, key: string) => void;
}) {
  /** 真正在拖的项，`null` 表示还没超过阈值、或者没在拖 */
  const dragKey = ref<string | null>(null);
  /** 指针当前落点，`null` 表示无落点 */
  const dropAt = ref<T | null>(null);

  let candidate: string | null = null;
  let pointerId: number | null = null;
  let startX = 0;
  let startY = 0;
  /**
   * 这一轮指针操作**是否真的拖动过**，即超过阈值
   *
   * 与 `dragKey !== null` 分开记：`dragKey` 会在 `reset()` 里清掉，而"要不要抑制
   * 紧随的 click"必须保留到 click 派发那一刻，也就是 `onPointerUp` 之后浏览器再发的那一次。
   * 原来的写法靠 `moving && target !== null` 决定抑制，**重排拖拽的 hitTest 永远有落点**，
   * 返回的是"插到第几项之前"这个数字，于是"按下—松手"这种没动过的点击也被当成
   * 拖动、把 click 吞掉 —— 表现就是分组头点不动，用户报的"点击后默认拖动、无法收起"。
   */
  let dragged = false;
  /** 拖拽结束后抑制紧随的 click，否则松手会顺手触发这一项的点击 */
  let suppressClick = false;

  function onPointerDown(e: PointerEvent, key: string) {
    if (e.button !== 0) return;
    suppressClick = false;
    dragged = false;
    pointerId = e.pointerId;
    startX = e.clientX;
    startY = e.clientY;
    candidate = key;
    dragKey.value = null;
    dropAt.value = null;
  }

  function onPointerMove(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId || !candidate) return;
    if (!dragKey.value) {
      if (Math.hypot(e.clientX - startX, e.clientY - startY) <= DRAG_THRESHOLD) return;
      dragKey.value = candidate;
      dragged = true;
    }
    dropAt.value = deps.hitTest(e);
  }

  function reset() {
    pointerId = null;
    candidate = null;
    dragKey.value = null;
    dropAt.value = null;
    // 注意：**不清 `dragged`** —— 见它上面的说明，click 还没派发
  }

  function onPointerUp(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;
    const moving = dragged;
    const target = dropAt.value;
    // **先把 key 取出来再 reset**：reset 会清掉 dragKey，之后回调里就读不到了
    const key = candidate;
    reset();
    if (!moving || key === null) return;
    // 拖过了：不论有没有落点，这一次的 click 都不该再触发"切换"（折叠 / 选中）
    suppressClick = true;
    if (target !== null) deps.onDrop(target, key);
  }

  /** 消费拖拽后的抑制点击标记；返回 true 表示本次 click 应被忽略 */
  function consumeSuppressClick(): boolean {
    if (suppressClick) {
      suppressClick = false;
      return true;
    }
    return false;
  }

  onMounted(() => {
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    // 指针被系统收走（触摸被打断等）当作取消
    window.addEventListener("pointercancel", reset);
  });
  onUnmounted(() => {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", reset);
  });

  return { dragKey, dropAt, onPointerDown, consumeSuppressClick, reset };
}

/** 状态分组的三种类型，与 ModPane 的 STATE_KEYS 一一对应 */
export type ModStateGroup = "on" | "off" | "fail";

/** 松手时的投放目标 */
export type ModDropTarget =
  /** 自建分组：归到该组 */
  | { kind: "group"; group: string }
  /** 未分组：移出自建分组，即组名是空串的那个容器 */
  | { kind: "ungrouped" }
  /** 状态分组：按状态启用 / 禁用 */
  | { kind: "state"; state: ModStateGroup };

/** 模组行拖拽的依赖 */
interface ModDragDeps {
  /** 提交：落点 + 拖着的那些模组 key，即 SHA1 */
  onDrop: (target: ModDropTarget, keys: string[]) => void;
}

/**
 * 模组行的拖拽：拖进分组 / 状态分组
 *
 * - `deps.onDrop`: 松手时提交，落点与拖着的模组键一起交出去
 *
 * # 返回值
 *
 * - `draggingKey`: 正在被拖的模组键，`null` 表示没在拖
 * - `dropTarget`: 指针下的落点，`null` 表示无落点
 * - `onRowPointerDown`: 在模组行上按下，开始一轮手势
 * - `consumeSuppressClick`: 取走"这次 click 要吞掉"的标记
 */
export function useModDrag(deps: ModDragDeps) {
  /** 指针下方是哪个投放容器 */
  function hitTest(e: PointerEvent): ModDropTarget | null {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    // 靠近内容区上下边缘自动滚动，方便投给远处的分组
    const scroller = (el?.closest?.(".item-list, .mod-table") ?? null) as HTMLElement | null;
    if (scroller) {
      const rect = scroller.getBoundingClientRect();
      if (e.clientY < rect.top + 60) scroller.scrollTop -= 10;
      else if (e.clientY > rect.bottom - 60) scroller.scrollTop += 10;
    }

    // 状态分组优先判：它是"更具体"的落点；两者不会嵌在一起，但先判少一次查找
    const state = (el?.closest?.("[data-state-group]") ?? null) as HTMLElement | null;
    if (state) {
      const value = state.dataset.stateGroup;
      return value === "on" || value === "off" || value === "fail"
        ? { kind: "state", state: value }
        : null;
    }

    const box = (el?.closest?.("[data-mod-group]") ?? null) as HTMLElement | null;
    if (!box) return null;
    const group = box.dataset.modGroup ?? "";
    return group ? { kind: "group", group } : { kind: "ungrouped" };
  }

  const gesture = useDragGesture<ModDropTarget>({
    hitTest,
    // key 由骨架在松手那一刻交过来；**不能**读 gesture.dragKey，那时已被 reset 清掉
    onDrop: (target, key) => deps.onDrop(target, [key]),
  });

  function onRowPointerDown(e: PointerEvent, key: string) {
    gesture.onPointerDown(e, key);
  }

  return {
    draggingKey: gesture.dragKey,
    dropTarget: gesture.dropAt,
    onRowPointerDown,
    consumeSuppressClick: gesture.consumeSuppressClick,
  };
}

/**
 * 竖排列表的重排拖拽，用于分组头与左侧分类栏
 *
 * 落点 = "会插到第几项之前"：调用方给出这一列可拖项的**渲染顺序 key**，
 * 以及它们的元素（用于量位置），本 hook 只做阈值与指针位置判定。
 *
 * - `deps.keys`: 当前渲染顺序，与 DOM 里那一列的顺序一致
 * - `deps.rectOf`: 量某一项的位置；返回 `null` 表示这项现在不在 DOM 里
 * - `deps.onDrop`: 松手时把 `from` 插到 `insertAt` 之前，最终顺序由调用方算并落盘
 *
 * # 返回值
 *
 * - `dragKey`: 正在被拖的项 key，`null` 表示没在拖
 * - `insertAt`: 会插到第几项之前，`null` 表示无落点
 * - `onPointerDown`: 在某一项上按下，开始一轮手势
 * - `consumeSuppressClick`: 取走"这次 click 要吞掉"的标记
 */
export function useReorderDrag(deps: {
  /** 当前渲染顺序，与 DOM 里那一列的顺序一致 */
  keys: () => string[];
  /** 量某一项的位置；返回 null 表示这项现在不在 DOM 里 */
  rectOf: (key: string) => DOMRect | null;
  /** 松手：把 `from` 插到 `insertAt` 之前，调用方自己算最终顺序并落盘 */
  onDrop: (from: string, insertAt: number) => void;
}) {
  const gesture = useDragGesture<number>({
    hitTest: (e) => {
      const keys = deps.keys();
      let index = keys.length;
      for (let i = 0; i < keys.length; i++) {
        const rect = deps.rectOf(keys[i]);
        if (rect && e.clientY < rect.top + rect.height / 2) {
          index = i;
          break;
        }
      }
      return index;
    },
    // 被拖项的 key 由骨架交过来。**以前这里读 `gesture.dragKey`**，
    // 而那时它已被 reset 清成 null —— 于是 `from` 永远是空、
    // `deps.onDrop` 根本没被调用过：拖分组头看着有插入线，松手却什么都没发生，
    // 顺序自然也永远没写进 `GroupOrder`（用户报的"模组分组无法移动顺序"）。
    onDrop: (index, key) => deps.onDrop(key, index),
  });

  return {
    dragKey: gesture.dragKey,
    insertAt: gesture.dropAt,
    onPointerDown: gesture.onPointerDown,
    consumeSuppressClick: gesture.consumeSuppressClick,
  };
}
