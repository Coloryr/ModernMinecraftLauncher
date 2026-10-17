// 下载进度口径（全仓统一，别在别处另写一套）
//
// 一个进度单元里含**多个下载项目（文件）**时，进度按「已完成项目数 / 总项目数」算；
// 只含 **1 个项目**时才回退到「已下字节 / 总字节」。
//
// 为什么这么分：整合包安装这类任务一次要下几十个文件，按字节算的话百分比被最大的那个
// 文件拽着走 —— 小文件都下完了条子还停在 20%；按项目数才反映"还剩几个没下"。
// 反过来单文件任务（单独下一个模组）没有"项目数"可言，只有字节有意义。
//
// 下载管理窗口的列表与总览、下载指示器、资源下载进度条都从这里取数，
// 免得同一个数字在两处算出不同的值。
import type { DownloadTaskDto, ResourceTaskDto } from "./bindings";

/**
 * 算 0–100 百分比
 *
 * - `now`: 已完成量
 * - `all`: 总量
 *
 * # 返回值
 *
 * 百分比；总量未知按 0 算，不返回 NaN
 */
export function percent(now: number, all: number): number {
  if (!(all > 0)) return 0;
  return Math.min(100, (now / all) * 100);
}

/**
 * 该进度单元是否按项目数计，文件多于 1 个才值得数个数
 *
 * - `total`: 该进度单元里的文件总数
 *
 * # 返回值
 *
 * 文件多于 1 个时为 true
 */
export function byProjectCount(total: number): boolean {
  return total > 1;
}

/**
 * 单个下载任务的进度，0–100：多文件按项目数，单文件按字节
 *
 * - `task`: 下载任务
 *
 * # 返回值
 *
 * 进度百分比
 */
export function downloadTaskPercent(task: DownloadTaskDto): number {
  const finished = task.completed + task.failed;
  if (byProjectCount(task.total)) {
    return percent(finished, task.total);
  }
  if (task.allBytes > 0) {
    return percent(task.nowBytes, task.allBytes);
  }
  // 单文件但还没拿到总大小，下载器尚未读到 Content-Length：退回文件数口径
  return percent(finished, task.total);
}

/**
 * 下载任务集合的总进度，0–100
 *
 * 与单任务同一口径，把全部任务汇总成一个进度。
 *
 * - `tasks`: 下载任务列表
 *
 * # 返回值
 *
 * 进度百分比
 */
export function downloadOverallPercent(tasks: DownloadTaskDto[]): number {
  const files = tasks.reduce((n, x) => n + x.total, 0);
  const finished = tasks.reduce((n, x) => n + x.completed + x.failed, 0);
  if (byProjectCount(files)) {
    return percent(finished, files);
  }
  const all = tasks.reduce((n, x) => n + x.allBytes, 0);
  if (all > 0) {
    return percent(
      tasks.reduce((n, x) => n + x.nowBytes, 0),
      all,
    );
  }
  return percent(finished, files);
}

/** 资源下载任务按「下载项目（pid）」分组后的进度 */
export interface ResourceGroupProgress {
  /** 项目 ID */
  pid: string;
  /** 显示名（取组内第一条） */
  name: string;
  /** 项目图标地址（取组内第一条）；空串表示没有，界面用首字母占位 */
  icon: string;
  /** 组内文件数 */
  total: number;
  /** 组内已结束（完成 / 失败 / 取消）的文件数 */
  finished: number;
  /** 组内是否有失败 */
  failed: boolean;
  /** 组内是否有取消 */
  cancelled: boolean;
  /** 组内是否全部完成 */
  done: boolean;
  /** 组内进度（0–100）：多文件按项目数，单文件按该文件自己的字节进度 */
  percent: number;
  /** 是否按项目数计（决定文案说"项目"还是百分比） */
  byCount: boolean;
}

/**
 * 按 pid 把资源下载任务分组
 *
 * 同一个项目下选了多个版本 / 多个文件时算**一个下载项目**，进度按组内文件数推进；
 * 只有一个文件时用该文件自己的字节进度，见模块头部的口径说明。
 *
 * - `tasks`: 资源下载任务列表
 *
 * # 返回值
 *
 * 每个 pid 一组，含组内进度与"是否按项目数计"
 */
export function resourceGroups(tasks: ResourceTaskDto[]): ResourceGroupProgress[] {
  const map = new Map<string, ResourceTaskDto[]>();
  for (const task of tasks) {
    const list = map.get(task.pid);
    if (list) {
      list.push(task);
    } else {
      map.set(task.pid, [task]);
    }
  }

  return [...map.entries()].map(([pid, list]) => {
    const finished = list.filter((task) => task.done || task.failed || task.cancelled).length;
    const byCount = byProjectCount(list.length);
    return {
      pid,
      name: list[0].name,
      icon: list[0].icon,
      total: list.length,
      finished,
      failed: list.some((task) => task.failed),
      cancelled: list.some((task) => task.cancelled),
      done: list.every((task) => task.done),
      byCount,
      percent: byCount ? percent(finished, list.length) : list[0].progress,
    };
  });
}

/**
 * 资源下载的总进度，0–100
 *
 * 还没下完的各项目进度的均值，已结束的不参与平均；全都结束了就是 100。
 * 进度卡片与标题栏指示器共用这一份，免得同一个数字在两处算出不同的值。
 *
 * - `tasks`: 资源下载任务列表
 *
 * # 返回值
 *
 * 进度百分比
 */
export function resourceOverallPercent(tasks: ResourceTaskDto[]): number {
  const groups = resourceGroups(tasks);
  const active = groups.filter((group) => group.finished < group.total);
  if (!active.length) return 100;
  return active.reduce((acc, group) => acc + group.percent, 0) / active.length;
}
