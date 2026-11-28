// 左侧分类栏的宽度：拖栏的右边缘改宽度，拖过的值记在本地（丢了也只是回到默认宽度）
//
// 资源管理 / 资源收藏 / 方块列表三个窗口共用一份宽度 —— 这三条栏是同一个东西，
// 分开存的话换个窗口就要再调一次。
import { computed, onBeforeUnmount, ref, type CSSProperties } from "vue";
import { KEYS, readNumber, writeNumber } from "../lib/storage";

/** 默认宽度，单位 px：与 themes.css 的 --rail-w 一致 */
const DEFAULT_WIDTH = 168;
/** 下限：分类名 + 条目数是这一栏的全部内容，再窄就只剩省略号了（默认 168） */
const MIN_WIDTH = 160;
/** 上限：左边栏不该吃掉半个窗口 */
const MAX_WIDTH = 360;

/**
 * 可拖动改宽度的左侧分类栏
 *
 * # 返回值
 *
 * - `railStyle`: 挂到栏上的样式，覆盖 `--rail-w`
 * - `onResizeStart`: 右边缘拖动把手的 `pointerdown` 处理器
 */
export function useRailWidth() {
  const stored = readNumber(KEYS.railWidth, DEFAULT_WIDTH);
  const width = ref(clamp(Number.isFinite(stored) ? stored : DEFAULT_WIDTH));

  /** 拖动起点：指针 x 与当时的宽度 */
  let startX = 0;
  let startWidth = 0;
  /** 正在拖的那次指针 id */
  let pointerId: number | null = null;

  function clamp(value: number): number {
    return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(value)));
  }

  function onMove(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;
    width.value = clamp(startWidth + (e.clientX - startX));
  }

  function onUp(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;
    pointerId = null;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    writeNumber(KEYS.railWidth, width.value);
  }

  /** 右边缘把手按下：记下起点，之后跟指针走 */
  function onResizeStart(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    pointerId = e.pointerId;
    startX = e.clientX;
    startWidth = width.value;
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  onBeforeUnmount(() => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
  });

  const railStyle = computed<CSSProperties>(() => ({ "--rail-w": `${width.value}px` }) as CSSProperties);

  return { width, railStyle, onResizeStart };
}
