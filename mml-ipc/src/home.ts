// 自定义主页面契约：启动器注入的 window.mml
//
// 桥接对象由启动器建立（Rust 侧 custom_home_bridge.js + 前端 customHomeBridge.ts）；
// 命令、事件名与事件 payload 都复用包内生成物。本文件手写，不在 Rust 源码扫描范围内。

import { GameExit, GameLog, InstanceChange, LaunchError, LaunchState, type EventPayloads } from "./listens";

/** 主题：启动器把「跟随系统」解析成这两档后下发 */
export type MmlTheme = "Dark" | "Light";

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

declare global {
  interface Window {
    /** 不在启动器里打开时不存在 */
    mml?: MmlApi;
  }
}
