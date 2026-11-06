// 点「取消启动」时如果正在同步服务器包，先问一句：跳过这次更新继续启动，还是结束启动
//
// 与其它启动前询问同一套写法：状态模块 + 常驻在 App 里的弹窗，调用方 await 结果。
import { ref } from "vue";

/** 用户对「取消更新」的选择 */
export type LaunchCancelAnswer = "resume" | "skip" | "stop";

/** 弹窗是否打开 */
export const launchCancelOpen = ref(false);
/** 当前启动阶段，弹窗里说明用 */
export const launchCancelStage = ref("");

/** 等待回答的 promise 回调 */
let pending: ((answer: LaunchCancelAnswer) => void) | null = null;

/**
 * 打开「取消更新」询问弹窗
 *
 * - `stage`: 当前启动阶段，如 downloadserverpack
 *
 * # 返回值
 *
 * 用户的选择，见 LaunchCancelAnswer
 */
export function askLaunchCancel(stage: string): Promise<LaunchCancelAnswer> {
  // 上一问未回答时按"继续更新"放掉，避免旧 Promise 永远不 settle
  pending?.("resume");
  pending = null;

  return new Promise((resolve) => {
    pending = resolve;
    launchCancelStage.value = stage;
    launchCancelOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `answer`: 用户的选择
 */
export function answerLaunchCancel(answer: LaunchCancelAnswer): void {
  launchCancelOpen.value = false;
  launchCancelStage.value = "";
  const resolve = pending;
  pending = null;
  resolve?.(answer);
}
