// 行虚拟化的**窗口计算**：按行高算出"该渲染哪几行、上下各留多少空白"。
//
// 只做算术、不碰 DOM 也不引 vue，方便单测（见 tests/useVirtualRows.test.mjs）；
// 取元素位置与滚动状态的那一半在 useVirtualRows.ts。
//
// 两种口径：
// - **定高**（[visibleWindow]）：被藏起来的行按同一个行高估着撑高，行高由 CSS 钉死，
//   常量与 resource.css 里那几条同源，改一处要改两处；
// - **变高**（[visibleWindowOfHeights]）：每行高度按行种给，分组列表那种"标题 / 实例行 /
//   平铺卡片各一档"的列表用它，同样要求高度钉死在 CSS 里。

/** 资源列表（模组列表 / 存档 / 资源包 …）一行的高度，单位 px；与 `.vlist-rows .item-row` 的 height 同源 */
export const LIST_ROW_H = 56;
/** 模组列表的行间距，单位 px；与 `.mod-rows` 的 gap 同源 */
export const LIST_ROW_GAP = 6;
/** 其它资源分类列表（存档 / 资源包 / 光影 …）的行间距，单位 px；与组件内联给的 gap 同源 */
export const RESOURCE_ROW_GAP = 8;
/** 模组**表格**视图一行的高度，单位 px；与 `.mod-table .mod-virtual-row` 的 height 同源 */
export const TABLE_ROW_H = 32;

/** 视口上下各多渲染多少，单位 px */
export const OVERSCAN = 400;

/** 一次虚拟窗口 */
export interface VirtualWindow {
  /** 要渲染的第一行下标，含 */
  start: number;
  /** 要渲染的最后一行下标，不含 */
  end: number;
  /** 上面藏起来的行占的高度，单位 px */
  padTop: number;
  /** 下面藏起来的行占的高度，单位 px */
  padBottom: number;
}

/**
 * 算当前该渲染哪几行
 *
 * 行容器在滚动容器内容坐标里的顶边是 `rootTop`，第 i 行的顶边就是 `rootTop + i * (rowH + gap)`；
 * 可见区间就是"视口 ± overscan"与这个序列的交集。上下留白**精确**补回隐藏行的高度：
 * 行间距只出现在渲染出来的行之间，一行都没渲染时末尾那行的间距也不存在。
 *
 * 截图墙那种一排多个的网格也能用它：把"行"当成"第几排"，`count` 给排数即可。
 *
 * - `scrollTop`: 滚动容器的滚动位置，单位 px
 * - `viewHeight`: 滚动容器的高度，单位 px
 * - `rootTop`: 行容器顶边在容器内容坐标里的位置，单位 px
 * - `count`: 行数（网格时是排数）
 * - `rowH`: 行高，单位 px
 * - `gap`: 行间距，单位 px；默认 0
 * - `overscan`: 上下各多渲染多少，单位 px
 *
 * # 返回值
 *
 * 见 [`VirtualWindow`]
 */
export function visibleWindow(options: {
  scrollTop: number;
  viewHeight: number;
  rootTop: number;
  count: number;
  rowH: number;
  gap?: number;
  overscan?: number;
}): VirtualWindow {
  const { scrollTop, viewHeight, rootTop, count, rowH } = options;
  const gap = options.gap ?? 0;
  const overscan = options.overscan ?? OVERSCAN;
  const slot = rowH + gap;
  if (count <= 0 || slot <= 0) {
    return { start: 0, end: 0, padTop: 0, padBottom: 0 };
  }

  const first = Math.floor((scrollTop - overscan - rootTop) / slot);
  const last = Math.ceil((scrollTop + viewHeight + overscan - rootTop) / slot);
  const start = Math.min(Math.max(first, 0), count);
  const end = Math.min(Math.max(last, start), count);

  let padTop = start * slot;
  let padBottom = (count - end) * slot;
  // 一行都没渲染时，整列里就没有行间距了：末尾那行的间距要从留白里扣掉。
  // 优先扣下面那侧；组在视口上方时下面本来就是 0，改扣上面那侧
  if (end <= start) {
    if (padBottom >= gap) {
      padBottom -= gap;
    } else {
      padTop -= gap;
    }
  }
  return { start, end, padTop: Math.max(padTop, 0), padBottom: Math.max(padBottom, 0) };
}

/**
 * 算当前该渲染哪几行，**每行高度可以不同**
 *
 * 分组列表那种"分组标题 / 添加行 / 实例行 / 平铺卡片各一档高"的列表用这个：先把各行的
 * 顶边累加成前缀和，再二分找出可见区间。留白按"藏起来的那些行的高度之和"精确补回，
 * 所以滚动条长度与滚动位置都和全渲染时一致。
 *
 * 行高由调用方按行种钉死（CSS 里也是固定 height），不要靠渲染出来再量：量出来再改，
 * 滚动条会在滚动过程中一跳一跳。
 *
 * - `scrollTop`: 滚动容器的滚动位置，单位 px
 * - `viewHeight`: 滚动容器的高度，单位 px
 * - `rootTop`: 行容器顶边在容器内容坐标里的位置，单位 px
 * - `heights`: 每行的高度，单位 px，顺序与行一致
 * - `gap`: 行间距，单位 px；默认 0
 * - `overscan`: 上下各多渲染多少，单位 px
 *
 * # 返回值
 *
 * 见 [`VirtualWindow`]
 */
export function visibleWindowOfHeights(options: {
  scrollTop: number;
  viewHeight: number;
  rootTop: number;
  heights: readonly number[];
  gap?: number;
  overscan?: number;
}): VirtualWindow {
  const { scrollTop, viewHeight, rootTop, heights } = options;
  const gap = options.gap ?? 0;
  const overscan = options.overscan ?? OVERSCAN;
  const count = heights.length;
  if (count <= 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };

  // 前缀和：offset[i] 是第 i 行的顶边，offset[count] 是整列高度
  const offset = new Array<number>(count + 1);
  offset[0] = 0;
  for (let i = 0; i < count; i += 1) offset[i + 1] = offset[i] + heights[i] + gap;

  const top = scrollTop - overscan - rootTop;
  const bottom = scrollTop + viewHeight + overscan - rootTop;

  // 第一个下边缘越过 top 的行
  let lo = 0;
  let hi = count;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (offset[mid] + heights[mid] > top) hi = mid;
    else lo = mid + 1;
  }
  const start = Math.min(lo, count);

  // 第一个顶边越过 bottom 的行，它的前一个就是最后要渲染的那行
  lo = start;
  hi = count;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (offset[mid] < bottom) lo = mid + 1;
    else hi = mid;
  }
  const end = Math.max(start, Math.min(lo, count));

  let padTop = start > 0 ? offset[start] : 0;
  let padBottom = end < count ? offset[count] - offset[end] : 0;
  // 与定高版同口径：一行都没渲染时整列里没有行间距，末尾那行的间距要从留白里扣掉
  if (end <= start && count > 0 && gap > 0) {
    if (padBottom >= gap) padBottom -= gap;
    else padTop = Math.max(0, padTop - gap);
  }
  return { start, end, padTop, padBottom };
}
