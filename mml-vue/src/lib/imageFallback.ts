// 图片加载失败的统一兜底
//
// 应用里除了 AsyncImage，还有不少地方直接写 `<img>`：账户头像 / 皮肤贴图 / 新闻大图 /
// 资源图标 / 整合包正文里的外链图……来源都是网络或 `mml-image://`，挂掉时浏览器只会
// 画一个破图图标，很扎眼。
//
// error 事件**不冒泡**，所以在捕获阶段统一听一次：把失败的图换成一张灰底占位图。
// 用"改 src"而不是靠 CSS 盖住 —— 占位图是真能加载成功的图，不会留下破图图标，
// 也不用赌浏览器对 `content` 替换破图的支持。
// 元素上打标记，防止占位图自己失败时来回递归。
//
// **例外**：组件若自己管失败兜底，例如实例图标要回退成"渐变底 + 首字母"，
// 在 `<img>` 上写 `data-no-fallback` 即可跳过这里。必须给这个出口的原因是
// 这里的替换会触发一次 **load** —— 那些组件监听 load 来隐藏自己的占位，
// 被这一下误触发就会把占位藏掉，最后图标区整个空着。

/** 占位图：灰底 + 图片图标，深浅两种主题都看得清 */
const FALLBACK_SVG = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#9aa3af" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="8.5" cy="9" r="1.5"/><path d="m21 15-5-5L5 21"/></svg>`;

/** 占位图 data URI（样式见 styles/base.css 的 img[data-mml-fallback]） */
export const IMAGE_FALLBACK_SRC = `data:image/svg+xml,${encodeURIComponent(FALLBACK_SVG)}`;

/** 装一次即可（应用启动时调用，见 main.ts） */
export function installImageFallback() {
  document.addEventListener(
    "error",
    (e) => {
      const el = e.target;
      if (!(el instanceof HTMLImageElement)) return;
      // 组件自己管兜底（data-no-fallback）：别插手
      if (el.dataset.noFallback !== undefined) return;
      // 已经换过一次就不再动：占位图本身失败（理论上不会）时不会来回递归
      if (el.dataset.mmlFallback) return;
      el.dataset.mmlFallback = "1";
      el.src = IMAGE_FALLBACK_SRC;
    },
    true,
  );
}
