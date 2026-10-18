// 下载队列的实时总进度
//
// 下载窗口、标题栏上的下载指示器、启动按钮三处要的是同一个数，口径见 lib/progress.ts。
// 启动阶段的百分比由内核按"阶段等权"给（下载阶段恒为 36），所以下载期间必须换成这里的真实值。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api, onDownloadItem, onDownloadTask } from "../lib/api";
import { downloadOverallPercent } from "../lib/progress";
import type { DownloadStatusDto } from "../lib/bindings";

/**
 * 跟踪下载队列的实时总进度
 *
 * # 返回值
 *
 * - `percent`: 队列总进度百分比，0–100
 * - `hasTasks`: 队列里当前有没有任务
 */
export function useDownloadProgress() {
  const status = ref<DownloadStatusDto>({ tasks: [], threads: [], speed: 0, paused: false });

  /** 拉一次快照；失败保留上次数据，一次失败不该让界面上的进度跳回 0 */
  async function refresh() {
    try {
      status.value = await api.getDownloadStatus();
    } catch {
      /* 忽略：后端不可用时保留上次显示 */
    }
  }

  let unsubs: Array<() => void> = [];

  onMounted(async () => {
    await refresh();
    // 任务 / 文件状态变化都会广播，跟着刷新即可
    unsubs.push(await onDownloadTask(() => void refresh()));
    unsubs.push(await onDownloadItem(() => void refresh()));
  });

  onUnmounted(() => {
    unsubs.forEach((fn) => fn());
    unsubs = [];
  });

  return {
    percent: computed(() => downloadOverallPercent(status.value.tasks)),
    hasTasks: computed(() => status.value.tasks.length > 0),
  };
}
