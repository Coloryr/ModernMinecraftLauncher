// 资源下载任务状态（添加资源窗口 / 主窗口 / 资源管理窗口共用）
//
// 任务在后端全局表维护（DOWNLOAD_NOW），与窗口生命周期解耦；本模块只负责把
// add-resource-status 事件 / 初始查询落到本地 ref。终态（完成 / 失败）的条目
// **不会自动移除**，由用户点「清除已完成」清（见 add_resource_clear_done）。
//
// 另外顺手维护一个"资源列表已过期"标记：下载完成意味着磁盘上真的多了文件，
// 资源管理窗口切回来时靠它决定要不要重读列表（见 useResourceData 的 sync）。
import { ref } from "vue";
import { api, onAddResourceStatus } from "./api";
import type { ResourceStatusDto } from "./bindings";

/** 资源列表是否可能已过期：期间有资源下载完成 */
const listDirty = ref(false);

/** 上一份快照里"已完成"的条目，键是 `pid|fid`，用来认出新完成的那些 */
let doneKeys = new Set<string>();

/** 是否已经收到过一份快照；第一份只当基线，里面的"已完成"不算这次的 */
let seeded = false;

/**
 * 认出新完成的下载任务，有就把列表标记为过期
 *
 * - `e`: 最新的任务快照
 */
function markDirty(e: ResourceStatusDto) {
  const keys = new Set<string>();
  let fresh = false;
  for (const task of e.tasks) {
    if (!task.done) continue;
    const key = `${task.pid}|${task.fid}`;
    keys.add(key);
    if (!doneKeys.has(key)) fresh = true;
  }
  const changed = seeded && fresh;
  doneKeys = keys;
  seeded = true;
  if (changed) listDirty.value = true;
}

/**
 * 取走"资源列表已过期"的标记，资源管理窗口切回来时用它决定要不要重读
 *
 * # 返回值
 *
 * 上次取走之后有资源下载完成则为 true，同时把标记清掉
 */
export function takeResourceListDirty(): boolean {
  if (!listDirty.value) return false;
  listDirty.value = false;
  return true;
}

/**
 * 订阅资源下载任务状态
 *
 * - `own`: 是否为添加资源窗口本体。true = 始终显示进度条；
 *   false = 仅在添加资源窗口关闭时负责显示，主窗口用
 *
 * # 返回值
 *
 * - `status`: 最近一次任务快照
 * - `init`: 挂载时调用，先同步一次快照再订阅事件，返回解绑函数
 */
export function useResourceStatus(own: boolean) {
  const status = ref<ResourceStatusDto | null>(null);

  function apply(e: ResourceStatusDto) {
    status.value = e;
  }

  /** 挂载时调用：先同步一次快照，再订阅事件；返回解绑函数 */
  async function init(): Promise<() => void> {
    try {
      // 首份快照只当基线：里面的"已完成"不是这段期间完成的
      const first = await api.getResourceStatus();
      markDirty(first);
      apply(first);
    } catch {
      status.value = null;
    }
    return onAddResourceStatus((e) => {
      // 先认"有没有下载完成"：不显示进度条的窗口（资源管理）也要靠它知道该重读，
      // 下面那条过滤只决定本窗口要不要跟着更新进度
      markDirty(e);
      // 添加资源窗口开着时主窗口不显示，由 windowOpen 决定，这里只存数据
      if (!own && e.windowOpen) return;
      apply(e);
    });
  }

  return { status, init };
}
