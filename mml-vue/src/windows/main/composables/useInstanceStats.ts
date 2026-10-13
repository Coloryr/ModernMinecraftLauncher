// 实例的启动统计：启动次数 / 累计游玩时长 / 最近游玩时间。
//
// 数据来自内核统计：`stats.data` 的 `StatsInstanceDto`。这里取代了原先写死在 MainWindow
// 里的 `PLAY_HOURS` 模拟表 —— 那张表拿 `11111111-…` 这种假 uuid 当键，真实实例一律查到 0，
// 所以界面上恒显示「0 小时」。
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

  /** 统计快照；加载中与未知实例都为 null */
  function statsOf(uuid: string): StatsInstanceDto | null {
    return stats.value.find((s) => s.uuid === uuid) ?? null;
  }

  /** 累计游玩小时数；加载中与未知实例都按 0 算，保留一位小数 */
  function playHoursOf(uuid: string): number {
    const seconds = statsOf(uuid)?.seconds ?? 0;
    return Math.round(seconds / 360) / 10;
  }

  return { statsOf, playHoursOf, refresh };
}
