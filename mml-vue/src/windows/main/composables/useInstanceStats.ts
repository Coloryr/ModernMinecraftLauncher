// 实例的启动统计：启动次数 / 累计游玩时长 / 最近游玩时间。
//
// 数据来自内核统计，即 `stats.data` 里的 `StatsInstanceDto`。内核只在内存里累加，
// 这里拿到的是一份快照，所以启动、失败、退出都要重拉，运行中还要按间隔补拉。
import { onMounted, onUnmounted, ref } from "vue";

import { api } from "../../../lib/api";
import type { StatsInstanceDto } from "../../../lib/bindings";

/** 运行中补拉累计时长的间隔，单位毫秒；界面按 0.1 小时显示，30 秒足够跟上 */
const RUNNING_REFRESH_MS = 30_000;

/**
 * 实例启动统计查询
 *
 * - `options.running`: 选中实例是否正在运行；返回 `true` 时按固定间隔补拉累计时长
 *
 * # 返回值
 *
 * - `statsOf`: 按实例 uuid 取统计快照，未知实例为 `null`
 * - `playHoursOf`: 按实例 uuid 取累计小时数，保留一位小数
 * - `refresh`: 重新拉取统计；启动成功、启动失败、游戏退出与核心加载完成时调用
 */
export function useInstanceStats(options: { running?: () => boolean } = {}) {
  const stats = ref<StatsInstanceDto[]>([]);

  async function refresh() {
    try {
      stats.value = (await api.getStatsData()).instances;
    } catch {
      // 统计拿不到就按 0 显示（与原来查表落空的表现一致），不打扰用户
      stats.value = [];
    }
  }

  onMounted(refresh);
  // 运行中的这一局后端一直在累加，不重拉的话界面上的时长停在启动那一刻。
  // 窗口最小化 / 隐藏时不空转 IPC，回来时由 game-exit 或下一次 tick 补上
  const timer = window.setInterval(() => {
    if (document.hidden || !options.running?.()) return;
    void refresh();
  }, RUNNING_REFRESH_MS);
  onUnmounted(() => window.clearInterval(timer));

  /**
   * 统计快照
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 该实例的统计；加载中与未知实例为 `null`
   */
  function statsOf(uuid: string): StatsInstanceDto | null {
    return stats.value.find((s) => s.uuid === uuid) ?? null;
  }

  /**
   * 累计游玩小时数
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 小时数，保留一位小数；加载中与未知实例按 0 算
   */
  function playHoursOf(uuid: string): number {
    const seconds = statsOf(uuid)?.seconds ?? 0;
    // 先按 0.1 小时取整再除回去，避免浮点尾数
    return Math.round(seconds / 360) / 10;
  }

  return { statsOf, playHoursOf, refresh };
}
