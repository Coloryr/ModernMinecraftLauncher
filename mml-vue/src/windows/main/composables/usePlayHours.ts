// 实例的累计游玩时长（实例详情里那一行）。
//
// 数据来自内核统计：`stats.data` 的 `StatsInstanceDto.seconds`。
// 这里取代了原先写死在 MainWindow 里的 `PLAY_HOURS` 模拟表 —— 那张表拿 `11111111-…`
// 这种假 uuid 当键，真实实例一律查到 0，所以界面上恒显示「0 小时」。
import { onMounted, ref } from "vue";

import { api } from "../../../lib/api";
import type { StatsInstanceDto } from "../../../lib/bindings";

/**
 * 实例的累计游玩时长查询
 *
 * # 返回值
 *
 * - `playHoursOf`: 按实例 uuid 取累计小时数，保留一位小数
 */
export function usePlayHours() {
  const stats = ref<StatsInstanceDto[]>([]);

  onMounted(async () => {
    try {
      stats.value = (await api.getStatsData()).instances;
    } catch {
      // 统计拿不到就按 0 显示（与原来查表落空的表现一致），不打扰用户
      stats.value = [];
    }
  });

  /** 累计游玩小时数；加载中与未知实例都按 0 算，保留一位小数 */
  function playHoursOf(uuid: string): number {
    const seconds = stats.value.find((s) => s.uuid === uuid)?.seconds ?? 0;
    return Math.round(seconds / 360) / 10;
  }

  return { playHoursOf };
}
