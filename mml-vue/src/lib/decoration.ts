// 贴靠布局：把本窗口最大化按钮的矩形交给 tauri-plugin-decoration
//
// Windows 11 的贴靠面板靠窗口回答 `WM_NCHITTEST → HTMAXBUTTON`，即"鼠标是否在
// 最大化按钮上"。插件放一个原生命中子窗口来回答，它的位置原本取自插件自己的按钮；
// 要保留自绘外观，就走本地 vendor 版加的「外部控件提供者」接口直接给矩形
// 见 mml-gui/vendor/patches/tauri-plugin-decoration.patch 里的 `M²L 本地修改`。
// 不注册时插件量不到按钮，激活会退回原生 frame，系统标题栏与自绘按钮并存。
//
// 坐标一律物理像素：插件内部自己做 CSS 像素 × devicePixelRatio，这里再乘一次
// 就是双重换算。
//
// 热区必须与最大化按钮逐像素重合：横向越过相邻按钮会让关闭键点不动，纵向高过
// 按钮会让贴靠面板随指针位置漂移。已知面板跟随鼠标而非锚定按钮下方：子窗口对系统
// 只是一个答了 HTMAXBUTTON 的独立控件，不是顶层窗口的 caption 按钮；让父窗口认领
// 这块区域则面板不弹。

import { ref } from "vue";

/** 插件暴露的全局对象，与插件 js/titlebar.js 的 GLOBAL_NAME 一致 */
const DECORATION_GLOBAL = "__TAURI_PLUGIN_DECORATION__";

/**
 * 本窗口的装饰是否已由插件接管
 *
 * 只在激活命令返回 `custom` 后置 true；注册热区本身不代表插件接过了标题栏。
 */
export const pluginDecorated = ref(false);

/** 贴靠热区几何，单位物理像素，与插件内部口径一致 */
interface SnapRect {
  /** 左上角横坐标，单位物理像素 */
  x: number;
  /** 左上角纵坐标，单位物理像素 */
  y: number;
  /** 宽度，单位物理像素 */
  width: number;
  /** 高度，单位物理像素 */
  height: number;
  /** 标题栏高度，单位物理像素 */
  titlebarHeight: number;
  /** 窗口按钮整组宽度，单位物理像素 */
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

/** 取插件挂在 window 上的运行时对象；插件没装时为 null */
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
  // 右键：热区把 WM_NCRBUTTONUP 也吃掉了（见 vendor/patches/tauri-plugin-decoration.patch），
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

/**
 * 量最大化按钮，得到插件要的贴靠热区
 *
 * - `root`: 在这些元素范围内查
 *
 * # 返回值
 *
 * 物理像素的矩形；按钮不存在、不可见或已卸载时为 null
 */
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

  // 热区与按钮逐像素重合，边界别外扩，原因见文件头
  return {
    x: px(rect.left),
    y: px(rect.top),
    width: px(rect.width),
    height: px(rect.height),
    titlebarHeight: px(barRect.height),
    controlBandWidth: px(Math.max(bandRect.width, rect.width)),
  };
}

/**
 * 等最大化按钮渲染出来
 *
 * **激活必须在按钮就绪之后**：插件在激活流程里就要量这个矩形，量不到会以
 * `kind: "disabled"` 完成这次激活，之后再补报也改不回 —— 实测原生 frame 没被摘掉，
 * 系统标题栏与自绘按钮并存。
 *
 * 主窗口启动时先显示启动画面，标题栏的 `.window-controls` 在 MainTopbar 里，
 * 要等核心 load 完成、切到主界面才渲染，所以必须等。
 *
 * - `root`: 在这些元素范围内查
 * - `timeoutMs`: 超时上限，单位毫秒，缺省 15 秒；到点还没出现就放弃等待，让激活带着无热区继续，至少能起界面
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
