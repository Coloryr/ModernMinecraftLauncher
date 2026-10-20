// 自定义界面契约：宿主注入的注册表与入口上下文
//
// 这些类型不依赖 Vue 或 DOM 运行时，组件本身用 unknown 表示，由宿主按自己的组件类型收窄。

/** 窗口页面注册表：整页包的 custom.js 通过它登记回调 */
export interface WindowPageRegistry {
  /**
   * 注册回调
   *
   * - `fn`: 返回组件就替换该窗口，返回 null 走内置页面
   */
  push(fn: (kind: string) => unknown | null): void;
}

/** 主页内容注册表：自定义主页包的 custom.js 通过它登记回调 */
export interface HomePageRegistry {
  /**
   * 注册回调
   *
   * - `fn`: 返回组件就替换内置主页内容区，返回 null 回落到 iframe / 内置主页
   */
  push(fn: () => unknown | null): void;
}

/** 替换控件的参数 */
export interface CustomCssReplaceOptions {
  /** 新文字；不给时保持原文字 */
  text?: string;
  /** 追加的类名 */
  className?: string;
  /** 直接写进元素 style 的属性 */
  style?: Partial<CSSStyleDeclaration>;
  /** 替换组件；给出时隐藏原控件并挂载它 */
  component?: unknown;
  /** 传给替换组件的 props */
  props?: Record<string, unknown>;
}

/** mml-vue 传给自定义 CSS 入口脚本的上下文 */
export interface CustomCssContext {
  /** 包 id */
  id: string;
  /** 确保 custom.css 已挂上 */
  loadCss: () => void;
  /** 选第一个匹配的元素 */
  select: <T extends Element = HTMLElement>(selector: string) => T | null;
  /** 替换控件：component 给出时挂 Vue 组件，否则改文字 / 类名 */
  replace: (target: string | Element, options: CustomCssReplaceOptions) => Element | null;
  /** 注册清理函数 */
  onCleanup: (fn: () => void) => void;
}
