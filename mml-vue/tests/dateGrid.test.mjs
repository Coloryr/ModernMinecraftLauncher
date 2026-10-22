// 日期选择器的月历网格：周一起始、固定 42 格、只认真的那一天。
import test from "node:test";
import assert from "node:assert/strict";
import { formatDate, monthCells, parseDate, shiftMonth } from "../src/lib/dateGrid.ts";

test("42 格从包含 1 号的那一周的周一开始", () => {
  // 2026-10-01 是周四：第一格应是 2026-09-28（周一）
  const cells = monthCells(2026, 10);
  assert.equal(cells.length, 42);
  assert.equal(cells[0].date, "2026-09-28");
  assert.equal(cells[0].inMonth, false);
  const first = cells.find((c) => c.date === "2026-10-01");
  assert.equal(first.day, 1);
  assert.equal(first.inMonth, true);
});

test("当月天数落在网格里，跨月的格子标 inMonth=false", () => {
  const cells = monthCells(2026, 2);
  const inMonth = cells.filter((c) => c.inMonth);
  assert.equal(inMonth.length, 28); // 2026 不是闰年
  assert.equal(inMonth[0].date, "2026-02-01");
  assert.equal(inMonth.at(-1).date, "2026-02-28");
});

test("闰年 2 月是 29 天", () => {
  assert.equal(monthCells(2024, 2).filter((c) => c.inMonth).length, 29);
});

test("解析只认真的那一天", () => {
  assert.deepEqual(parseDate("2026-10-22"), { year: 2026, month: 10, day: 22 });
  assert.equal(parseDate("2026-02-30"), null);
  assert.equal(parseDate("2026-13-01"), null);
  assert.equal(parseDate("26-1-1"), null);
  assert.equal(parseDate(""), null);
});

test("翻月跨年", () => {
  assert.deepEqual(shiftMonth(2026, 1, -1), { year: 2025, month: 12 });
  assert.deepEqual(shiftMonth(2026, 12, 1), { year: 2027, month: 1 });
  assert.deepEqual(shiftMonth(2026, 6, 0), { year: 2026, month: 6 });
});

test("格式化补零", () => {
  assert.equal(formatDate(2026, 1, 5), "2026-01-05");
  assert.equal(formatDate(999, 12, 31), "0999-12-31");
});
