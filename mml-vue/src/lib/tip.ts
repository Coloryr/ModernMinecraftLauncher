// v-tip 全局指令：统一悬停提示，替代原生 title。
// WebView2 对原生 title 不可靠——tooltip 计时器靠元素内的 mousemove 启动，
// 鼠标从外部直接移入并停住时第一次经常不触发；且系统 tooltip 样式与暗色 UI 不搭。
// 用法：v-tip="t('xxx')"，提示按锚点摆（锚点下方居中，超出视口回收）；
// v-tip.mouse="..." 则跟着光标走（工具型内容预览，如资源窗口的存档信息）；
// 值为空串 / undefined / null 时不显示。

type TipState = {
  value: string;
  /** 是否跟随光标摆位；锚点当前的 clientRect 不看 */
  followMouse: boolean;
  enter: (e: MouseEvent) => void;
  leave: () => void;
  down: () => void;
};

const SHOW_DELAY = 400; // ms；系统默认约 1s，自绘取更跟手的节奏

let tipEl: HTMLDivElement | null = null;
let timer = 0;
let anchor: Element | null = null;
const states = new WeakMap<Element, TipState>();

/** 光标落点（视口坐标）：mouseenter 与 mousemove 各刷一次，跟随式摆位用 */
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

/** 先渲染出字再摆位；display 不能是 none，否则 offset 量不出来 */
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

/** 按锚点或光标落点摆位，超界就收回来 */
function position() {
  if (!anchor || !tipEl) return;
  const el = tipEl;
  const tw = el.offsetWidth;
  const th = el.offsetHeight;
  let x: number;
  let y: number;
  if (states.get(anchor)?.followMouse) {
    // 跟着光标：往右下偏一档；右缘贴边就往里收，底上放不下就翻到上方
    x = pointer.x + 12;
    y = pointer.y + 16;
    if (x + tw > window.innerWidth - 4) x = Math.max(4, pointer.x - tw - 12);
    if (y + th > window.innerHeight - 4) y = Math.max(4, pointer.y - th - 12);
  } else {
    // 锚点下方水平居中，超出视口就收回；下方放不下且上方有空间则放上方
    const rect = anchor.getBoundingClientRect();
    x = Math.max(4, Math.min(rect.left + rect.width / 2 - tw / 2, window.innerWidth - tw - 4));
    y = rect.bottom + 6;
    if (y + th > window.innerHeight - 4 && rect.top - th - 6 >= 4) y = rect.top - th - 6;
  }
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
}

/**
 * 提示待显示 / 显示期间挂着的鼠标监听：跟随式顺手重新摆位。
 * 提示本身 pointer-events: none（styles/tip.css），监听不会被它截走
 */
function onPointerMove(e: MouseEvent) {
  pointer.x = e.clientX;
  pointer.y = e.clientY;
  if (!anchor) return;
  // 锚点已从文档树上摘走（切页时 KeepAlive 收走、列表重渲染换行）：mouseleave 不会再来，
  // 只能靠这里收；否则提示会留在原地，跟随式的还会一路跟着光标跑
  if (!anchor.isConnected) {
    hide();
    return;
  }
  if (states.get(anchor)?.followMouse) position();
}

/** 只刷文本与位置，不改动事件绑定；updated 钩子与 mousemove 共用 */
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

function bind(el: HTMLElement, value: string, followMouse: boolean) {
  const state: TipState = {
    value,
    followMouse,
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
  /** `.mouse` 修饰符：跟着光标摆位，而不是按锚点 */
  modifiers?: { mouse?: boolean };
};

export const vTip = {
  mounted(el: HTMLElement, binding: TipBinding) {
    bind(el, String(binding.value ?? ""), binding.modifiers?.mouse === true);
  },
  updated(el: HTMLElement, binding: TipBinding) {
    const state = states.get(el);
    if (!state) return;
    state.value = String(binding.value ?? "");
    state.followMouse = binding.modifiers?.mouse === true;
    // 显示中内容变了（如状态文本刷新）：即时重刷文本与位置
    if (anchor === el) refresh(el);
  },
  unmounted(el: HTMLElement) {
    unbind(el);
  },
};
