// 资源下载任务状态（添加资源窗口 / 主窗口共用）
//
// 任务在后端全局表维护（DOWNLOAD_NOW），与窗口生命周期解耦；本模块只负责把
// add-resource-status 事件 / 初始查询落到本地 ref。终态（完成 / 失败）的条目
// 由后端保留一段时间后自动移除并广播，前端无需清理。
import { ref } from "vue";
import { api, onAddResourceStatus } from "./api";
import type { ResourceStatusDto } from "./bindings";

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
      apply(await api.getResourceStatus());
    } catch {
      status.value = null;
    }
    return onAddResourceStatus((e) => {
      // 添加资源窗口开着时主窗口不显示（由 windowOpen 决定，这里只存数据）
      if (!own && e.windowOpen) return;
      apply(e);
    });
  }

  return { status, init };
}
