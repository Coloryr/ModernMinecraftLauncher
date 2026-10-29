// v-tip 全局指令：统一悬停提示，替代原生 title。
// WebView2 对原生 title 不可靠——tooltip 计时器靠元素内的 mousemove 启动，
// 鼠标从外部直接移入并停住时第一次经常不触发；且系统 tooltip 样式与暗色 UI 不搭。
//
// **一律跟着光标摆**（右下偏一档，贴边回收）：卡片、长行这类宽元素按锚点居中挂下来
// 会离光标很远，看着像"没跟着手走"。早先按锚点摆、只有 `v-tip.mouse` 跟随，两种混用
// 观感不一致，现在统一跟随；`.mouse` 仍可写，只是不再有区别。
//
// 值为空串 / undefined / null 时不显示。

type TipState = {
  /** 当前提示文案；空串表示不显示 */
  value: string;
  /** 指针进入：记落点、挂监听并起延时 */
  enter: (e: MouseEvent) => void;
  /** 指针离开：立即收起 */
  leave: () => void;
  /** 按下鼠标：收起，避免挡点击目标 */
  down: () => void;
};

/** 悬停多久后显示，单位 ms；系统默认约 1s，自绘取更跟手的节奏 */
const SHOW_DELAY = 400;

let tipEl: HTMLDivElement | null = null;
let timer = 0;
let anchor: Element | null = null;
const states = new WeakMap<Element, TipState>();

/** 光标落点（视口坐标）：mouseenter 与 mousemove 各刷一次 */
const pointer = { x: 0, y: 0 };

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
    window.removeEventListener("blur", hide);
    document.removeEventListener("mousemove", onPointerMove);
    anchor = null;
  }
  if (tipEl) tipEl.classList.remove("show");
}

// 先渲染出字再摆位；display 不能是 none，否则 offset 量不出来
function show() {
  timer = 0;
  if (!anchor) return;
  // 等这四百毫秒的工夫里锚点已经没了（切页、行被换掉）：不显示，也不用摆位
  if (!anchor.isConnected) {
    hide();
    return;
  }
  const text = states.get(anchor)?.value;
  if (!text) return;
  const el = ensureEl();
  el.textContent = text;
  el.classList.add("show");
  position();
}

/** 按光标落点摆位，贴边就收回来 */
function position() {
  if (!anchor || !tipEl) return;
  const el = tipEl;
  const tw = el.offsetWidth;
  const th = el.offsetHeight;
  // 往右下偏一档；右缘 / 底部放不下就往里收
  let x = pointer.x + 12;
  let y = pointer.y + 16;
  if (x + tw > window.innerWidth - 4) x = Math.max(4, pointer.x - tw - 12);
  if (y + th > window.innerHeight - 4) y = Math.max(4, pointer.y - th - 12);
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
}

/**
 * 提示待显示 / 显示期间挂着的鼠标监听：重新摆位。
 * 提示本身 pointer-events: none（styles/tip.css），监听不会被它截走
 */
function onPointerMove(e: MouseEvent) {
  pointer.x = e.clientX;
  pointer.y = e.clientY;
  if (!anchor) return;
  // 锚点已从文档树上摘走（切页时 KeepAlive 收走、列表重渲染换行）：mouseleave 不会再来，
  // 只能靠这里收；否则提示会留在原地，一路跟着光标跑
  if (!anchor.isConnected) {
    hide();
    return;
  }
  position();
}

/** 只刷文本，不改动事件绑定；updated 钩子与 mousemove 共用 */
function refresh(el: HTMLElement) {
  const text = states.get(el)?.value;
  // 显示中内容被清空（如行状态不再有提示）：直接收掉，别留上一份
  if (!text) {
    if (anchor === el) hide();
    return;
  }
  if (!tipEl) return;
  tipEl.textContent = text;
  position();
}

function bind(el: HTMLElement, value: string) {
  const state: TipState = {
    value,
    enter: (e) => {
      const cur = states.get(el);
      if (!cur?.value) return;
      // 上一份挂起或显示中的提示先收干净：不然旧定时器到点还会把它显示出来
      hide();
      anchor = el;
      // 落点先按进来这一下记，四百毫秒后才显示，期间 mousemove 会继续刷
      pointer.x = e.clientX;
      pointer.y = e.clientY;
      // 滚动时锚点移位，固定定位的提示会飘，干脆先收掉
      window.addEventListener("scroll", hide, true);
      // 切到别的窗口（多窗口模式）时这一侧不会再有鼠标事件，失焦就要收掉
      window.addEventListener("blur", hide);
      document.addEventListener("mousemove", onPointerMove);
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

type TipBinding = {
  value: unknown;
};

export const vTip = {
  mounted(el: HTMLElement, binding: TipBinding) {
    bind(el, String(binding.value ?? ""));
  },
  updated(el: HTMLElement, binding: TipBinding) {
    const state = states.get(el);
    if (!state) return;
    state.value = String(binding.value ?? "");
    // 显示中内容变了（如状态文本刷新）：即时重刷文本与位置
    if (anchor === el) refresh(el);
  },
  unmounted(el: HTMLElement) {
    unbind(el);
  },
};
