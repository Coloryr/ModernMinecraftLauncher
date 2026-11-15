// 贴靠热区：把自绘最大化按钮的矩形交给 tauri-plugin-decoration
//
// 插件在 Windows 上用原生命中区回答 WM_NCHITTEST→HTMAXBUTTON，位置取自它自己的按钮；
// 本地 vendor 版增加了「外部控件提供者」接口，让我们保留自绘按钮也能有贴靠面板。
// 不注册时插件量不到按钮，激活会以 disabled 结束、原生 frame 不摘除，
// 表现就是系统标题栏与自绘标题栏并存。
//
// 坐标一律物理像素：插件内部自己做 CSS 像素 × devicePixelRatio。

/** 插件挂在 window 上的运行时对象 */
const DECORATION_GLOBAL = "__TAURI_PLUGIN_DECORATION__";

/** 贴靠热区几何，单位物理像素 */
interface SnapRect {
  /** 左上角横坐标 */
  x: number;
  /** 左上角纵坐标 */
  y: number;
  /** 宽度 */
  width: number;
  /** 高度 */
  height: number;
  /** 标题栏高度 */
  titlebarHeight: number;
  /** 窗口按钮整组宽度 */
  controlBandWidth: number;
}

/** 插件声明的外部提供者接口 */
interface DecorationRuntime {
  setExternalControlProvider?: (provider: (() => SnapRect | null) | null) => void;
  /** 热区悬停变化：鼠标事件被原生命中区吃掉，DOM 收不到 */
  externalControlHoverListener?: ((hovered: boolean) => void) | null;
  /** 热区点击 */
  externalControlClickListener?: (() => void) | null;
}

/** 取插件运行时；插件没起来时为 null */
function decorationRuntime(): DecorationRuntime | null {
  const runtime = (window as unknown as Record<string, unknown>)[DECORATION_GLOBAL];
  return (runtime as DecorationRuntime) ?? null;
}

/** 找标题栏里的最大化按钮 */
function findMaximize(root: HTMLElement): HTMLElement | null {
  const button = root.querySelector<HTMLElement>('[data-window-control="maximize"]');
  return button?.isConnected ? button : null;
}

/**
 * 量最大化按钮，得到插件要的贴靠热区
 *
 * - `root`: 在这些元素范围内查，一般是标题栏
 *
 * # 返回值
 *
 * 物理像素的矩形；按钮不存在、不可见或已卸载时为 null
 */
function measure(root: HTMLElement): SnapRect | null {
  if (!root.isConnected) return null;
  const maximize = findMaximize(root);
  if (!maximize) return null;

  const rect = maximize.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  const scale = window.devicePixelRatio > 0 ? window.devicePixelRatio : 1;
  const bar = maximize.closest(".titlebar") ?? root;
  const barRect = bar.getBoundingClientRect();
  const band = maximize.closest(".window-controls") ?? maximize;
  const bandRect = band.getBoundingClientRect();
  const px = (value: number) => Math.round(value * scale);

  // 热区与按钮逐像素重合：越界会让相邻按钮点不动或贴靠面板漂移
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
 * 注册贴靠热区提供者
 *
 * 必须在激活装饰之前调用：插件在激活流程里就要量这块矩形。
 *
 * - `root`: 标题栏元素
 *
 * # 返回值
 *
 * 清理函数，组件卸载时调用
 */
export function registerSnapTarget(root: HTMLElement | null): () => void {
  const runtime = decorationRuntime();
  if (!runtime?.setExternalControlProvider || !root) return () => {};

  runtime.setExternalControlProvider(measure.bind(null, root));
  // 热区盖住最大化按钮后，DOM 收不到悬停与点击，改由插件转发
  runtime.externalControlHoverListener = (hovered: boolean) => {
    findMaximize(root)?.classList.toggle("is-snap-hover", hovered);
  };
  runtime.externalControlClickListener = () => {
    findMaximize(root)?.click();
  };

  return () => {
    runtime.setExternalControlProvider?.(null);
    runtime.externalControlHoverListener = null;
    runtime.externalControlClickListener = null;
    findMaximize(root)?.classList.remove("is-snap-hover");
  };
}
