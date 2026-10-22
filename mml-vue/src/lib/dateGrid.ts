// 日期选择器的月历网格：只做算术，不碰 DOM 也不引 vue，方便单测（见 tests/dateGrid.test.mjs）。
// 组件在 components/ui/BaseDatePicker.vue。

/** 一格日期 */
export interface DayCell {
  /** 完整日期，YYYY-MM-DD */
  date: string;
  /** 日号，1..31 */
  day: number;
  /** 是否属于当前显示的月份；false 表示上月末 / 下月初那几格 */
  inMonth: boolean;
}

/** 一年的月份数 */
const MONTHS = 12;
/** 一周的天数，也用来把周日的 0 挪到末尾 */
const WEEK = 7;
/** 固定 6 行：月份在 5 行与 6 行之间跳会让弹层高度忽高忽低 */
const CELLS = WEEK * 6;

/**
 * 数字补前导零
 *
 * - `n`: 整数
 *
 * # 返回值
 *
 * 至少两位的字符串
 */
function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/**
 * 年月日拼成 YYYY-MM-DD
 *
 * - `year`: 四位年份
 * - `month`: 月份，1..12
 * - `day`: 日号，1..31
 *
 * # 返回值
 *
 * YYYY-MM-DD
 */
export function formatDate(year: number, month: number, day: number): string {
  return String(year).padStart(4, "0") + "-" + pad(month) + "-" + pad(day);
}

/** 解析结果 */
export interface DateParts {
  /** 四位年份 */
  year: number;
  /** 月份，1..12 */
  month: number;
  /** 日号，1..31 */
  day: number;
}

/**
 * 解析 YYYY-MM-DD
 *
 * 只认这个格式，且要真的是那一天（2026-02-30 返回 null）。用本地日历校验，不做时区换算。
 *
 * - `value`: 待解析的字符串
 *
 * # 返回值
 *
 * 解析成功给 [DateParts]，格式不对或日期不存在给 null
 */
export function parseDate(value: string): DateParts | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!m) return null;
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  if (month < 1 || month > MONTHS || day < 1 || day > 31) return null;
  const d = new Date(year, month - 1, day);
  if (d.getFullYear() !== year || d.getMonth() !== month - 1 || d.getDate() !== day) return null;
  return { year, month, day };
}

/**
 * 取某月要显示的 42 格
 *
 * 周一起始：第一格是包含 1 号的那一周的周一，前几格是上个月的月末。
 *
 * - `year`: 四位年份
 * - `month`: 月份，1..12
 *
 * # 返回值
 *
 * 42 个 [DayCell]，按行优先排列
 */
export function monthCells(year: number, month: number): DayCell[] {
  const first = new Date(year, month - 1, 1);
  // getDay() 里 0 是周日；周一起始时把它挪到末尾
  const offset = (first.getDay() + WEEK - 1) % WEEK;
  const cells: DayCell[] = [];
  for (let i = 0; i < CELLS; i++) {
    const d = new Date(year, month - 1, 1 - offset + i);
    cells.push({
      date: formatDate(d.getFullYear(), d.getMonth() + 1, d.getDate()),
      day: d.getDate(),
      inMonth: d.getMonth() === month - 1,
    });
  }
  return cells;
}

/**
 * 前 / 后挪一个月
 *
 * - `year`: 四位年份
 * - `month`: 月份，1..12
 * - `delta`: 挪几个月，负数往前
 *
 * # 返回值
 *
 * 挪完的年月；月份已归一化到 1..12
 */
export function shiftMonth(year: number, month: number, delta: number): { year: number; month: number } {
  const index = year * MONTHS + (month - 1) + delta;
  return { year: Math.floor(index / MONTHS), month: ((index % MONTHS) + MONTHS) % MONTHS + 1 };
}
