// 实例配色：按 uuid 稳定取一组，**图标与图表共用**，同一实例在哪儿都是同一个颜色。
// 真图标缺失 / 加载失败时 InstanceIcon 用渐变色块兜底；统计窗口的单日饼图取同组的首色。

/** 一组两色，前者做纯色用途（图表色块），两者拼出图标兜底的渐变 */
const PALETTE: readonly (readonly [string, string])[] = [
  ["#3f8cff", "#5f6cff"],
  ["#34d399", "#22d3ee"],
  ["#f59e0b", "#ef4444"],
  ["#a855f7", "#ec4899"],
  ["#14b8a6", "#3b82f6"],
  ["#f97316", "#f43f5e"],
  ["#84cc16", "#22c55e"],
  ["#06b6d4", "#6366f1"],
];

/**
 * uuid → 色组下标
 *
 * - `uuid`: 实例标识
 *
 * # 返回值
 *
 * 0..PALETTE.length 之间的稳定下标
 */
function indexOf(uuid: string): number {
  let h = 0;
  for (const c of uuid) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h % PALETTE.length;
}

/**
 * 实例图标的兜底渐变
 *
 * - `uuid`: 实例标识
 *
 * # 返回值
 *
 * 可直接给 `background` 的 linear-gradient 值
 */
export function instanceGradient(uuid: string): string {
  const [a, b] = PALETTE[indexOf(uuid)];
  return `linear-gradient(135deg, ${a}, ${b})`;
}

/**
 * 实例的纯色（图表色块用）
 *
 * - `uuid`: 实例标识
 *
 * # 返回值
 *
 * 十六进制颜色，与同实例图标的渐变起点同色
 */
export function instanceColor(uuid: string): string {
  return PALETTE[indexOf(uuid)][0];
}
