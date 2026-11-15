// 右侧换页的"分层错峰"入场：标题先出，下面的分组 / 卡片按顺序依次落下
//
// 用 Web Animations API 而不是 CSS 类：子元素数量不固定，延迟要按实际数量摊开；
// 全局"界面动画"关掉时直接完成，不阻塞切换。
//
// 没有行可错峰时（空列表、加载骨架、数据还在路上）整块淡入：子页切换常常先落到
// 一个还没有行的空页上，只淡标题的话看着就是硬切。
//
// 透明高斯模糊（html.bg-frosted）开着时，**淡入不能落在毛玻璃的祖先上**：祖先带不透明度动画，
// 子树的 backdrop-filter 就采不到背后的背景图，卡片会从毛玻璃退成半透明底板，动画结束才恢复。
// 位移不受影响（实测：祖先 transform 与元素自己淡入都不破坏模糊，只有祖先 opacity 会），
// 所以含毛玻璃的容器只做位移，淡入拆给"不是毛玻璃祖先"的元素，见 fadeTargets。
import { animations } from "../lib/appearance";

/** 完成回调，Vue Transition 用 */
type Done = () => void;

/** 统一的入场缓动，各处动画共用一条曲线 */
const EASING = "cubic-bezier(0.22, 1, 0.36, 1)";

/** 本页有没有毛玻璃表面：没有这个类时一棵子树里都不会有 backdrop-filter，跳过扫描 */
function frostedMode(): boolean {
  return document.documentElement.classList.contains("bg-frosted");
}

/**
 * 元素自己是否带背景模糊
 *
 * - `el`: 目标元素
 *
 * # 返回值
 *
 * 计算样式里 backdrop-filter 不是 none 时为 true
 */
function hasBackdrop(el: Element): boolean {
  const style = getComputedStyle(el);
  const value =
    style.backdropFilter ||
    (style as CSSStyleDeclaration & { webkitBackdropFilter?: string }).webkitBackdropFilter;
  return !!value && value !== "none";
}

/**
 * 子树里带背景模糊的元素，不含自己
 *
 * - `root`: 要扫描的子树根
 *
 * # 返回值
 *
 * 文档顺序；没有时为空数组
 */
function frostedInside(root: Element): HTMLElement[] {
  const out: HTMLElement[] = [];
  root.querySelectorAll("*").forEach((el) => {
    if (hasBackdrop(el)) out.push(el as HTMLElement);
  });
  return out;
}

/**
 * 这次淡入该落在哪些元素上
 *
 * 沿"含毛玻璃"的分支往下走，遇到不再包含毛玻璃的元素就停：它连同整棵子树一起淡入。
 * 毛玻璃元素本身也在结果里 —— 它自己的不透明度不影响自己的 backdrop-filter。
 *
 * - `root`: 要淡入的那块
 * - `frosted`: root 子树里的毛玻璃元素，见 {@link frostedInside}
 *
 * # 返回值
 *
 * 需要各自挂一次透明度动画的元素
 */
function fadeTargets(root: Element, frosted: HTMLElement[]): HTMLElement[] {
  const out: HTMLElement[] = [];
  const walk = (node: Element) => {
    for (const child of Array.from(node.children)) {
      // 子元素自己就是毛玻璃不算"祖先"：它自己淡入是安全的
      const isAncestor = frosted.some((target) => target !== child && child.contains(target));
      if (isAncestor) walk(child);
      else out.push(child as HTMLElement);
    }
  };
  walk(root);
  return out;
}

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
    const fadeItems = fadeSelector ? Array.from(el.querySelectorAll(fadeSelector)) : [];
    const frosted = frostedMode();
    const running: Animation[] = [];
    /** 本轮建出来的动画：全部结束才算这次入场结束 */
    const made: Animation[] = [];
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

    /**
     * 建一个动画并登记
     *
     * - `target`: 动画作用的元素
     * - `frames`: 关键帧
     * - `ms`: 时长，单位毫秒
     * - `delay`: 延迟，单位毫秒
     */
    const make = (target: Element, frames: Keyframe[], ms: number, delay: number) => {
      made.push(target.animate(frames, { duration: ms, delay, easing: EASING, fill: "both" }));
    };

    /**
     * 淡入：毛玻璃开着时拆给不会破坏模糊的元素，否则就淡它自己
     *
     * - `target`: 原本要淡的那块
     * - `ms` / `delay`: 时长与延迟，单位毫秒
     */
    const fadeIn = (target: Element, ms: number, delay: number) => {
      const inner = frosted ? frostedInside(target) : [];
      const parts = inner.length ? fadeTargets(target, inner) : [target as HTMLElement];
      parts.forEach((part) => make(part, [{ opacity: 0 }, { opacity: 1 }], ms, delay));
    };

    /**
     * 位移 + 淡入：含毛玻璃的容器只位移，淡入交给 {@link fadeIn} 拆下去
     *
     * - `target`: 要入场的元素
     * - `ms` / `delay`: 时长与延迟，单位毫秒
     * - `shift`: 入场位移，单位像素
     */
    const slideIn = (target: Element, ms: number, delay: number, shift: number) => {
      const inner = frosted ? frostedInside(target) : [];
      if (!inner.length) {
        make(
          target,
          [
            { opacity: 0, transform: `translateY(${shift}px)` },
            { opacity: 1, transform: "translateY(0)" },
          ],
          ms,
          delay,
        );
        return;
      }
      make(
        target,
        [{ transform: `translateY(${shift}px)` }, { transform: "translateY(0)" }],
        ms,
        delay,
      );
      fadeTargets(target, inner).forEach((part) => {
        make(part, [{ opacity: 0 }, { opacity: 1 }], ms, delay);
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
        slideIn(el, 240, 0, 8);
      } else {
        // 标题只做透明度渐变，位置固定，不跟着下面的分组上下移动
        fadeItems.forEach((item) => fadeIn(item, 240, 0));

        // 元素多时压缩间隔，整套入场不超过 320 + 420ms
        const gap = items.length > 1 ? Math.min(step, 420 / (items.length - 1)) : 0;
        items.forEach((item, index) => slideIn(item, duration, index * gap, offset));
      }

      if (!made.length) {
        settle();
        return;
      }
      let left = made.length;
      const finish = () => {
        left -= 1;
        if (left <= 0) settle();
      };
      made.forEach((animation) => track(animation, finish));
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
