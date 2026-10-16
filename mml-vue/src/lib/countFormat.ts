// 下载量这类大数字的简化显示
//
// 列表卡片上原样铺 `1,234,567` 太长，把标题挤没了；这里只留量级 + 一个 `+`，
// 表示"至少这么多"：1千+ / 1万+ / 1百万+ / 1亿+。
//
// **中英进位口径不同**，分档表各写一份：中文四位一进（万 / 亿），英文三位一进（K / M / B）。
// 不到 100 的数字不简化，原样显示。
//
// 这里只做纯计算，语言由调用方传进来；界面用的是 i18n 里读当前语言的那个包装。

/** 一个量级档 */
interface CountUnit {
  /** 量级起点，如 10000 是中文的"万" */
  unit: number;
  /** 跟着数字的单元名；空串表示只显示数字本身 */
  label: string;
  /**
   * 显示值 = `floor(n / unit) × scale`
   *
   * 中文各档都是 1（1万、12万）；英文只有百位档要 ×100（200+），K / M / B 都是 1
   */
  scale: number;
}

/** 各语言的分档表，从大到小排 */
const UNITS: Record<string, CountUnit[]> = {
  zh_cn: [
    { unit: 1e8, label: "亿", scale: 1 },
    { unit: 1e7, label: "千万", scale: 1 },
    { unit: 1e6, label: "百万", scale: 1 },
    { unit: 1e4, label: "万", scale: 1 },
    { unit: 1e3, label: "千", scale: 1 },
    { unit: 100, label: "百", scale: 1 },
  ],
  en_us: [
    { unit: 1e12, label: "T", scale: 1 },
    { unit: 1e9, label: "B", scale: 1 },
    { unit: 1e6, label: "M", scale: 1 },
    { unit: 1e3, label: "K", scale: 1 },
    { unit: 100, label: "", scale: 100 },
  ],
};

/**
 * 按指定语言简化大数字
 *
 * - `n`: 原始数字；非有限数或负数按 0 处理
 * - `lang`: 语言，取值 `zh_cn` / `en_us`；不认识的语言按中文
 *
 * # 返回值
 *
 * 形如 `1万+` / `10K+`；不到 100 时是原数的十进制写法
 */
export function formatCountIn(n: number, lang: string): string {
  const value = Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
  if (value < 100) return String(value);

  for (const item of UNITS[lang] ?? UNITS.zh_cn) {
    if (value >= item.unit) {
      return `${Math.floor(value / item.unit) * item.scale}${item.label}+`;
    }
  }
  return String(value);
}
