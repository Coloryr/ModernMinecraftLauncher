// 自定义主页面桥接（父窗口侧）
//
// iframe 里的 `window.mml` 由 Rust 侧注入的 `custom_home_bridge.js` 提供，通过 postMessage
// 把 invoke / 订阅请求发到主窗口，这里负责校验来源、转发命令、按白名单转发事件。
//
// 消息格式，与 custom_home_bridge.js 严格对应：
//   iframe → 父：{ __mml: 1, id, type: "invoke", cmd, args }
//   iframe → 父：{ __mml: 1, id, type: "subscribe", event }
//   iframe → 父：{ __mml: 1, type: "unsubscribe", id }
//   iframe → 父：{ __mml: 1, type: "ready" }，握手；父窗口回 ready + 当前主题
//   父 → iframe：{ __mml: 1, id, ok: true, data } / { __mml: 1, id, ok: false, error }
//   父 → iframe：{ __mml: 1, type: "event", id, event, payload }
//   父 → iframe：{ __mml: 1, type: "ready", theme: "Dark" | "Light" }
//   父 → iframe：{ __mml: 1, type: "theme", theme }（主题变化时主动推）
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { watch } from "vue";
import { GameExit, GameLog, InstanceChange, LaunchError, LaunchState } from "./listens";
import { resolvedTheme } from "./theme";

/** 与 custom_home_bridge.js 约定的协议版本 */
const PROTOCOL = 1;

/**
 * 事件白名单（一处，要扩就改这里）
 *
 * 常量来自 `listens.ts`（值即 wire 事件名）：启动状态 / 启动失败 / 游戏退出 / 游戏日志 / 实例变更。
 */
const ALLOWED_EVENTS: ReadonlySet<string> = new Set<string>([
  LaunchState,
  LaunchError,
  GameExit,
  GameLog,
  InstanceChange,
]);

/**
 * 事件转发的攒批窗口（ms）
 *
 * `game-log` 是流式的且量大，每条都立刻 postMessage 会是「父→iframe」的双跳开销；
 * 攒一个窗口再逐条发出，既限住消息速率又不丢事件、不改 payload 形状。
 */
const FLUSH_INTERVAL = 50;

/** iframe → 父 的消息，字段都可能缺，全部当 unknown 处理 */
interface BridgeRequest {
  __mml: number;
  type?: string;
  id?: number;
  cmd?: unknown;
  args?: unknown;
  event?: unknown;
}

/**
 * 给一个自定义主页面的 iframe 挂上桥接
 *
 * 只认 `event.source === iframe.contentWindow` 且带协议标记的消息，其余一律忽略。
 *
 * - `iframe`: 目标 iframe
 *
 * # 返回值
 *
 * 解绑函数，组件卸载时调用：移除监听、清掉攒批、取消全部事件订阅
 */
export function attachCustomHomeBridge(iframe: HTMLIFrameElement): () => void {
  /** 订阅 id → 事件名 */
  const subscriptions = new Map<number, string>();
  /** 事件名 → 已订阅的 id 集合：同一事件多个 id 订阅时只 listen 一次，多路分发 */
  const byEvent = new Map<string, Set<number>>();
  /** 事件名 → 已就绪的底层 listen 取消函数 */
  const unlistens = new Map<string, UnlistenFn>();
  /** 事件名 → 正在建立的 listen，避免同一事件并发 listen 两次 */
  const pendingListens = new Map<string, Promise<UnlistenFn>>();

  /** 待转发的事件（攒批） */
  let queue: Array<{ id: number; event: string; payload: unknown }> = [];
  let flushTimer: number | null = null;
  let disposed = false;
  /** 是否已经完成握手：iframe 的桥接脚本会一直重复发 ready，直到收到应答 */
  let handshaken = false;

  /** 回发到 iframe；opaque origin 拿不到确切 origin，只能 targetOrigin "*" */
  function post(msg: Record<string, unknown>) {
    if (disposed) return;
    const win = iframe.contentWindow;
    if (!win) return;
    win.postMessage({ __mml: PROTOCOL, ...msg }, "*");
  }

  function flush() {
    flushTimer = null;
    if (queue.length === 0) return;
    const batch = queue;
    queue = [];
    for (const item of batch) {
      post({ type: "event", id: item.id, event: item.event, payload: item.payload });
    }
  }

  function pushEvent(id: number, event: string, payload: unknown) {
    queue.push({ id, event, payload });
    if (flushTimer === null) flushTimer = window.setTimeout(flush, FLUSH_INTERVAL);
  }

  /** 订阅：白名单外一律拒绝，回一条错误应答让页面自己知道 */
  function subscribe(id: number, event: string) {
    if (!ALLOWED_EVENTS.has(event)) {
      post({ id, ok: false, error: `event not allowed: ${event}` });
      return;
    }
    subscriptions.set(id, event);
    let ids = byEvent.get(event);
    if (!ids) {
      ids = new Set<number>();
      byEvent.set(event, ids);
    }
    ids.add(id);
    if (pendingListens.has(event)) return;

    const pending: Promise<UnlistenFn> = listen(event, (e) => {
      // 原始 payload 直接转发，不走 api.ts 那些带类型转换的包装
      for (const sub of byEvent.get(event) ?? []) {
        pushEvent(sub, event, e.payload);
      }
    }).then((un) => {
      // 建立期间订阅被取消、或同一事件已经重新 listen 了一次：解绑这份多余的
      if (pendingListens.get(event) === pending && byEvent.has(event)) unlistens.set(event, un);
      else un();
      return un;
    });
    pendingListens.set(event, pending);
  }

  /** 取消一个订阅；该事件没人订了就解绑底层监听 */
  function unsubscribe(id: number) {
    const event = subscriptions.get(id);
    if (event === undefined) return;
    subscriptions.delete(id);
    const ids = byEvent.get(event);
    ids?.delete(id);
    if (!ids || ids.size > 0) return;
    byEvent.delete(event);
    pendingListens.delete(event);
    const un = unlistens.get(event);
    unlistens.delete(event);
    un?.();
  }

  /** 调命令：放行全部已注册命令，Tauri 对未注册命令会自行报错，错误串原样回给页面 */
  async function handleInvoke(id: number, cmd: unknown, args: unknown) {
    // 握手还没完成就来的 invoke：说明 iframe 侧已经不是当前这份脚本（重载 / 换页），
    // 立刻回一条错误，别让它永远挂在那里
    if (!handshaken) {
      post({ id, ok: false, error: "mml bridge: handshake not completed" });
      return;
    }
    try {
      const data = await invoke(String(cmd), (args ?? {}) as Record<string, unknown>);
      post({ id, ok: true, data });
    } catch (err) {
      post({ id, ok: false, error: String(err) });
    }
  }

  function onMessage(e: MessageEvent) {
    if (disposed) return;
    // 只认这个 iframe 发来的、带协议标记的消息
    if (e.source !== iframe.contentWindow) return;
    const data = e.data as BridgeRequest | null | undefined;
    if (!data || typeof data !== "object" || data.__mml !== PROTOCOL) return;

    // 握手：回一条 ready（带当前主题），iframe 据此 resolve window.mml.ready 并应用主题。
    // iframe 侧在收到应答前会每 100ms 重发一次 ready，所以这里只管每次都回，不去重。
    // 重发是它发现自己没收到应答时的重试，压制它会让页面一直卡在握手前
    if (data.type === "ready") {
      handshaken = true;
      post({ type: "ready", theme: resolvedTheme() });
      return;
    }
    const id = Number(data.id);
    if (!Number.isFinite(id)) return;
    if (data.type === "invoke") {
      void handleInvoke(id, data.cmd, data.args);
    } else if (data.type === "subscribe") {
      subscribe(id, String(data.event));
    } else if (data.type === "unsubscribe") {
      unsubscribe(id);
    }
  }

  window.addEventListener("message", onMessage);

  // 主题：iframe 是独立文档，拿不到启动器 `<html data-theme>` 上的 CSS 变量，
  // 所以握手时先回一份，之后主题变化时再主动推，设置窗口切换或 System 跟随系统都算。
  // 注册 watch 时不会立刻触发（没有 immediate），首屏由握手那条负责，不会重复发。
  const stopThemeWatch = watch(resolvedTheme, (v) => {
    post({ type: "theme", theme: v });
  });

  return () => {
    disposed = true;
    stopThemeWatch();
    window.removeEventListener("message", onMessage);
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    queue = [];
    for (const un of unlistens.values()) un();
    unlistens.clear();
    pendingListens.clear();
    subscriptions.clear();
    byEvent.clear();
  };
}
