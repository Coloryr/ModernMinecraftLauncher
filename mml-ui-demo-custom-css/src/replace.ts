// 入口函数：mml-vue 加载 custom.js 后调用
//
// 选择实例启动按钮，用 Vue 写的替换按钮挂到它旁边；原按钮隐藏，点击仍走原来的启动逻辑。
// 替换组件在 src/ReplaceButton.vue，打包时由 Vite 编译进 custom.js。

import type { CustomCssContext } from "mml-ipc";
import ReplaceButton from "./ReplaceButton.vue";

/** 主页面实例启动按钮 */
const BUTTON = ".launch-actions .launch-main-btn";

/**
 * 选择并替换实例启动按钮
 *
 * - `ctx`: mml-vue 给的上下文
 */
function replace(ctx: CustomCssContext) {
  ctx.loadCss();

  /** 已经挂过的原按钮，避免 MutationObserver 重复挂载 */
  const replaced = new Set<HTMLElement>();
  let scheduled = false;

  const apply = () => {
    scheduled = false;
    const el = ctx.select<HTMLElement>(BUTTON);
    if (!el || replaced.has(el)) return;
    replaced.add(el);
    ctx.replace(el, {
      component: ReplaceButton,
      props: {
        text: "开始冒险",
        // 原按钮被隐藏但仍在 DOM 里，click 仍会触发它自己的启动处理
        onLaunch: () => el.click(),
      },
    });
  };

  const schedule = () => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(apply);
  };

  // 启动按钮随实例列表异步出现，等它进 DOM 再替换
  const observer = new MutationObserver(schedule);
  observer.observe(document.body, { childList: true, subtree: true });
  apply();

  ctx.onCleanup(() => {
    observer.disconnect();
    replaced.clear();
  });
}

// 入口注册：mml-vue 加载脚本时 document.currentScript 就是它自己，注册表按 id 归档
const registry = (window as unknown as {
  __mmlCustomCss: { push: (fn: (ctx: CustomCssContext) => void) => void };
}).__mmlCustomCss;
registry.push(replace);
