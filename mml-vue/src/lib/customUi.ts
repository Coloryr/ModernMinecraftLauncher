// 自定义界面（前端应用层）
//
// 三种换皮互相独立，开关与 CSS 条目都在 gui_config.client：
// - 自定义主页面：主窗口内容区的整页替换，见 windows/main 的 CustomHomePage；
// - 自定义 CSS：按条目挂样式表；包里有 custom.js 时再加载脚本，挂载后调用它的入口函数；
// - 完全自定义界面：主窗口整体换成自定义页面，地址交给 App.vue 的 iframe。
//
// 内容不在前端：CSS、入口脚本与整页包都由后端从运行目录的 custom_ui/ 现读。
// 这里只做：启动拉一次 custom_ui_status；按状态挂链接 / 加载脚本 / 给整页地址；
// 挂载完成后调用入口函数；收到 custom-ui-change 或 client-config-change 后重拉并重新应用。
//
// 入口脚本约定：包根放 custom.js，执行时调用 window.__mmlCustomCss.push(fn) 注册；
// 一个包可以注册多个函数，fn 收到 CustomCssContext（loadCss / select / replace / onCleanup）。
//
// 开发时可以绕开设置直接看效果，只影响本次会话、不写配置：
// - ?customCss=http://localhost:1521/custom.css
// - ?customPage=http://localhost:1522/

import { createApp, ref, watch, type Component } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CustomCssContext,
  CustomCssReplaceOptions,
  WindowPageRegistry,
} from "mml-ipc";
import { onClientConfigChange, onCustomUiChange } from "./api";
import { commands, type CustomUiInfoDto } from "./bindings";
import { saveGuiConfig } from "./guiConfig";
import { resolvedTheme, theme } from "./theme";

/** 后端状态；null 表示还没拉到或后端不可用 */
export const customUiStatus = ref<CustomUiInfoDto | null>(null);
/** 完全自定义界面开关：开启且已导入时主窗口整页替换 */
export const customPageOn = ref(false);
/** 整页入口地址，来自后端；开发参数可覆盖 */
export const customUiPageUrl = ref("");

/** 整页包里的窗口页面回调版本号；注册 / 清理后自增，驱动 App.vue 重算 */
export const windowPageVersion = ref(0);

/** 整页包注册的窗口页面回调：按窗口 kind 返回要替换的组件 */
type WindowPageFn = (kind: string) => Component | null;

/** 当前整页包的回调 */
const windowPages: WindowPageFn[] = [];

/** 整页入口脚本与附加样式，切包 / 关包时移除 */
let pageScript: HTMLScriptElement | null = null;
let pageStyle: HTMLLinkElement | null = null;
/** 已加载的脚本地址，避免重复插入 */
let pageScriptUrl = "";
/** 安装窗口页面注册表；脚本执行时把自己的回调 push 进来 */
function installWindowPageRegistry() {
  const registry: WindowPageRegistry = {
    push(fn) {
      windowPages.push(fn as WindowPageFn);
      windowPageVersion.value += 1;
    },
  };
  (window as unknown as { __mmlWindowPages?: WindowPageRegistry }).__mmlWindowPages = registry;
}

/**
 * 按窗口 kind 取整页包提供的页面组件
 *
 * - `kind`: 窗口标识，如 main / settings
 *
 * # 返回值
 *
 * 第一个非空的组件；没有时返回 null，窗口继续用内置页面
 */
export function resolveWindowPage(kind: string): Component | null {
  // 读一下版本号，让调用方的 computed 在脚本注册后重算
  void windowPageVersion.value;
  for (const fn of windowPages) {
    try {
      const page = fn(kind);
      if (page) return page;
    } catch (err) {
      console.error("[custom-page] 窗口页面回调失败", kind, err);
    }
  }
  return null;
}

/** 样式链接 id 前缀，后面接包 id */
const LINK_PREFIX = "mml-custom-ui-css-";
/** 入口脚本 id 前缀，后面接包 id */
const SCRIPT_PREFIX = "mml-custom-ui-js-";

/** 包里注册的入口函数 */
type EntryFn = (ctx: CustomCssContext) => void;

/** 一个已启用包的运行时状态 */
interface PackageState {
  /** 包 id */
  id: string;
  /** 样式地址，空串表示文件缺失 */
  cssUrl: string;
  /** 入口脚本地址，空串表示包里没有 custom.js */
  scriptUrl: string;
  /** 还没执行的入口函数 */
  pending: EntryFn[];
  /** 注册的清理函数 */
  cleanups: Array<() => void>;
  /** 已插入的脚本元素 */
  script?: HTMLScriptElement;
}

/** id 到包状态的表；脚本通过 id 找回自己的包 */
const packages = new Map<string, PackageState>();
/** 挂载完成前入口函数先排队 */
let started = false;
/** 开发参数指定的 CSS 地址 */
let devCssUrl = "";
/** 开发参数指定的整页地址 */
let devPageUrl = "";

declare global {
  interface Window {
    /** 自定义 CSS 包的入口注册表；由 custom.js 调用，mml-vue 消费 */
    __mmlCustomCss?: { push: (fn: EntryFn) => void };
  }
}

/** 入口注册表；脚本执行期间用 document.currentScript 取包 id */
function installRegistry() {
  window.__mmlCustomCss = {
    push(fn) {
      const el = document.currentScript as HTMLScriptElement | null;
      const id = el?.dataset.customUiId ?? "";
      const pkg = packages.get(id);
      if (!pkg) return;
      pkg.pending.push(fn);
      flush(pkg);
    },
  };
}

function byId(id: string): HTMLElement | null {
  return document.getElementById(id);
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

/** 挂上或更新一个包的样式链接 */
function ensureLink(id: string, url: string) {
  if (!url) return;
  const link = ensure("link", LINK_PREFIX + id);
  link.rel = "stylesheet";
  if (link.getAttribute("href") !== url) link.setAttribute("href", url);
}

/**
 * 把自定义 CSS 同步成一组 link，顺序即数组顺序
 *
 * 只留下这一轮要用的链接，href 没变就不动它，避免每次应用都重新拉一遍样式。
 *
 * - `items`: 要生效的包 id 与样式地址
 */
function syncLinks(items: Array<{ id: string; url: string }>) {
  const ids = new Set(items.map((item) => LINK_PREFIX + item.id));
  for (const el of Array.from(document.querySelectorAll('link[id^="' + LINK_PREFIX + '"]'))) {
    if (!ids.has(el.id)) el.remove();
  }
  for (const item of items) ensureLink(item.id, item.url);
}

/** 执行一个包里排队中的入口函数 */
function flush(pkg: PackageState) {
  if (!started) return;
  const ctx = makeContext(pkg);
  for (const fn of pkg.pending.splice(0)) {
    try {
      fn(ctx);
    } catch (err) {
      console.error("[custom-css] 入口执行失败", pkg.id, err);
    }
  }
}

/** 包被关闭 / 删除 / 换脚本时收尾：跑清理、移除脚本 */
function teardown(pkg: PackageState) {
  pkg.pending.length = 0;
  // 同一控件可能被连续替换，按相反顺序撤销才能回到最初状态。
  for (const fn of pkg.cleanups.splice(0).reverse()) {
    try {
      fn();
    } catch (err) {
      console.error("[custom-css] 清理失败", pkg.id, err);
    }
  }
  pkg.script?.remove();
  pkg.script = undefined;
}

/** 替换一个控件的文字 / 类名 / 内联样式 */
function replaceControl(
  target: string | Element,
  options: CustomCssReplaceOptions,
  onCleanup: (fn: () => void) => void,
): Element | null {
  const el = typeof target === "string" ? document.querySelector(target) : target;
  if (!el) return null;
  // 组件替换：原控件留在 DOM 里但隐藏，Vue 组件挂在旁边；关包时还原
  if (options.component) {
    const parent = el.parentNode;
    if (!parent) return null;
    const host = document.createElement("div");
    // display: contents 让组件根节点直接参与父级布局，不多占一层
    host.style.display = "contents";
    parent.insertBefore(host, el.nextSibling);
    const previous = (el as HTMLElement).style.display;
    (el as HTMLElement).style.display = "none";
    const app = createApp(options.component as Component, options.props ?? {});
    app.mount(host);
    onCleanup(() => {
      app.unmount();
      host.remove();
      (el as HTMLElement).style.display = previous;
    });
    return host;
  }
  if (options.text !== undefined) {
    const textNode = Array.from(el.childNodes).find(
      (node) => node.nodeType === Node.TEXT_NODE && (node.textContent ?? "").trim() !== "",
    );
    if (textNode) {
      const previous = textNode.textContent;
      onCleanup(() => { textNode.textContent = previous; });
      textNode.textContent = options.text;
    } else {
      // 保留原节点，恢复时不丢失子控件的事件监听与状态。
      const children = Array.from(el.childNodes);
      onCleanup(() => { el.replaceChildren(...children); });
      el.textContent = options.text;
    }
  }
  if (options.className) {
    const added = options.className.split(/\s+/).filter((name) => name && !el.classList.contains(name));
    onCleanup(() => { el.classList.remove(...added); });
    el.classList.add(...added);
  }
  if (options.style) {
    const previous = el.getAttribute("style");
    onCleanup(() => {
      if (previous === null) el.removeAttribute("style");
      else el.setAttribute("style", previous);
    });
    Object.assign((el as HTMLElement).style, options.style);
  }
  return el;
}

/** 打包一次调用用的上下文 */
function makeContext(pkg: PackageState): CustomCssContext {
  return {
    id: pkg.id,
    loadCss: () => ensureLink(pkg.id, pkg.cssUrl),
    select: <T extends Element = HTMLElement>(selector: string) => document.querySelector<T>(selector),
    replace: (target, options) =>
      replaceControl(target, options, (fn) => {
        pkg.cleanups.push(fn);
      }),
    onCleanup: (fn) => {
      pkg.cleanups.push(fn);
    },
  };
}

/** host 侧的 window.mml：整页包组件直接跑在窗口里时用它，命令与事件走 Tauri */
interface HostMml {
  invoke<T = unknown>(cmd: string, args?: Record<string, unknown> | null): Promise<T>;
  on(event: string, cb: (payload: unknown) => void): () => void;
  onTheme(cb: (theme: "Dark" | "Light") => void): () => void;
  readonly theme: "Dark" | "Light";
  ready: Promise<void>;
}

/** 给直接渲染的整页 / 主页组件装 window.mml；已有时不覆盖 */
export function ensureHostMml() {
  const target = window as unknown as { mml?: HostMml };
  if (target.mml) return;
  target.mml = {
    invoke: (cmd, args) => invoke(cmd, args ?? undefined),
    on(event, cb) {
      let off: (() => void) | null = null;
      let cancelled = false;
      void listen(event, (payload) => cb(payload.payload)).then((unlisten) => {
        if (cancelled) unlisten();
        else off = unlisten;
      });
      return () => {
        cancelled = true;
        off?.();
      };
    },
    onTheme: (cb) =>
      watch(theme, () => cb(resolvedTheme() === "Light" ? "Light" : "Dark")),
    get theme() {
      return resolvedTheme() === "Light" ? "Light" : "Dark";
    },
    ready: Promise.resolve(),
  };
}

/** 加载整页入口脚本；失败也 resolve，窗口回落到内置页面 */
function loadPageScript(url: string): Promise<void> {
  return new Promise((resolve) => {
    const script = document.createElement("script");
    script.id = "mml-custom-page-script";
    script.src = url;
    script.async = true;
    script.addEventListener("load", () => resolve());
    script.addEventListener("error", () => {
      console.warn("[custom-page] 入口脚本加载失败", url);
      resolve();
    });
    document.head.appendChild(script);
    pageScript = script;
  });
}

/** 挂上或更新整页包附带的样式 */
function ensurePageStyle(url: string) {
  if (!url) return;
  const link = pageStyle ?? document.createElement("link");
  link.id = "mml-custom-page-style";
  link.rel = "stylesheet";
  if (link.getAttribute("href") !== url) link.setAttribute("href", url);
  if (!pageStyle) document.head.appendChild(link);
  pageStyle = link;
}

/** 关包 / 切包：移除脚本与样式、清回调、撤掉 host 的 window.mml */
function teardownWindowPages() {
  pageScript?.remove();
  pageScript = null;
  pageStyle?.remove();
  pageStyle = null;
  pageScriptUrl = "";
  windowPages.length = 0;
  windowPageVersion.value += 1;
}

/** 按状态同步整页包入口脚本；必须在挂载前 await，窗口页面才能从第一帧就是替换后的 */
async function syncWindowPages(info: CustomUiInfoDto | null) {
  const url = info?.pageOn ? info.pageScriptUrl : "";
  if (!url) {
    teardownWindowPages();
    return;
  }
  if (pageScriptUrl === url) {
    ensurePageStyle(info?.pageStyleUrl ?? "");
    return;
  }
  teardownWindowPages();
  installWindowPageRegistry();
  await loadPageScript(url);
  pageScriptUrl = url;
  ensurePageStyle(info?.pageStyleUrl ?? "");
  if (windowPages.length > 0) {
    ensureHostMml();
    windowPageVersion.value += 1;
  }
}

/**
 * 按状态同步入口脚本：新包插 script，撤掉的包跑清理并删节点
 *
 * 脚本以 classic script 加载，执行期间 document.currentScript 就是它自己，
 * 所以入口注册表能按 data-custom-ui-id 找回包。
 */
function syncScripts(items: Array<{ id: string; url: string; scriptUrl: string }>) {
  const ids = new Set(items.map((item) => item.id));
  for (const [id, pkg] of packages) {
    if (!ids.has(id)) {
      teardown(pkg);
      packages.delete(id);
    }
  }
  for (const item of items) {
    let pkg = packages.get(item.id);
    if (!pkg) {
      pkg = { id: item.id, cssUrl: item.url, scriptUrl: item.scriptUrl, pending: [], cleanups: [] };
      packages.set(item.id, pkg);
    }
    pkg.cssUrl = item.url;
    if (pkg.scriptUrl !== item.scriptUrl) {
      teardown(pkg);
      pkg.scriptUrl = item.scriptUrl;
    }
    if (!item.scriptUrl || pkg.script) continue;
    const script = document.createElement("script");
    script.id = SCRIPT_PREFIX + item.id;
    script.dataset.customUiId = item.id;
    script.src = item.scriptUrl;
    script.async = true;
    script.addEventListener("error", () =>
      console.warn("[custom-css] 入口脚本加载失败", item.scriptUrl),
    );
    document.head.appendChild(script);
    pkg.script = script;
  }
}

/** 按当前状态应用样式与入口脚本；整页模式不注入，由 App.vue 渲染整页 iframe */
export function applyCustomUi() {
  if (devCssUrl) {
    syncLinks([{ id: "dev", url: devCssUrl }]);
    syncScripts([]);
    return;
  }
  const info = customUiStatus.value;
  if (!info?.cssOn) {
    syncLinks([]);
    syncScripts([]);
    return;
  }
  const items = info.css
    .filter((item) => item.enabled && item.url)
    .map((item) => ({ id: item.id, url: item.url, scriptUrl: item.scriptUrl }));
  syncLinks(items);
  syncScripts(items);
}

/** 挂载完成后放行入口函数：先挂载再替换，入口里才查得到控件 */
export function startCustomCss() {
  started = true;
  for (const pkg of packages.values()) flush(pkg);
}

/** 把后端状态落到响应式变量；开发参数优先 */
function applyInfo(info: CustomUiInfoDto | null) {
  customUiStatus.value = info;
  if (devPageUrl) {
    customPageOn.value = true;
    customUiPageUrl.value = devPageUrl;
    return;
  }
  if (devCssUrl) {
    customPageOn.value = false;
    return;
  }
  customPageOn.value = info?.pageOn ?? false;
  customUiPageUrl.value = info?.pageEntryUrl ?? "";
}

/** 重拉后端状态并重新应用；后端不可用时保留开发参数那套 */
async function refreshStatus() {
  try {
    applyInfo(await commands.customUi.status());
  } catch {
    /* 纯浏览器预览没有后端，忽略 */
  }
  applyCustomUi();
  await syncWindowPages(customUiStatus.value);
}

/**
 * 初始化：读开发参数、拉一次后端状态，并挂跨窗口同步
 *
 * 必须在挂载前 await：整页模式先拿到开关与入口地址，否则会先闪一下内置界面。
 * 入口脚本会在这时开始加载，但入口函数要等 startCustomCss 之后才执行。
 *
 * # 返回值
 *
 * 状态就绪的 Promise
 */
export async function initCustomUi() {
  installRegistry();
  const params = new URLSearchParams(location.search);
  devCssUrl = params.get("customCss") ?? "";
  devPageUrl = params.get("customPage") ?? "";

  await refreshStatus();

  // 导入 / 删除文件不改配置，只有这一条事件；非 Tauri 环境下 listen 会拒绝，忽略
  onCustomUiChange(() => void refreshStatus()).catch(() => {});
  // 开关与条目走 gui_config，多窗口下靠这条广播；与 CSS 无关的设置变更也重拉一遍，
  // 状态现算很便宜，省得再比一遍字段
  onClientConfigChange(() => void refreshStatus()).catch(() => {});
}

/**
 * 关闭完全自定义界面并落配置
 *
 * 供整页界面的开发逃生按钮用；生产环境关掉它的入口是设置窗口。
 */
export async function exitCustomUi() {
  devPageUrl = "";
  customPageOn.value = false;
  customUiPageUrl.value = "";
  await saveGuiConfig({ client: { customPageOn: false } });
}
