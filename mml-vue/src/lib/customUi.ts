// 自定义界面（前端骨架）
//
// 两种模式：
// - css：把一段 CSS 注入所有窗口，可以用选择器改内置控件与 CSS 变量；
// - page：主窗口整页换成自定义页面，页面里用 window.mml 调启动器命令。
//
// 数据现在先放 localStorage + 开发参数；后端导入 zip / 配置落盘 / 多窗口广播还没接，
// 见仓库根 task1.md。

import { ref } from "vue";
import { KEYS, emitChange, onStorageChange, readString, writeString } from "./storage";

/** 自定义界面模式：关闭 / 注入 CSS / 整页重写 */
export type CustomUiMode = "off" | "css" | "page";

const KEY_MODE = KEYS.customUiMode;
const KEY_CSS = KEYS.customUiCss;
const KEY_PAGE = KEYS.customUiPage;

const STYLE_ID = "mml-custom-ui-css";
const LINK_ID = "mml-custom-ui-css-link";

/** 只认三个合法值，其余一律当关闭 */
function parseMode(value: string): CustomUiMode {
  return value === "css" || value === "page" ? value : "off";
}

/** 当前模式 */
export const customUiMode = ref<CustomUiMode>(parseMode(readString(KEY_MODE)));
/** 自定义 CSS 文本，css 模式生效 */
export const customUiCss = ref(readString(KEY_CSS));
/** 整页重写的页面地址，page 模式的主窗口生效 */
export const customUiPageUrl = ref(readString(KEY_PAGE));

/** 开发参数指定的 CSS 地址（?customCss=...），只影响本次会话，不写本地 */
let devCssUrl = "";

function byId(id: string): HTMLElement | null {
  return document.getElementById(id);
}

function removeById(id: string) {
  byId(id)?.remove();
}

/** 取或建一个 head 里的元素；重复应用只更新内容，不堆节点 */
function ensure<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  id: string,
): HTMLElementTagNameMap[K] {
  const found = byId(id);
  if (found) return found as HTMLElementTagNameMap[K];
  const el = document.createElement(tag);
  el.id = id;
  document.head.appendChild(el);
  return el;
}

/**
 * 按当前状态注入 / 撤掉自定义 CSS
 *
 * page 模式不注入，由 App.vue 渲染整页 iframe。
 */
export function applyCustomUiCss() {
  if (customUiMode.value !== "css") {
    removeById(STYLE_ID);
    removeById(LINK_ID);
    return;
  }
  if (devCssUrl) {
    removeById(STYLE_ID);
    const link = ensure("link", LINK_ID);
    link.rel = "stylesheet";
    if (link.getAttribute("href") !== devCssUrl) link.setAttribute("href", devCssUrl);
    return;
  }
  removeById(LINK_ID);
  ensure("style", STYLE_ID).textContent = customUiCss.value;
}

/**
 * 初始化：读开发参数，并挂跨窗口同步
 *
 * 开发时可以绕开设置直接看效果：
 * - ?customCss=http://localhost:1521/custom.css
 * - ?customPage=http://localhost:1522/
 */
export function initCustomUi() {
  const params = new URLSearchParams(location.search);
  devCssUrl = params.get("customCss") ?? "";
  const page = params.get("customPage") ?? "";
  if (page) {
    customUiMode.value = "page";
    customUiPageUrl.value = page;
  } else if (devCssUrl) {
    customUiMode.value = "css";
  }
  applyCustomUiCss();

  onStorageChange(KEY_MODE, (value) => {
    customUiMode.value = parseMode(value ?? "");
    applyCustomUiCss();
  });
  onStorageChange(KEY_CSS, (value) => {
    customUiCss.value = value ?? "";
    applyCustomUiCss();
  });
  onStorageChange(KEY_PAGE, (value) => {
    customUiPageUrl.value = value ?? "";
  });
}

/**
 * 切换模式并落本地；其它窗口靠 storage 事件同步
 *
 * - mode: 目标模式
 */
export function setCustomUiMode(mode: CustomUiMode) {
  customUiMode.value = mode;
  writeString(KEY_MODE, mode);
  emitChange(KEY_MODE, mode);
  applyCustomUiCss();
}

/**
 * 保存 CSS 文本并立即应用
 *
 * - css: 完整 CSS 文本
 */
export function setCustomUiCss(css: string) {
  customUiCss.value = css;
  writeString(KEY_CSS, css);
  emitChange(KEY_CSS, css);
  applyCustomUiCss();
}

/**
 * 保存整页地址；主窗口会立即换成该页面
 *
 * - url: 页面入口地址
 */
export function setCustomUiPageUrl(url: string) {
  customUiPageUrl.value = url;
  writeString(KEY_PAGE, url);
  emitChange(KEY_PAGE, url);
}
