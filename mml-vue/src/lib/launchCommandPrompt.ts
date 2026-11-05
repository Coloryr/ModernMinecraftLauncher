// 启动前 / 启动后命令的确认弹窗
//
// 启动流程（主窗口 / 存档页 / 日志窗口 / 分组与批量启动）共用：启动前向后端要一次
// 这次会执行的命令，有就挂起询问，由 App 里常驻的 LaunchCommandModal 回答。
import { ref } from "vue";
import type { LaunchCommandDto } from "./bindings";

/** 弹窗是否打开 */
export const launchCommandOpen = ref(false);

/** 这次问的是哪些命令；关闭后清空 */
export const launchCommandInfo = ref<LaunchCommandDto | null>(null);

/** 用户的选择 */
export type LaunchCommandAnswer = {
  /** 这次怎么处理：run 执行 / skip 跳过 / cancel 取消启动 */
  action: "run" | "skip" | "cancel";
  /** 是否保留提醒；false = 不再提醒，以后直接执行 */
  notice: boolean;
};

/** 等待回答的 promise 回调 */
let pending: ((answer: LaunchCommandAnswer) => void) | null = null;

/**
 * 打开启动命令确认弹窗并等待用户选择
 *
 * - `info`: 这次会执行的启动前 / 启动后命令，没配的那条为空
 *
 * # 返回值
 *
 * 用户选择执行 / 跳过 / 取消，以及是否关掉提醒
 */
export function askLaunchCommand(info: LaunchCommandDto): Promise<LaunchCommandAnswer> {
  // 上一问还没回答就来了新的一问：先按"跳过、继续提醒"放掉旧的，
  // 否则它的 Promise 永远不 settle。正常启动是串行的，这里只是兜底。
  pending?.({ action: "skip", notice: true });
  pending = null;

  return new Promise((resolve) => {
    pending = resolve;
    launchCommandInfo.value = info;
    launchCommandOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `answer`: 用户的选择
 */
export function answerLaunchCommand(answer: LaunchCommandAnswer) {
  launchCommandOpen.value = false;
  launchCommandInfo.value = null;
  const resolve = pending;
  pending = null;
  resolve?.(answer);
}
