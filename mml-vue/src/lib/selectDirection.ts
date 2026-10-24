// 下拉方向：靠近窗口底部的 <select> 打开时改成向上，避免弹层被窗口边缘截掉
//
// `appearance: base-select` 的弹层方向由 CSS `position-area` 决定；这里在点击 / 键盘打开前
// 按可用空间给 select 打一个 `.select-up`，方向样式在 styles/controls.css。

/** 向上展开的标记类 */
const UP_CLASS = "select-up";

/** 单个选项的估算高度，单位 px：与 controls.css 里 option 的 8px + 8px 内边距、13px 字号一致 */
const ITEM_HEIGHT = 34;

/**
 * 估算弹层高度
 *
 * - `select`: 目标下拉
 *
 * # 返回值
 *
 * 选项数 × 单行高度 + 弹层内边距，最多取视口高度的 60%
 */
function estimateHeight(select: HTMLSelectElement): number {
  return Math.min(select.options.length * ITEM_HEIGHT + 14, window.innerHeight * 0.6);
}

/**
 * 按当前位置决定这次向上还是向下
 *
 * - `event`: 打开下拉的 pointerdown / keydown 事件
 */
function markDirection(event: Event) {
  const target = event.target as HTMLElement | null;
  const select = target?.closest?.("select.field-select") as HTMLSelectElement | null;
  if (!select) return;
  const rect = select.getBoundingClientRect();
  const below = window.innerHeight - rect.bottom;
  // 下方放不下、上方更宽裕才翻上去；两边都放不下时保持向下，让弹层自己滚
  select.classList.toggle(UP_CLASS, below < estimateHeight(select) && rect.top > below);
}

/** 给全应用的下拉挂上方向判断；只挂一次，交给事件捕获处理所有窗口与弹窗 */
export function initSelectDirection() {
  document.addEventListener("pointerdown", markDirection, true);
  document.addEventListener(
    "keydown",
    (event) => {
      if (event.key === "Enter" || event.key === " " || event.key === "ArrowDown") {
        markDirection(event);
      }
    },
    true,
  );
}
