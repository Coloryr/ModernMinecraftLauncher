// v-tip 全局指令：统一悬停提示，替代原生 title。
// WebView2 对原生 title 不可靠——tooltip 计时器靠元素内的 mousemove 启动，
// 鼠标从外部直接移入并停住时第一次经常不触发；且系统 tooltip 样式与暗色 UI 不搭。
// 用法：v-tip="t('xxx')"；值为空串 / undefined / null 时不显示。

type TipState = {
  value: string;
  enter: () => void;
  leave: () => void;
  down: () => void;
};

const SHOW_DELAY = 400; // ms；系统默认约 1s，自绘取更跟手的节奏

let tipEl: HTMLDivElement | null = null;
let timer = 0;
let anchor: Element | null = null;
const states = new WeakMap<Element, TipState>();

function ensureEl(): HTMLDivElement {
  if (!tipEl) {
    tipEl = document.createElement("div");
    tipEl.className = "app-tip";
    document.body.appendChild(tipEl);
  }
  return tipEl;
}

function hide() {
  if (timer) {
    clearTimeout(timer);
    timer = 0;
  }
  if (anchor) {
    window.removeEventListener("scroll", hide, true);
    anchor = null;
  }
  if (tipEl) tipEl.classList.remove("show");
}

function show() {
  timer = 0;
  if (!anchor) return;
  const text = states.get(anchor)?.value;
  if (!text) return;
  const el = ensureEl();
  el.textContent = text;
  el.classList.add("show");
  // 先渲染出来再量尺寸，display 不能是 none，随后按锚点定位：
  // 锚点下方水平居中，超出视口就收回；下方放不下且上方有空间则放上方
  const rect = anchor.getBoundingClientRect();
  const tw = el.offsetWidth;
  const th = el.offsetHeight;
  let x = rect.left + rect.width / 2 - tw / 2;
  x = Math.max(4, Math.min(x, window.innerWidth - tw - 4));
  let y = rect.bottom + 6;
  if (y + th > window.innerHeight - 4 && rect.top - th - 6 >= 4) y = rect.top - th - 6;
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
}

function bind(el: HTMLElement, value: string) {
  const state: TipState = {
    value,
    enter: () => {
      const cur = states.get(el);
      if (!cur?.value) return;
      anchor = el;
      // 滚动时锚点移位，固定定位的提示会飘，干脆先收掉
      window.addEventListener("scroll", hide, true);
      timer = window.setTimeout(show, SHOW_DELAY);
    },
    leave: hide,
    down: hide,
  };
  states.set(el, state);
  el.addEventListener("mouseenter", state.enter);
  el.addEventListener("mouseleave", state.leave);
  el.addEventListener("mousedown", state.down);
}

function unbind(el: HTMLElement) {
  const state = states.get(el);
  if (!state) return;
  el.removeEventListener("mouseenter", state.enter);
  el.removeEventListener("mouseleave", state.leave);
  el.removeEventListener("mousedown", state.down);
  states.delete(el);
  if (anchor === el) hide();
}

export const vTip = {
  mounted(el: HTMLElement, binding: { value: unknown }) {
    bind(el, String(binding.value ?? ""));
  },
  updated(el: HTMLElement, binding: { value: unknown }) {
    const state = states.get(el);
    if (!state) return;
    state.value = String(binding.value ?? "");
    // 显示中内容变了（如状态文本刷新）：即时重算文本与位置
    if (anchor === el) show();
  },
  unmounted(el: HTMLElement) {
    unbind(el);
  },
};
