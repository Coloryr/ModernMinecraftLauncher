// 前端错误写进 logs.log
//
// 用户反馈问题贴的是 logs.log，webview 控制台里的东西他们看不到；这里把错误转给 Rust
// 落日志（命令 `window_log_frontend`）。只收错误级别：`console.error`、未捕获异常、
// 未处理的 Promise 拒绝。warn / info 不转，免得日志被普通输出刷满。
import { commands } from "./bindings";
import { isTauri } from "./util";

/** 与 windowManager 的 WIN_PARAM 同值：那边会拖进一大堆模块，这里只读一个查询串 */
const WIN_PARAM = "window";
/** 单条错误最多带多少字符：堆栈很长时别把日志撑爆 */
const MAX_LEN = 4000;
/** 同一条错误的去重窗口，单位毫秒 */
const DEDUPE_MS = 2000;
/** 去重表最多记多少条 */
const DEDUPE_MAX = 200;

let installed = false;
/** 正在转发：转发链自己出错时不能再走 console.error，否则递归 */
let forwarding = false;
/** 最近出现过的错误文本 → 时间戳 */
const recent = new Map<string, number>();

/**
 * 任意值展开成可读文本
 *
 * - `value`: 错误、字符串、事件或任意对象
 *
 * # 返回值
 *
 * Error 取堆栈（没堆栈退回 message），对象走 JSON，其余走 String
 */
function describe(value: unknown): string {
  if (value instanceof Error) return value.stack || value.message || String(value);
  if (typeof value === "string") return value;
  if (typeof value === "object" && value !== null) {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

/**
 * 把 console.error 的多个参数拼成一条消息
 *
 * - `args`: console.error 收到的原始参数
 *
 * # 返回值
 *
 * 空格分隔的一条文本，超长时截断
 */
export function formatError(args: unknown[]): string {
  return args
    .map(describe)
    .filter((text) => text.length > 0)
    .join(" ")
    .slice(0, MAX_LEN);
}

/** 当前窗口 kind，日志里能看出是哪个窗口出的问题；单窗口模式下就是当前页，没有参数即主窗口 */
function windowKind(): string {
  return new URLSearchParams(window.location.search).get(WIN_PARAM) ?? "main";
}

/**
 * 转发一条前端错误
 *
 * 相同文本在去重窗口内只记一次：5 秒轮询反复失败时不至于把日志刷满。
 *
 * - `text`: 已经拼好的一条错误文本
 */
export function reportFrontendError(text: string) {
  if (!installed || forwarding || !text) return;
  const now = Date.now();
  const last = recent.get(text);
  if (last !== undefined && now - last < DEDUPE_MS) return;
  if (recent.size >= DEDUPE_MAX) {
    for (const [key, at] of recent) {
      if (now - at >= DEDUPE_MS) recent.delete(key);
    }
  }
  recent.set(text, now);
  forwarding = true;
  commands.windows
    .logFrontend(windowKind(), text)
    .catch(() => { })
    .finally(() => {
      forwarding = false;
    });
}

/**
 * 装上前端错误转发
 *
 * 必须在挂载前调用：越早装，启动阶段的错误越能记上。浏览器预览没有 IPC，直接不装。
 */
export function installFrontendErrorLog() {
  if (installed || !isTauri()) return;
  installed = true;

  const rawError = console.error.bind(console);
  console.error = (...args: unknown[]) => {
    rawError(...args);
    reportFrontendError(formatError(args));
  };

  // 未捕获异常：带文件名与行列（console.error 那条不一定有）
  window.addEventListener("error", (event) => {
    const where = event.filename ? ` (${event.filename}:${event.lineno}:${event.colno})` : "";
    reportFrontendError(describe(event.error ?? event.message) + where);
  });
  // 未处理的 Promise 拒绝：本条文案是机器标识，不进 i18n
  window.addEventListener("unhandledrejection", (event) => {
    reportFrontendError("unhandledrejection: " + describe(event.reason));
  });
}