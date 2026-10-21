// 贴靠布局：把 M²L 自己的窗口按钮位置交给 tauri-plugin-decoration
//
// 背景（为什么需要这个文件）
// ------------------------------------------------------------------
// Windows 11 的「窗口位置选择」贴靠面板，靠的是系统在窗口上问一句话：
// "鼠标底下是不是最大化按钮？" 靠 `WM_NCHITTEST` → `HTMAXBUTTON` 回答。
// 用自绘标题栏的应用必须自己回答这个问题，否则悬停最大化按钮不会弹面板。
//
// 插件的做法是：在最大化按钮的位置放一个**原生子窗口**，由它把该消息答成
// HTMAXBUTTON。而这个子窗口的**位置**，原本是插件用它自己那套按钮量出来的 ——
// 想保留我们自己的按钮外观就撞上了两难：
//   - 不给它按钮 → 它量不出矩形 → 报错 → 原生 frame 摘不掉，系统标题栏、
//     插件按钮、我们的按钮三套并存，实测就是这样；
//   - 用它按钮 → 就得接受它的外观。
//
// 解法：给插件的本地 vendor 版加了一个「外部控件提供者」接口
// 见 mml-gui/vendor/patches/external-controls.patch 里的 `M²L 本地修改` 注释。
// 注册之后，插件不再要求自己那套按钮存在，直接用**我们给出的矩形**摆那个
// 原生命中区。外观完全由我们掌控。
//
// 坐标口径
// ------------------------------------------------------------------
// 插件内部吃的是**物理像素**，它自己做 CSS 像素 × devicePixelRatio，
// 所以这里也按物理像素上报，避免双重换算。

import { ref } from "vue";

/** 插件暴露的全局对象，与插件 js/titlebar.js 的 GLOBAL_NAME 一致 */
const DECORATION_GLOBAL = "__TAURI_PLUGIN_DECORATION__";

/**
 * 本窗口是否已把窗口按钮交给插件装饰
 *
 * 注册成功后置 true。给"插件会自己画按钮"的场景用 —— 目前只有 macOS：
 * 那边插件用的是**原生红黄绿**（不是 HTML 按钮），我们不该再画自己的圆点。
 * Windows 下插件不再画按钮（热区由本模块提供），这里置位只是状态记录。
 */
export const pluginDecorated = ref(false);

/** 贴靠热区矩形，单位物理像素，与插件内部口径一致 */
interface SnapRect {
  x: number;
  y: number;
  width: number;
  height: number;
  titlebarHeight: number;
  controlBandWidth: number;
}

/** 声明的外部提供者接口 */
interface DecorationRuntime {
  setExternalControlProvider?: (provider: (() => SnapRect | null) | null) => void;
  /** 插件转发的热区悬停变化：鼠标事件被原生命中区吃掉，DOM 收不到 */
  externalControlHoverListener?: ((hovered: boolean) => void) | null;
  /** 插件转发的热区点击 */
  externalControlClickListener?: (() => void) | null;
  /** 插件转发的热区右键 */
  externalControlRightClickListener?: (() => void) | null;
}

function decorationRuntime(): DecorationRuntime | null {
  const runtime = (window as unknown as Record<string, unknown>)[DECORATION_GLOBAL];
  return (runtime as DecorationRuntime) ?? null;
}

/**
 * 注册贴靠热区提供者
 *
 * 回调里**现查**当前窗口右上角最大化按钮的位置（每次插件要几何时都会调），
 * 所以窗口缩放 / 全屏切换 / 按钮重挂载后都会自动跟上，不需要手动失效。
 *
 * 用选择器现查、而不是绑定某个元素引用：主窗口启动时有一段启动画面
 * SplashScreen，标题栏要等启动完成才渲染出来；绑引用的话注册那一刻还拿不到按钮。
 *
 * - `root`: 在这些元素范围内查（一般是主窗口根节点）
 *
 * # 返回值
 *
 * 清理函数，组件卸载时调用
 */
export function registerSnapTarget(root: HTMLElement | null): () => void {
  const runtime = decorationRuntime();
  if (!runtime?.setExternalControlProvider) {
    // 插件没起来时什么都不做：浏览器预览，或插件版本没这个接口
    return () => { };
  }

  if (!root) {
    runtime.setExternalControlProvider?.(null);
    pluginDecorated.value = false;
    return () => { };
  }

  pluginDecorated.value = true;
  runtime.setExternalControlProvider(measure.bind(null, root));

  // 悬停态：原生命中区盖在 webview 上、吃掉了鼠标消息，我们自己的按钮
  // **收不到 DOM mouseenter**，所以 :hover 样式不会亮。改为听插件转发的事件，
  // 手动给按钮打一个标记类，由样式按这个类表现悬停，见 WindowControls.vue
  runtime.externalControlHoverListener = (hovered: boolean) => {
    const btn = findMaximizeBtn(root);
    if (!btn) return;
    btn.classList.toggle("snap-hover", hovered);
  };
  // 点击同理：鼠标事件到不了 DOM，按钮的 @click 也不会触发，改为听插件转发
  runtime.externalControlClickListener = () => {
    findMaximizeBtn(root)?.click();
  };
  // 右键：热区把 WM_NCRBUTTONUP 也吃掉了（见 vendor/patches/snap-right-click.patch），
  // 这里补一个合成事件，让按钮上的 @contextmenu 照常走
  runtime.externalControlRightClickListener = () => {
    const btn = findMaximizeBtn(root);
    if (!btn) return;
    const rect = btn.getBoundingClientRect();
    btn.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: Math.round(rect.left + rect.width / 2),
        clientY: Math.round(rect.bottom),
      }),
    );
  };

  return () => {
    runtime.setExternalControlProvider?.(null);
    runtime.externalControlHoverListener = null;
    runtime.externalControlClickListener = null;
    runtime.externalControlRightClickListener = null;
    findMaximizeBtn(root)?.classList.remove("snap-hover");
    pluginDecorated.value = false;
  };
}

/** 找当前窗口右上角的最大化按钮；windows 样式才有，macos 是左端原生圆点 */
function findMaximizeBtn(root: HTMLElement): HTMLElement | null {
  const btn = root.querySelector<HTMLElement>(".window-controls.windows .wc-btn.maximize");
  return btn?.isConnected ? btn : null;
}

/** 贴靠热区矩形，单位物理像素，与插件内部口径一致 */
function measure(root: HTMLElement): SnapRect | null {
  if (!root.isConnected) return null;

  const maximizeBtn = findMaximizeBtn(root);
  if (!maximizeBtn) return null;

  const rect = maximizeBtn.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  const scale = window.devicePixelRatio > 0 ? window.devicePixelRatio : 1;

  // 标题栏高度取按钮所在的那条标题栏；控件带宽度取**整组按钮**的宽度
  const bar = maximizeBtn.closest("header") ?? maximizeBtn.parentElement;
  const barRect = bar?.getBoundingClientRect() ?? rect;
  const bandEl = maximizeBtn.parentElement;
  const bandRect = bandEl?.getBoundingClientRect() ?? rect;

  const px = (v: number) => Math.round(v * scale);

  // 贴靠热区：**与最大化按钮逐像素重合**。
  //
  // 两个边界都是踩过坑换来的（都是"扩出去"导致的）：
  //
  // 1. 横向不能越过相邻按钮。热区会吃掉落在里面的鼠标消息、一律回答
  //    "这是最大化按钮"，即 WM_NCHITTEST → HTMAXBUTTON。曾把它扩到标题栏右缘
  //    （含关闭键那段），结果关闭键点不动、鼠标放在关闭键上却高亮最大化按钮。
  //
  // 2. 纵向也不能比按钮高。曾做成"整条标题栏高、贴顶"（y=0 h=64），
  //    结果面板位置随指针在热区内的位置漂移（上半显示在中间、下半跟随鼠标）。
  //
  // 这个矩形交给插件，用来摆那个原生命中区（子窗口）。
  //
  // 已知限制：贴靠面板弹出后**跟随鼠标**而非锚在按钮下方。原因是系统锚定时认的是
  // "顶层窗口有没有 caption 按钮"，而子窗口对系统只是一个碰巧答了 HTMAXBUTTON 的
  // 独立控件。试过让**父窗口**在 WM_NCHITTEST 里认领这块区域（Avalonia 的做法），
  // 但插件那套"无边框 + 子窗口冒充"的架构下，父窗口拿不到 caption 语义，
  // 结果是面板干脆不弹了 —— 已回退。要彻底解决需要让窗口保留 WS_CAPTION 并用
  // WM_NCCALCSIZE 抹掉非客户区，那是另一条更大的路。
  const out = {
    x: px(rect.left),
    y: px(rect.top),
    width: px(rect.width),
    height: px(rect.height),
    titlebarHeight: px(barRect.height),
    controlBandWidth: px(Math.max(bandRect.width, rect.width)),
  };

  return out;
}

/**
 * 等最大化按钮渲染出来
 *
 * **激活必须在按钮就绪之后**：插件在激活流程里就要量这个矩形，量不到会报
 * `kind: "disabled"`，那次投递会被当作"没有贴靠热区"而生效 —— 之后再补报
 * 也改不回那次已经完成的激活。实测原生 frame 没被摘掉，系统标题栏与自绘按钮并存。
 *
 * 主窗口启动时先显示启动画面，标题栏的 `.window-controls` 在 MainTopbar 里，
 * 要等核心 load 完成、切到主界面才渲染，所以必须等。
 *
 * - `root`: 在这些元素范围内查
 * - `timeoutMs`: 超时上限；到点还没出现就放弃等待，让激活带着无热区继续，至少能起界面
 *
 * # 返回值
 *
 * 按钮出现（或超时）时 resolve
 */
export function waitForSnapTarget(root: HTMLElement | null, timeoutMs = 15000): Promise<void> {
  return new Promise((resolve) => {
    if (!root) {
      resolve();
      return;
    }
    if (measure(root)) {
      resolve();
      return;
    }

    const deadline = Date.now() + timeoutMs;
    let observer: ResizeObserver | null = null;
    let timer = 0;

    const finish = () => {
      observer?.disconnect();
      observer = null;
      if (timer) {
        window.clearInterval(timer);
        timer = 0;
      }
      resolve();
    };

    const check = () => {
      if (measure(root)) {
        finish();
        return;
      }
      if (Date.now() >= deadline) {
        // 超时不致命：激活会带着"无热区"继续，界面照常起来，只是没有贴靠面板
        console.warn("[decoration] 等待最大化按钮超时，本次不带贴靠热区");
        finish();
      }
    };

    if (typeof ResizeObserver === "function") {
      observer = new ResizeObserver(check);
      observer.observe(root);
    }
    // ResizeObserver 在"子树整体替换但 root 尺寸没变"时不会回调，启动画面 → 主界面
    // 恰好可能如此，用轮询兜底；两者任一先到即可
    timer = window.setInterval(check, 100);
  });
}
