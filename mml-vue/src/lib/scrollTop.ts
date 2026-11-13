// 滚动到列表开头 / 滚到某个锚点

import { animations } from "./appearance";

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

/** 每个滚动容器当前的滚动动画序号；新的动画一开始，旧的立刻停手 */
const scrollSeq = new WeakMap<HTMLElement, number>();

/**
 * 平滑滚到某个元素，按它所在滚动容器的内容区顶部对齐
 *
 * 每帧自己算位置，不用 `scrollIntoView({ behavior: "smooth" })`：WebView 里那条
 * 平滑滚动不一定生效（实测会直接跳过去），而且它不认本应用的"界面动画"开关。
 *
 * - `el`: 目标元素
 * - `gap`: 目标上方的留白，单位 px，对应 CSS 的 scroll-margin-top
 */
export function scrollToElementSmooth(el: HTMLElement | null, gap = 8) {
  if (!el) return;
  const scroller = scrollParent(el);
  if (!scroller) return;

  const top = scroller.scrollTop + el.getBoundingClientRect().top - scroller.getBoundingClientRect().top - gap;
  const maxTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
  const target = Math.max(0, Math.min(top, maxTop));
  const from = scroller.scrollTop;
  const delta = target - from;
  if (Math.abs(delta) < 1) return;

  const seq = (scrollSeq.get(scroller) ?? 0) + 1;
  scrollSeq.set(scroller, seq);

  if (!animations.value) {
    scroller.scrollTop = target;
    return;
  }

  // 距离越远滚得越久，但两头都收住：太短看不出在滚，太长会让人等
  const duration = Math.min(520, 220 + Math.abs(delta) * 0.5);
  const start = performance.now();
  const step = (now: number) => {
    if (scrollSeq.get(scroller) !== seq) return;
    const t = Math.min(1, (now - start) / duration);
    // easeOutCubic：先快后慢，与全局 var(--ease-out) 同一观感
    scroller.scrollTop = from + delta * (1 - (1 - t) ** 3);
    if (t < 1) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}
