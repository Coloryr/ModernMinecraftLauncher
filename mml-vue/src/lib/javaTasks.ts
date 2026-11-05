// Java 安装任务状态：标题栏指示器与进度弹窗共用
//
// 任务在后端维护，窗口关掉也在跑；这里只把 java-install-status 事件落到本地 ref。
import { ref } from "vue";
import { onJavaInstallStatus } from "./api";
import { commands } from "./bindings";
import type { JavaInstallTaskDto } from "./bindings";

/** 当前全部安装任务 */
export const javaTasks = ref<JavaInstallTaskDto[]>([]);

/**
 * 任务是否还在进行
 *
 * - `task`: 安装任务
 */
export function isRunning(task: JavaInstallTaskDto): boolean {
  return task.stage !== "done" && task.stage !== "failed" && task.stage !== "cancelled";
}

/**
 * 总进度：进行中任务的进度均值，全部结束时为 100
 *
 * - `tasks`: 任务列表
 */
export function overallPercent(tasks: JavaInstallTaskDto[]): number {
  const running = tasks.filter(isRunning);
  if (!running.length) return 100;
  const sum = running.reduce((acc, task) => acc + Math.min(1, Math.max(0, task.progress)), 0);
  return (sum / running.length) * 100;
}

/**
 * 订阅安装任务：先拉一次当前列表，再跟事件走
 *
 * # 返回值
 *
 * 退订函数
 */
export async function initJavaTasks(): Promise<() => void> {
  javaTasks.value = await commands.javaDownload.status().catch(() => []);
  return onJavaInstallStatus((list) => {
    javaTasks.value = list;
  });
}
