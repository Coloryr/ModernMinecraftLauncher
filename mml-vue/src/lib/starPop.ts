// 收藏星标点击反馈：按钮弹一下，详情页与项目卡片共用
//
// 用 Web Animations API 而不是 CSS 类：连点两下时不会因为类名没来得及清掉而没有动画；
// 全局"界面动画"关掉时直接不播。
import { animations } from "./appearance";

/**
 * 让被点的星标按钮弹一下
 *
 * - `el`: 星标按钮；为空或环境不支持动画时什么都不做
 */
export function popStar(el: Element | null | undefined): void {
  if (!el || !animations.value || typeof el.animate !== "function") return;
  el.animate(
    [
      { transform: "scale(1) rotate(0deg)" },
      { transform: "scale(1.28) rotate(-10deg)", offset: 0.4 },
      { transform: "scale(0.92) rotate(4deg)", offset: 0.72 },
      { transform: "scale(1) rotate(0deg)" },
    ],
    { duration: 300, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
  );
}
