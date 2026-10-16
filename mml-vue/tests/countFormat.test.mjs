// 大数字简化（下载量）的回归：中英分档各一组
//
// 分档口径是用户定的：中文 百 / 千 / 万 / 百万 / 千万 / 亿，英文 100 / K / M / B，
// 两边进位不同，别改成一个通用规则。
import test from "node:test";
import assert from "node:assert/strict";
import { formatCountIn } from "../src/lib/countFormat.ts";

test("中文：百 / 千 / 万 / 百万 / 千万 / 亿", () => {
  assert.equal(formatCountIn(99, "zh_cn"), "99");
  assert.equal(formatCountIn(100, "zh_cn"), "1百+");
  assert.equal(formatCountIn(999, "zh_cn"), "9百+");
  assert.equal(formatCountIn(1000, "zh_cn"), "1千+");
  assert.equal(formatCountIn(9999, "zh_cn"), "9千+");
  assert.equal(formatCountIn(10000, "zh_cn"), "1万+");
  assert.equal(formatCountIn(12345, "zh_cn"), "1万+");
  assert.equal(formatCountIn(99999, "zh_cn"), "9万+");
  assert.equal(formatCountIn(100000, "zh_cn"), "10万+");
  assert.equal(formatCountIn(1234567, "zh_cn"), "1百万+");
  assert.equal(formatCountIn(12345678, "zh_cn"), "1千万+");
  assert.equal(formatCountIn(123456789, "zh_cn"), "1亿+");
  assert.equal(formatCountIn(1234567890, "zh_cn"), "12亿+");
});

test("英文：100 / K / M / B，与中文分档不同", () => {
  assert.equal(formatCountIn(99, "en_us"), "99");
  assert.equal(formatCountIn(234, "en_us"), "200+");
  assert.equal(formatCountIn(999, "en_us"), "900+");
  assert.equal(formatCountIn(1234, "en_us"), "1K+");
  assert.equal(formatCountIn(12345, "en_us"), "12K+");
  assert.equal(formatCountIn(1234567, "en_us"), "1M+");
  assert.equal(formatCountIn(123456789, "en_us"), "123M+");
  assert.equal(formatCountIn(1234567890, "en_us"), "1B+");
});

test("非法输入按 0，不认识的语言按中文", () => {
  assert.equal(formatCountIn(0, "zh_cn"), "0");
  assert.equal(formatCountIn(-5, "zh_cn"), "0");
  assert.equal(formatCountIn(Number.NaN, "en_us"), "0");
  assert.equal(formatCountIn(1000, "fr"), "1千+");
});
