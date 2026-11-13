// 右侧换页的"分层错峰"入场：标题先出，下面的分组 / 卡片按顺序依次落下
//
// 用 Web Animations API 而不是 CSS 类：子元素数量不固定，延迟要按实际数量摊开；
// 全局"界面动画"关掉时直接完成，不阻塞切换。
//
// 没有行可错峰时（空列表、加载骨架、数据还在路上）整块淡入：子页切换常常先落到
// 一个还没有行的空页上，只淡标题的话看着就是硬切。
import { animations } from "../lib/appearance";

/** 完成回调，Vue Transition 用 */
type Done = () => void;

/**
 * 生成一组分层错峰的进入 / 离开钩子
 *
 * - `selector`: 要错峰的元素，按文档顺序播放
 * - `options.duration`: 单个元素的动画时长，单位毫秒
 * - `options.step`: 相邻元素的基础间隔，单位毫秒；数量多时会自动压缩
 * - `options.offset`: 入场位移，单位像素
 * - `options.max`: 最多错峰的个数，超出的元素直接出现，避免长列表创建上千个动画
 * - `options.fade`: 只做渐变的元素选择器，通常是页面标题；不参与位移和错峰
 *
 * # 返回值
 *
 * - `enter`: 给 `<Transition @enter>` 用；有行就逐行落下，没有行就整块淡入
 * - `leave`: 旧页立即移除，不做退场
 */
export function useStaggerEnter(
  selector: string,
  options: {
    duration?: number;
    step?: number;
    offset?: number;
    max?: number;
    fade?: string;
  } = {},
) {
  const duration = options.duration ?? 320;
  const step = options.step ?? 60;
  const offset = options.offset ?? 18;
  const max = options.max ?? 24;
  const fadeSelector = options.fade ?? "";

  function enter(el: Element, done: Done) {
    if (!animations.value) {
      done();
      return;
    }
    // 没有 Web Animations 的环境直接完成，别把切换卡住
    if (typeof el.animate !== "function") {
      done();
      return;
    }
    const items = Array.from(el.querySelectorAll(selector)).slice(0, max);
    if (!items.length) {
      // 整块入场：位移比逐行小一档，整页一起动时别晃得太夸张
      const animation = el.animate(
        [
          { opacity: 0, transform: "translateY(8px)" },
          { opacity: 1, transform: "translateY(0)" },
        ],
        { duration: 240, easing: "cubic-bezier(0.22, 1, 0.36, 1)", fill: "both" },
      );
      animation.finished
        .catch(() => undefined)
        .finally(() => {
          animation.cancel();
          done();
        });
      return;
    }
    const fadeItems = fadeSelector ? Array.from(el.querySelectorAll(fadeSelector)) : [];
    let left = items.length + fadeItems.length;
    const finish = () => {
      left -= 1;
      if (left === 0) done();
    };

    // 标题只做透明度渐变，位置固定，不跟着下面的分组上下移动
    fadeItems.forEach((item) => {
      const animation = item.animate([{ opacity: 0 }, { opacity: 1 }], {
        duration: 240,
        easing: "cubic-bezier(0.22, 1, 0.36, 1)",
        fill: "both",
      });
      animation.finished
        .catch(() => undefined)
        .finally(() => {
          animation.cancel();
          finish();
        });
    });

    // 元素多时压缩间隔，整套入场不超过 320 + 420ms
    const gap = items.length > 1 ? Math.min(step, 420 / (items.length - 1)) : 0;
    items.forEach((item, index) => {
      const animation = item.animate(
        [
          { opacity: 0, transform: `translateY(${offset}px)` },
          { opacity: 1, transform: "translateY(0)" },
        ],
        {
          duration,
          delay: index * gap,
          easing: "cubic-bezier(0.22, 1, 0.36, 1)",
          fill: "both",
        },
      );
      animation.finished
        .catch(() => undefined)
        .finally(() => {
          animation.cancel();
          finish();
        });
    });
  }

  function leave(_el: Element, done: Done) {
    done();
  }

  return { enter, leave };
}
