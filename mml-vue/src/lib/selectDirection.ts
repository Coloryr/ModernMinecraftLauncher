// 下拉方向：靠近可视区底部的 <select> 打开时改成向上，避免弹层被边缘截掉
//
// `appearance: base-select` 的弹层方向由 CSS `position-area` 决定；这里在点击 / 键盘打开前
// 按可用空间给 select 打一个 `.select-up`，方向样式在 styles/controls.css。
//
// 判据是"下方放不放得下"，不是估算得多准：弹层高度由浏览器按选项算，估小了会被边缘截掉
// （两行版本估 82px、实际 95px 就卡在边缘上），所以底下再压一条 DROP_MIN 兜底 ——
// 贴着底边的下拉即便只有两三项，也宁可翻到上面去。
//
// 边界取"最近一层滚动容器与视口的交集"，不是直接拿 window.innerHeight：控件常被包在
// 详情页、弹窗正文这类自己滚的容器里，能露出来的区域到那层容器下沿就结束了，
// 按窗口高度算会多算出一截，弹层照旧贴着容器底边被截。
//
// 翻上去还要**封顶**：`position-try-order: normal` 的规矩是"首选位置放不下就回退到候选位置"，
// 弹层比上方空间高时浏览器会认为上面也放不下、又翻回下面（日志里 need=324 > above=308 就是这样）。
// 所以翻上去时把上方可用高度写成 `--picker-max-h`，让弹层按这个高度截断、内部滚动。

/** 向上展开的标记类 */
const UP_CLASS = "select-up";

/** 翻上去时给弹层封顶的变量，样式在 controls.css */
const PICKER_MAX_VAR = "--picker-max-h";

/** 封顶高度下限，单位 px：再挤也至少露出两三项 */
const PICKER_MIN = 120;

/** 弹层与可视边界之间留的空隙，单位 px */
const EDGE = 8;

/** 会被当作可视边界的 overflow 取值 */
const CLIP_OVERFLOW = /^(auto|scroll|hidden|overlay)$/;

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
 * 这个下拉能露出来的上下边界
 *
 * 从最近一层会裁剪内容的祖先算起，再与视口取交集；外层容器只会更宽，不用继续往上找。
 *
 * - `select`: 目标下拉
 *
 * # 返回值
 *
 * - `top` / `bottom`: 可视边界的纵坐标，视口坐标
 */
function visibleBounds(select: HTMLSelectElement): { top: number; bottom: number } {
  let top = 0;
  let bottom = window.innerHeight;
  for (let el = select.parentElement; el; el = el.parentElement) {
    const overflowY = getComputedStyle(el).overflowY;
    if (!CLIP_OVERFLOW.test(overflowY)) continue;
    const rect = el.getBoundingClientRect();
    top = Math.max(top, rect.top);
    bottom = Math.min(bottom, rect.bottom);
    break;
  }
  // 容器被折叠成 0 高之类的情况：退回视口，至少别算出负数空间
  return bottom > top ? { top, bottom } : { top: 0, bottom: window.innerHeight };
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
  const bounds = visibleBounds(select);
  const below = bounds.bottom - rect.bottom;
  const above = rect.top - bounds.top;
  // 选项多时按估算高度判（长列表往下必然被截），选项少时由 DROP_MIN 保底
  const need = Math.max(estimateHeight(select), DROP_MIN);
  // 两个条件任一成立就翻上去：下方放不下整个弹层，或控件已经在可视区下半屏。
  // 后者即便下方"刚好"放得下，弹层也会贴着边界（两三个版本的筛选正好卡这一档），
  // 不如统一往上弹；只有上方比下方窄时才留在下面，那两种落点都没余量，让弹层自己滚。
  const up = above > below && (below < need || rect.top - bounds.top > (bounds.bottom - bounds.top) / 2);
  select.classList.toggle(UP_CLASS, up);
  // 封顶：不封的话弹层比上方还高，浏览器会判定"上面也放不下"，直接回退成向下
  if (up) {
    select.style.setProperty(PICKER_MAX_VAR, Math.round(Math.max(PICKER_MIN, above - EDGE)) + "px");
  } else {
    select.style.removeProperty(PICKER_MAX_VAR);
  }
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
