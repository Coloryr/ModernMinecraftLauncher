// 实例的启动统计：启动次数 / 累计游玩时长 / 最近游玩时间。
//
// 数据来自内核统计，即 `stats.data` 里的 `StatsInstanceDto`。
import { onMounted, ref } from "vue";

import { api } from "../../../lib/api";
import type { StatsInstanceDto } from "../../../lib/bindings";

/**
 * 实例启动统计查询
 *
 * # 返回值
 *
 * - `statsOf`: 按实例 uuid 取统计快照，未知实例为 `null`
 * - `playHoursOf`: 按实例 uuid 取累计小时数，保留一位小数
 * - `refresh`: 重新拉取统计，游戏退出后调用，否则界面上的次数停在启动那一刻
 */
export function useInstanceStats() {
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
