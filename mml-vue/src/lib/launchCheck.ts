// 启动前检查弹窗的状态
//
// 启动流程（主窗口 / 存档页 / 日志窗口）共用：后端发现「有模组但加载器是原版」或
// 「模组多、最大内存偏低」时，这里挂起一次询问，由 App 里常驻的 LaunchCheckModal 回答。
import { ref } from "vue";
import type { LaunchPrecheckDto } from "./bindings";

/** 本次问的是哪一项 */
export type LaunchCheckKind = "loader" | "memory";

/** 弹窗是否打开 */
export const launchCheckOpen = ref(false);

/** 本次询问的类型 */
export const launchCheckKind = ref<LaunchCheckKind>("loader");

/** 本次要展示的检查结果 */
export const launchCheckInfo = ref<LaunchPrecheckDto | null>(null);

/**
 * 用户的选择
 *
 * - `ok`: 是否继续启动
 * - `disable`: 是否关掉这一项检查
 */
export type LaunchCheckAnswer = {
  /** 是否继续启动；false = 取消 */
  ok: boolean;
  /** 是否以后不再检查这一项 */
  disable: boolean;
};

/** 等待回答的 promise 回调 */
let pending: ((answer: LaunchCheckAnswer) => void) | null = null;

/**
 * 打开启动前检查弹窗并等待用户选择
 *
 * - `kind`: 加载器检查或内存检查
 * - `info`: 后端返回的模组数 / 最大内存等
 *
 * # 返回值
 *
 * 用户选择继续 / 取消，以及是否关掉这项检查
 */
export function askLaunchCheck(kind: LaunchCheckKind, info: LaunchPrecheckDto): Promise<LaunchCheckAnswer> {
  // 与账户占用同一口径：上一问未回答时按“取消、保留检查”放掉，别让旧 Promise 悬着
  pending?.({ ok: false, disable: false });
  pending = null;

  return new Promise((resolve) => {
    pending = resolve;
    launchCheckKind.value = kind;
    launchCheckInfo.value = info;
    launchCheckOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `ok`: 是否继续启动
 * - `disable`: 是否关掉这项检查
 */
export function answerLaunchCheck(ok: boolean, disable: boolean): void {
  launchCheckOpen.value = false;
  launchCheckInfo.value = null;
  const resolve = pending;
  pending = null;
  resolve?.({ ok, disable });
}
