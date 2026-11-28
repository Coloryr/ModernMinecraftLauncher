// 自定义主页面契约：启动器注入的 window.mml
//
// 桥接对象由启动器建立（Rust 侧 custom_home_bridge.js + 前端 customHomeBridge.ts）；
// 命令、事件名与事件 payload 都复用包内生成物。本文件手写，不在 Rust 源码扫描范围内。

import { createCommands } from "./bindings";
import { GameExit, GameLog, InstanceChange, LaunchError, LaunchState, type EventPayloads } from "./listens";

/** 主题：启动器把「跟随系统」解析成这两档后下发 */
export type MmlTheme = "Dark" | "Light";

/**
 * 主题变量下发（父 → iframe）
 *
 * iframe 是独立文档，拿不到宿主 `<html>` 上的 CSS 变量。启动器在握手后、以及主题 /
 * 强调色 / 纯色模式变化时发这一条，页面把它写进自己的 `:root`，就能直接用
 * `mml-vue/tailwind.css` 那套 token。整页 iframe 主题（`custom_ui/page.zip`）
 * 走同一个桥接，消息相同。
 *
 * 页面侧监听见 custom/README.md「iframe 主题拿主题色」。
 */
export interface MmlThemeTokens {
  /** 协议标记，与其它桥接消息一致 */
  __mml: 1;
  /** 消息类型 */
  type: "theme-tokens";
  /** 当前实际主题，与 `window.mml.theme` 同源 */
  theme: MmlTheme;
  /** 变量名 → 计算值；为空的不下发，页面按 key 取即可 */
  tokens: Record<string, string>;
}

/** iframe → 父：主动要一次主题变量，父窗口回 {@link MmlThemeTokens} */
export interface MmlThemeTokensRequest {
  /** 协议标记，与其它桥接消息一致 */
  __mml: 1;
  /** 消息类型 */
  type: "theme-tokens?";
}

/** 白名单事件与 payload：从生成的 EventPayloads 里挑，与 mml-vue 的 customHomeBridge.ts 一致 */
export type MmlHomeEvents = Pick<
  EventPayloads,
  typeof LaunchState | typeof LaunchError | typeof GameExit | typeof GameLog | typeof InstanceChange
>;

/** 自定义主页可订阅的事件名 */
export type MmlHomeEventName = keyof MmlHomeEvents;

/** 启动器注入的桥接对象 window.mml */
export interface MmlApi {
  /**
   * 调用启动器命令
   *
   * - cmd: 命令名，即 Rust 函数名，如 main_get_instances
   * - args: 参数对象，键名与 Rust 参数一致（camelCase）
   *
   * # 返回值
   *
   * 命令结果；命令不存在或执行失败时 reject
   */
  invoke<T = unknown>(cmd: string, args?: Record<string, unknown> | null): Promise<T>;

  /**
   * 订阅白名单事件，返回取消订阅函数
   *
   * - event: 事件名，取 listens.ts 的常量或 MmlHomeEventName
   * - cb: 收到 payload 时回调
   *
   * # 返回值
   *
   * 取消订阅函数，页面 / 组件卸载时必须调用
   */
  on<K extends MmlHomeEventName>(event: K, cb: (payload: MmlHomeEvents[K]) => void): () => void;
  /** 事件名是运行时字符串时的兜底重载，payload 无类型 */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  on(event: string, cb: (payload: any) => void): () => void;

  /**
   * 订阅主题变化，返回取消订阅函数
   *
   * 注册后不会立刻回调一次；当前值读 theme。
   */
  onTheme(cb: (theme: MmlTheme) => void): () => void;

  /**
   * 通过宿主导航打开工具页面，自动遵循单窗口 / 多窗口设置。
   * instance 为资源、日志、配置文件等实例窗口的目标 uuid。
   * 仅直接运行在宿主内的组件可用；iframe 与旧版宿主不提供此方法。
   */
  openWindow?(kind: string, instance?: string | null): Promise<void>;

  /**
   * 读取自定义存储
   *
   * 内容来自运行目录的 `.custom_config`，原样返回；文件不存在时是空串。
   */
  read_custom_config(): Promise<string>;

  /** `read_custom_config` 的兼容拼写 */
  read_cutsom_config(): Promise<string>;

  /**
   * 写入自定义存储
   *
   * - `data`: 要保存的字符串，原样覆盖 `.custom_config`，启动器不校验内容格式
   */
  write_custom_config(data: string): Promise<void>;

  /** 当前主题（启动器下发，实时更新） */
  theme: MmlTheme;

  /**
   * 与启动器父窗口的握手
   *
   * 不在启动器里打开时约 10 秒后才 resolve，且之后的 invoke 永远不会有应答；
   * 不要拿它判断是否在启动器里。
   */
  ready: Promise<void>;
}

/** 用 window.mml 建出的类型化命令表 */
export type HostCommands = ReturnType<typeof createCommands>;

/** 已建立的命令表；成功建立后复用，避免每个组件各建一份 */
let hostCommands: HostCommands | null = null;

/**
 * 取宿主命令表
 *
 * 首次调用时用 `window.mml.invoke` 建立，之后复用同一份。
 *
 * # 返回值
 *
 * 见 [`HostCommands`]；浏览器预览等没有 `window.mml` 的环境返回 null
 */
export function getCommands(): HostCommands | null {
  if (hostCommands) return hostCommands;
  if (typeof window === "undefined" || !window.mml) return null;
  hostCommands = createCommands(window.mml.invoke.bind(window.mml));
  return hostCommands;
}

declare global {
  interface Window {
    /** 不在启动器里打开时不存在 */
    mml?: MmlApi;
  }
}
