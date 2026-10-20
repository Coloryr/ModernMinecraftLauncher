// 自定义主页面（内容区组件）加载
//
// 与整页包的窗口回调分开：这里只替换主窗口的「主页内容区」，不是整个窗口。
// 包根 custom.js 在挂载前加载，脚本调用 window.__mmlHomePages.push(fn) 登记；
// App.vue 侧不参与，由 windows/main 的 CustomHomePage 按回调渲染。
//
// 没有 custom.js 时仍走原来的 iframe（index.html + mml-home 协议）。

import { ref, type Component } from "vue";
import type { HomePageRegistry } from "mml-ipc";
import { onClientConfigChange, onCustomHomeChange } from "./api";
import { commands } from "./bindings";
import { ensureHostMml } from "./customUi";
import { currentKind } from "../windows/windowManager";

/** 主页组件回调版本号；注册 / 清理后自增，驱动 CustomHomePage 重算 */
export const homePageVersion = ref(0);

/** 回调：返回组件就替换主页内容区 */
type HomePageFn = () => Component | null;

const homePages: HomePageFn[] = [];
let homeScript: HTMLScriptElement | null = null;
let homeStyle: HTMLLinkElement | null = null;
let loadedScriptUrl = "";
let started = false;

/** 安装主页注册表；脚本执行时 push 回调 */
function installHomeRegistry() {
  const registry: HomePageRegistry = {
    push(fn) {
      homePages.push(fn as HomePageFn);
      homePageVersion.value += 1;
    },
  };
  (window as unknown as { __mmlHomePages?: HomePageRegistry }).__mmlHomePages = registry;
}

/**
 * 取自定义主页组件
 *
 * # 返回值
 *
 * 第一个非空组件；没有时返回 null，回落到 iframe / 内置主页
 */
export function resolveHomePage(): Component | null {
  void homePageVersion.value;
  for (const fn of homePages) {
    try {
      const page = fn();
      if (page) return page;
    } catch (err) {
      console.error("[custom-home] 主页回调失败", err);
    }
  }
  return null;
}

/** 加载 custom.js；失败也 resolve，回落到 iframe */
function loadHomeScript(url: string): Promise<void> {
  return new Promise((resolve) => {
    const script = document.createElement("script");
    script.id = "mml-custom-home-script";
    script.src = url;
    script.async = true;
    script.addEventListener("load", () => resolve());
    script.addEventListener("error", () => {
      console.warn("[custom-home] 入口脚本加载失败", url);
      resolve();
    });
    document.head.appendChild(script);
    homeScript = script;
  });
}

function ensureHomeStyle(url: string) {
  if (!url) return;
  const link = homeStyle ?? document.createElement("link");
  link.id = "mml-custom-home-style";
  link.rel = "stylesheet";
  if (link.getAttribute("href") !== url) link.setAttribute("href", url);
  if (!homeStyle) document.head.appendChild(link);
  homeStyle = link;
}

function teardownHome() {
  homeScript?.remove();
  homeScript = null;
  homeStyle?.remove();
  homeStyle = null;
  loadedScriptUrl = "";
  homePages.length = 0;
  homePageVersion.value += 1;
}

/** 按状态同步主页 custom.js / custom.css；挂载前 await，窗口首帧就是替换后的 */
export async function syncCustomHome() {
  const info = await commands.customHome.status().catch(() => null);
  if (!info?.enabled || !info.installed || !info.scriptUrl) {
    teardownHome();
    return;
  }
  ensureHostMml();
  if (loadedScriptUrl === info.scriptUrl) {
    ensureHomeStyle(info.styleUrl);
    return;
  }
  teardownHome();
  installHomeRegistry();
  await loadHomeScript(info.scriptUrl);
  loadedScriptUrl = info.scriptUrl;
  ensureHomeStyle(info.styleUrl);
  if (homePages.length > 0) homePageVersion.value += 1;
}

/** 启动时加载一次并监听变更；重复调用无副作用 */
export async function initCustomHome() {
  // 只有主窗口有主页内容区，别在设置 / 下载窗口白加载一份组件
  if (started || currentKind.value !== "main") return;
  started = true;
  installHomeRegistry();
  await syncCustomHome();
  void onCustomHomeChange(() => {
    void syncCustomHome();
  });
  void onClientConfigChange(() => {
    void syncCustomHome();
  });
}
