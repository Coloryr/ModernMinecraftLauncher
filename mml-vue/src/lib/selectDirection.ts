// 下拉方向：靠近窗口底部的 <select> 打开时改成向上，避免弹层被窗口边缘截掉
//
// `appearance: base-select` 的弹层方向由 CSS `position-area` 决定；这里在点击 / 键盘打开前
// 按可用空间给 select 打一个 `.select-up`，方向样式在 styles/controls.css。
//
// 判据是"下方放不放得下"，不是估算得多准：弹层高度由浏览器按选项算，估小了会被窗口边缘
// 截掉（两行版本估 82px、实际 95px 就卡在边缘上），所以底下再压一条 DROP_MIN 兜底 ——
// 贴着窗口底边的下拉即便只有两三项，也宁可翻到上面去。

/** 向上展开的标记类 */
const UP_CLASS = "select-up";

/** 单个选项的行高估算，单位 px：option 的上下内边距各 8px 加一行文字 */
const ITEM_HEIGHT = 38;

/** 弹层自身的内边距与描边，单位 px */
const POPUP_PAD = 16;

/** 下方至少要有的可用高度，单位 px；比这还窄就翻上去 */
const DROP_MIN = 180;

/**
 * 估算弹层高度
 *
 * - `select`: 目标下拉
 *
 * # 返回值
 *
 * 选项数 × 单行高度 + 弹层内边距，最多取视口高度的 60%（长列表不会顶出整屏）
 */
function estimateHeight(select: HTMLSelectElement): number {
  return Math.min(select.options.length * ITEM_HEIGHT + POPUP_PAD, window.innerHeight * 0.6);
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
  const above = rect.top;
  // 选项多时按估算高度判（长列表往下必然被截），选项少时由 DROP_MIN 保底
  const need = Math.max(estimateHeight(select), DROP_MIN);
  // 两个条件任一成立就翻上去：下方放不下整个弹层，或控件已经在窗口下半屏。
  // 后者即便下方"刚好"放得下，弹层也会贴着窗口底边（两三个版本的筛选正好卡这一档），
  // 不如统一往上弹；只有上方比下方窄时才留在下面，那两种落点都没余量，让弹层自己滚。
  const up = above > below && (below < need || rect.top * 2 > window.innerHeight);
  select.classList.toggle(UP_CLASS, up);
}

/** 给全应用的下拉挂上方向判断；只挂一次，交给事件捕获处理所有窗口与弹窗 */
export function initSelectDirection(): void {
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
