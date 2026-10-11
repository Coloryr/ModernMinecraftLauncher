// 应用入口：禁用默认右键菜单，加载 GUI 配置后挂载根组件
import { createApp } from "vue";
import "./styles/index.css";
import App from "./App.vue";
import { loadGuiConfig } from "./lib/guiConfig";
import { applyTheme, theme } from "./lib/theme";
import { applyLocale, locale } from "./lib/i18n";
import { loadAccounts } from "./lib/accountStore";
import {
  restoreCapeConfig,
  restoreHeadConfig,
  restoreSelectedInstance,
  restoreSkinConfig,
  sidebarCollapsed,
  sidebarSide,
  viewMode,
} from "./lib/settings";
import { restoreBg } from "./lib/appearance";
import { setMultiWindow } from "./windows/windowManager";
import { restoreFontFamily } from "./lib/fonts";
import { installImageFallback } from "./lib/imageFallback";
import { vTip } from "./lib/tip";

// 图片加载失败的统一兜底：装上要趁早，免得启动过程中先加载的那批图漏掉
installImageFallback();

// 禁用右键默认菜单，包括 WebView2 / 浏览器自带的“刷新、返回、打印”等。
// 文本输入框 input / textarea / contenteditable 保留原生菜单，方便复制粘贴。
document.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement | null;
  if (
    target &&
    (target.closest("input") ||
      target.closest("textarea") ||
      target.closest("[contenteditable]"))
  ) {
    return;
  }
  e.preventDefault();
});

// 发布构建：拦掉 WebView2 自带的浏览器功能键，即 F5 / Ctrl+R 重载页面、
// F12 / Ctrl+Shift+C 开发者工具、Ctrl+P 打印。
//
// 这是**兜底**：正经做法是 WebView2 的设置项，见 src-tauri/src/windows/webview_keys.rs 的
// AreBrowserAcceleratorKeysEnabled / AreDevToolsEnabled。万一那两项没生效，即老版本
// WebView2 没有 Settings3，页面这一层还能挡住。开发构建 vite dev / tauri dev 不拦，
// 调试要用。
//
// 注意只拦"浏览器专有"的键：Ctrl+C / Ctrl+V / Ctrl+A / Ctrl+Z 这些编辑键一概不动。
if (import.meta.env.PROD) {
  window.addEventListener("keydown", (e) => {
    const ctrl = e.ctrlKey || e.metaKey;
    const blocked =
      e.key === "F5" ||
      e.key === "F12" ||
      (ctrl && ["r", "R", "p", "P"].includes(e.key)) ||
      (ctrl && e.shiftKey && ["c", "C", "r", "R"].includes(e.key));
    if (blocked) {
      e.preventDefault();
    }
  });
}

/** 启动引导：GUI 状态由 Rust 的 gui_config.json 提供，浏览器回退本地存储 */
async function bootstrap() {
  const cfg = await loadGuiConfig();
  if (cfg) {
    theme.value = cfg.theme;
    locale.value = cfg.locale;
    sidebarSide.value = cfg.mainWindow.sidebarSide;
    sidebarCollapsed.value = cfg.mainWindow.sidebarCollapsed;
    viewMode.value = cfg.mainWindow.viewMode;
    restoreSelectedInstance(cfg.mainWindow.selectedInstance);
    setMultiWindow(cfg.windowMode !== "Single");
    // 字体 / 皮肤与头像显示模式：只同步内存状态，不回写配置
    restoreFontFamily(cfg.font);
    restoreSkinConfig(cfg.skin.display, cfg.skin.pitch, cfg.skin.yaw);
    restoreCapeConfig(cfg.cape.display, cfg.cape.pitch, cfg.cape.yaw);
    restoreHeadConfig(cfg.head.headType, cfg.head.x, cfg.head.y);
  }

  // 背景图：不阻塞挂载 —— 大图走 base64 IPC 很慢，挂着等它 = 黑屏一阵。
  // 正常启动时背景 dataURL 已从本地存储秒取，无闪底色；
  // 只有首次设置背景的那一次会先出界面、背景随后淡入
  void restoreBg();

  applyTheme();
  applyLocale();
  loadAccounts(); // 账户列表由 Rust 提供（浏览器环境静默跳过）
  createApp(App).directive("tip", vTip).mount("#app");
}

bootstrap();
