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
    // 窗口没焦点时 WebKit 会暂停 Web Animations：启动时不抢前台，
    // 详情会一直停在 opacity: 0。直接完成，内容立刻可见。
    if (!document.hasFocus()) {
      done();
      return;
    }
    const items = Array.from(el.querySelectorAll(selector)).slice(0, max);
    const running: Animation[] = [];
    let settled = false;
    let watchdog: number | null = null;

    /** 只完成一次：超时兜底与正常结束可能同时到达 */
    const settle = () => {
      if (settled) return;
      settled = true;
      if (watchdog !== null) window.clearTimeout(watchdog);
      done();
    };

    /** 跟踪一个动画；结束或取消后都往 finish 走 */
    const track = (animation: Animation, finish: () => void) => {
      running.push(animation);
      animation.finished
        .catch(() => undefined)
        .finally(() => {
          animation.cancel();
          finish();
        });
    };

    // WebKit 在窗口失焦 / 后台时会暂停 Web Animations，finished 可能永远不 resolve。
    // 超时后直接取消动画并完成切换，避免内容一直停在 opacity: 0（实例详情整块消失）。
    watchdog = window.setTimeout(
      () => {
        running.forEach((animation) => animation.cancel());
        settle();
      },
      duration + Math.min(step * Math.max(0, items.length - 1), 420) + 600,
    );

    try {
      if (!items.length) {
        // 整块入场：位移比逐行小一档，整页一起动时别晃得太夸张
        const animation = el.animate(
          [
            { opacity: 0, transform: "translateY(8px)" },
            { opacity: 1, transform: "translateY(0)" },
          ],
          { duration: 240, easing: "cubic-bezier(0.22, 1, 0.36, 1)", fill: "both" },
        );
        track(animation, settle);
        return;
      }

      const fadeItems = fadeSelector ? Array.from(el.querySelectorAll(fadeSelector)) : [];
      let left = items.length + fadeItems.length;
      const finish = () => {
        left -= 1;
        if (left <= 0) settle();
      };

      // 标题只做透明度渐变，位置固定，不跟着下面的分组上下移动
      fadeItems.forEach((item) => {
        const animation = item.animate([{ opacity: 0 }, { opacity: 1 }], {
          duration: 240,
          easing: "cubic-bezier(0.22, 1, 0.36, 1)",
          fill: "both",
        });
        track(animation, finish);
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
        track(animation, finish);
      });
    } catch {
      // 某个元素不可动画时不能让整页停在起始帧
      settle();
    }
  }

  function leave(_el: Element, done: Done) {
    // 退场不做动画，但**不能同步 done()**：Vue 在 out-in 模式下可能已经先摘掉旧节点，
    // 同步完成会让它的 remove 回调拿到 null，实测抛 unhandledrejection: parentNode。
    // 放到下一个宏任务，让当前 patch 收尾后再完成切换。
    window.setTimeout(done, 0);
  }

  return { enter, leave };
}
