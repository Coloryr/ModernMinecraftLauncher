// 账户占用弹窗的状态
//
// 启动流程（主窗口 / 存档页 / 日志窗口）共用：后端返回「账户已被其它实例占用」后，
// 这里挂起一次询问，由 App 里常驻的 AccountLockModal 回答，再决定是否带 force 重试。
import { ref } from "vue";

/** 弹窗是否打开 */
export const accountLockOpen = ref(false);

/** 用户的选择 */
export type AccountLockAnswer = {
  /** 是否继续启动；false = 取消 */
  ok: boolean;
  /** 是否保留账户占用检查；false = 这次之后不再提示 */
  keep: boolean;
};

/** 等待回答的 promise 回调 */
let pending: ((answer: AccountLockAnswer) => void) | null = null;

/**
 * 打开账户占用弹窗并等待用户选择
 *
 * # 返回值
 *
 * 用户选择继续 / 取消，以及是否继续检测
 */
export function askAccountLock(): Promise<AccountLockAnswer> {
  // 上一问还没回答就来了新的一问：按“取消、继续检测”放掉旧的，避免它的 Promise 永远不落地
  pending?.({ ok: false, keep: true });
  pending = null;

  return new Promise((resolve) => {
    pending = resolve;
    accountLockOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `ok`: 是否继续启动
 * - `keep`: 是否继续检测账户占用
 */
export function answerAccountLock(ok: boolean, keep: boolean): void {
  accountLockOpen.value = false;
  const resolve = pending;
  pending = null;
  resolve?.({ ok, keep });
}
