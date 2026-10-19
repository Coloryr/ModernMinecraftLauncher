// window.mml 使用封装：宿主探测、带超时的 invoke、类型化命令与事件
//
// 命令、事件名与 payload 类型都来自 mml-ipc；本文件只留使用封装。
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
    const timer = setTimeout(() => reject(new TimeoutError(what + " 超时（" + ms + "ms 内没有应答）")), ms);
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
  return withTimeout(mml.invoke<T>(cmd, args ?? null), ms, "命令 " + cmd);
}

/** 命令包装的传输层：类型由 mml-ipc 提供 */
const transport: Invoke = (cmd, args) => invoke(cmd, args ?? undefined);

/** 类型化命令表，按启动器窗口分组 */
export const commands = createCommands(transport);

/** 事件订阅的传输层：不在启动器里时返回空取消函数 */
export const { onLaunchState, onGameExit, onLaunchError, onInstanceChange } = createEvents(
  (event, cb) => getMml()?.on(event, cb) ?? (() => { }),
);

/** 探测宿主：页面是否真的跑在启动器里 */
export async function probeHost(ms = 1500): Promise<boolean> {
  const mml = getMml();
  if (!mml) return false;
  try {
    await withTimeout(mml.invoke("main_load_state"), ms, "握手");
    return true;
  } catch (err) {
    return !(err instanceof TimeoutError);
  }
}

/** 当前主题（不在启动器里时按系统偏好给个值） */
export function currentTheme(): MmlTheme {
  const t = getMml()?.theme;
  if (t === "Light" || t === "Dark") return t;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "Light" : "Dark";
}
