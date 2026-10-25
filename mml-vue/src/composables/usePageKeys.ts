// 鼠标侧键翻页
//
// 鼠标的"后退 / 前进"侧键在 Chromium 里是 XBUTTON1 / XBUTTON2，DOM 上表现为
// `mouseup` 且 `button` 为 3（后退）/ 4（前进）。列表分页的窗口把它们接到上一页 / 下一页上，
// 并拦掉默认行为 —— 不拦的话 WebView 会自己回退历史，界面直接跳走。
//
// **只对当前活动的页面生效**：单窗口模式下多个窗口同时挂载，不能一次点击翻所有页。
import { onMounted, onUnmounted } from "vue";
import { usePageActive } from "../lib/pageActive";

/** 鼠标后退键的 button 值 */
const BUTTON_BACK = 3;
/** 鼠标前进键的 button 值 */
const BUTTON_FORWARD = 4;

type PageKeyOptions = {
  /** 上一页；没有上一页时调用方自己判（拿到就说明能翻） */
  prev?: () => void;
  /** 下一页 */
  next?: () => void;
  /** 这一页要不要响应；缺省表示一直响应，用来在详情页打开时让位 */
  enabled?: () => boolean;
};

/**
 * 把鼠标侧键接到上一页 / 下一页
 *
 * - `options`: 见 [`PageKeyOptions`]
 */
export function usePageKeys(options: PageKeyOptions) {
  const active = usePageActive();

  /** 侧键松开：后退翻上一页、前进翻下一页 */
  function onMouseUp(e: MouseEvent) {
    if (e.button !== BUTTON_BACK && e.button !== BUTTON_FORWARD) return;
    // 先拦掉历史回退 / 前进，否则界面会跟着 WebView 跳走
    e.preventDefault();
    e.stopPropagation();
    if (!active.value) return;
    if (options.enabled && !options.enabled()) return;
    if (e.button === BUTTON_BACK) options.prev?.();
    else options.next?.();
  }

  onMounted(() => window.addEventListener("mouseup", onMouseUp));
  onUnmounted(() => window.removeEventListener("mouseup", onMouseUp));
}
