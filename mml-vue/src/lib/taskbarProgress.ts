// 任务栏图标上的进度条
//
// Windows 的任务栏按钮会在图标下面画一条进度；其它平台由 Tauri 忽略。
// 安装整合包这类"整窗就一个任务"的场景用它，装完传 `null` 把进度条收掉。
// 进度条只是锦上添花：权限缺失或平台不支持时静默失败，不能挡住安装。
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";

/** 上一次真正发出去的整数百分比，避免每一帧都发一条 IPC */
let sent = Number.NaN;

/**
 * 更新任务栏图标的进度
 *
 * - `percent`: 0–100；`"indeterminate"` 表示还不知道总量；`null` 表示结束，收掉进度条
 */
export function setTaskbarProgress(percent: number | "indeterminate" | null): void {
  const next = percent === null ? -1 : percent === "indeterminate" ? -2 : Math.round(percent);
  // 四舍五入之后没变就不发：安装进度一秒能刷几十次
  if (next === sent) return;
  sent = next;

  const state =
    next === -1
      ? { status: ProgressBarStatus.None }
      : {
          status: next === -2 ? ProgressBarStatus.Indeterminate : ProgressBarStatus.Normal,
          progress: next === -2 ? undefined : Math.max(0, Math.min(100, next)),
        };
  getCurrentWindow()
    .setProgressBar(state)
    .catch(() => {
      // 失败就把缓存清掉，下一次进度还能重试
      sent = Number.NaN;
    });
}
