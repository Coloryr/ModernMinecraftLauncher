// 桥接使用小工具：宿主探测、带超时的 invoke、类型化命令与启动阶段文案
//
// 这一层不是桥接的一部分，只是把示例页里容易踩的坑封装好：
// - `window.mml` 只在启动器里存在；
// - 没有宿主时 invoke 永远不会有应答（不 reject 也不 resolve），必须自己加超时；
// - `mml.ready` 在没有宿主时也会 resolve（约 10 秒后），不能当宿主检测用。
//
// 命令、事件名、payload 与 `window.mml` 的类型都来自 `mml-ipc`，本文件只留使用封装。

import {
  createCommands,
  createEvents,
  type Invoke,
  type MmlApi,
  type MmlTheme,
} from "mml-ipc";

/** 拿桥接对象（不在启动器里时为 undefined） */
export function getMml(): MmlApi | undefined {
  return window.mml;
}

/** 带超时的 invoke 超时抛出的错误，用来区分「没人应答」与「命令自己报错」 */
export class TimeoutError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TimeoutError";
  }
}

/** 给 Promise 加超时（超时抛 TimeoutError） */
export function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new TimeoutError(`${what} 超时（${ms}ms 内没有应答）`)), ms);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (err: unknown) => {
        clearTimeout(timer);
        reject(err);
      },
    );
  });
}

/** 调启动器命令（默认 10 秒超时，避免页面卡在「转圈」上） */
export function invoke<T>(cmd: string, args?: Record<string, unknown>, ms = 10000): Promise<T> {
  const mml = getMml();
  if (!mml) return Promise.reject(new Error("当前不在启动器中打开，window.mml 不可用"));
  return withTimeout(mml.invoke<T>(cmd, args ?? null), ms, `命令 ${cmd}`);
}

// 命令包装的传输层：命令名与参数类型由 mml-ipc 提供，超时仍走上面的 invoke
const transport: Invoke = (cmd, args) => invoke(cmd, args ?? undefined);

/**
 * 类型化命令表，按启动器窗口分组
 *
 * 用法与启动器自己的前端一致，如 `commands.main.getInstances()`、
 * `commands.main.launchGame(uuid, null, null, null)`；传错参数或缺参数会在编译期报错。
 */
export const commands = createCommands(transport);

/** 探测宿主：页面是否真的跑在启动器里 */
export async function probeHost(ms = 1500): Promise<boolean> {
  const mml = getMml();
  if (!mml) return false;
  try {
    // 拿一条最轻的命令当探针：有应答（成功或报错都算）就说明父窗口在。
    // 用 main_load_state 是因为它不碰磁盘也不启动任何东西。
    await withTimeout(mml.invoke("main_load_state"), ms, "握手");
    return true;
  } catch (err) {
    // 超时 = 父窗口没应答；其它错误 = 命令自己失败，但父窗口确实在
    return !(err instanceof TimeoutError);
  }
}

/** 当前主题（不在启动器里时按系统偏好给个值，方便浏览器预览） */
export function currentTheme(): MmlTheme {
  const t = getMml()?.theme;
  if (t === "Light" || t === "Dark") return t;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "Light" : "Dark";
}

/** 订阅主题变化（不在启动器里时返回空函数；浏览器预览可自行监听 prefers-color-scheme） */
export function onTheme(cb: (theme: MmlTheme) => void): () => void {
  return getMml()?.onTheme(cb) ?? (() => { });
}

/**
 * 启动阶段文案
 *
 * state 取值与启动器一致；表里没有的一律原样显示，方便后端加新阶段时不至于空着。
 */
export const STAGE_LABELS: Record<string, string> = {
  launching: "正在启动",
  login: "登录账户",
  check: "检查游戏文件",
  readinfo: "读取版本信息",
  download: "下载缺失文件",
  jvm: "准备启动参数",
  pre: "执行启动前命令",
  post: "执行启动后命令",
  end: "启动完成",
  loadserverpack: "加载服务器包",
  checkserverpack: "检查服务器包",
  downloadserverpack: "下载服务器包",
};

// ---- 类型化事件订阅（启动器白名单事件，见 README） ----
// 事件名与 payload 类型由 mml-ipc 生成；这里只把 window.mml.on 接成生成函数要的传输层。

/** 订阅启动阶段（launch-state）/ 游戏退出（game-exit）/ 启动失败（launch-error） */
export const { onLaunchState, onGameExit, onLaunchError } = createEvents(
  (event, cb) => getMml()?.on(event, cb) ?? (() => { }),
);
