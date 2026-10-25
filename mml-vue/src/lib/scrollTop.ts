// 滚动到列表开头

/**
 * 找最近的纵向滚动容器
 *
 * - `el`: 起点元素
 *
 * # 返回值
 *
 * 第一个 `overflow-y` 为 auto / scroll 的祖先；找不到返回 `null`
 */
function scrollParent(el: HTMLElement): HTMLElement | null {
  let node = el.parentElement;
  while (node) {
    const style = getComputedStyle(node);
    if (/(auto|scroll)/.test(style.overflowY)) return node;
    node = node.parentElement;
  }
  return null;
}

/**
 * 把元素滚到它所在滚动容器的**内容区**顶部
 *
 * 翻页后「回到列表开头」用。别直接 `scrollIntoView({ block: "start" })`：它对齐的是
 * 滚动容器边框内侧，会把窗口正文的上内边距（22px）一起滚出去 —— 明明已经在顶部，
 * 一翻页滚动条还是会往下挪一小格。这里按内容区顶部算差值，已经在顶部时差值为 0，不动。
 *
 * - `el`: 列表最外层的元素
 */
export function scrollToContentTop(el: HTMLElement | null) {
  if (!el) return;
  const scroller = scrollParent(el);
  if (!scroller) return;

  const padding = Number.parseFloat(getComputedStyle(scroller).paddingTop) || 0;
  const delta = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top - padding;
  // 差不到一个像素就别动了：平滑滚动一点点反而像界面在抖
  if (Math.abs(delta) < 1) return;
  scroller.scrollBy({ top: delta, behavior: "smooth" });
}
