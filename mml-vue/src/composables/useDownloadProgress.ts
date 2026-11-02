// 下载队列的实时总进度
//
// 下载窗口、标题栏上的下载指示器、启动按钮三处要的是同一个数，口径见 lib/progress.ts。
// 启动阶段的百分比由内核按"阶段等权"给（下载阶段恒为 36），所以下载期间必须换成这里的真实值。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api, onDownloadItem, onDownloadTask } from "../lib/api";
import { downloadOverallPercent } from "../lib/progress";
import type { DownloadStatusDto } from "../lib/bindings";
import { useUnlisteners } from "./useUnlisteners";

/**
 * 跟踪下载队列的实时总进度
 *
 * # 返回值
 *
 * - `percent`: 队列总进度百分比，0–100
 * - `tasks`: 队列里的任务快照，要按任务另算口径时用它
 * - `hasTasks`: 队列里当前有没有任务
 */
export function useDownloadProgress() {
  const status = ref<DownloadStatusDto>({ tasks: [], threads: [], speed: 0, paused: false });

  /** 刷新合并窗口，单位毫秒：一个文件会发好几次状态事件，逐条拉状态会把主线程塞满；与下载窗口的 100ms 同档 */
  const REFRESH_DEBOUNCE_MS = 100;
  /** 合并窗口的定时器；null 表示没有排队的刷新 */
  let timer: number | null = null;

  /** 拉一次快照；失败保留上次数据，一次失败不该让界面上的进度跳回 0 */
  async function refresh() {
    try {
      status.value = await api.getDownloadStatus();
    } catch {
      /* 忽略：后端不可用时保留上次显示 */
    }
  }

  /** 合并事件触发的刷新：窗口内只拉一次，尾沿补上最后一次 */
  function scheduleRefresh() {
    if (timer !== null) return;
    timer = window.setTimeout(() => {
      timer = null;
      void refresh();
    }, REFRESH_DEBOUNCE_MS);
  }

  const { track } = useUnlisteners();

  onMounted(async () => {
    await refresh();
    // 任务 / 文件状态变化都会广播，跟着刷新即可
    track(onDownloadTask(scheduleRefresh));
    track(onDownloadItem(scheduleRefresh));
  });

  onUnmounted(() => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
  });

  return {
    tasks: computed(() => status.value.tasks),
    percent: computed(() => downloadOverallPercent(status.value.tasks)),
    hasTasks: computed(() => status.value.tasks.length > 0),
  };
}
