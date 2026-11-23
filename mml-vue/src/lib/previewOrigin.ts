/**
 * 截图 / 图片预览的展开起点
 *
 * 就是被点的那张缩略图在视口里的位置与尺寸，预览图据此从它那儿飞到浮层中央。
 * 一律用 `getBoundingClientRect()` 的视口坐标，别自己换算滚动偏移。
 */
export type PreviewOrigin = {
  /** 左边缘，视口坐标，单位 px */
  x: number;
  /** 上边缘，视口坐标，单位 px */
  y: number;
  /** 缩略图的显示宽度，单位 px */
  width: number;
  /** 缩略图的显示高度，单位 px */
  height: number;
};

/**
 * 取元素的视口矩形，作为预览展开的起点
 *
 * - `el`: 被点击的缩略图；没有元素时返回 null，预览退回整张图淡入
 *
 * # 返回值
 *
 * 见 [`PreviewOrigin`]
 */
export function previewOriginOf(el: Element | null | undefined): PreviewOrigin | null {
  if (!el) return null;
  const rect = el.getBoundingClientRect();
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}
