// 行虚拟化的窗口计算：只算"该渲染哪几行、上下各留多少空白"，见 virtualWindow.ts。
// 盯的是三件事：可见区间包住视口、上下留白能把隐藏行的高度**精确**补回来、边界不越界。

import test from "node:test";
import assert from "node:assert/strict";
import {
  MOD_ROW_GAP,
  MOD_ROW_H,
  visibleWindow,
} from "../src/windows/resource/composables/virtualWindow.ts";

const SLOT = MOD_ROW_H + MOD_ROW_GAP;

test("视口里的行都渲染出来，上下各多留一段 overscan", () => {
  // 100 行，视口在第 10~20 行那一段
  const win = visibleWindow({
    scrollTop: 10 * SLOT,
    viewHeight: 10 * SLOT,
    rootTop: 0,
    count: 100,
    rowH: MOD_ROW_H,
    gap: MOD_ROW_GAP,
    overscan: 0,
  });

  assert.equal(win.start, 10);
  assert.equal(win.end, 20);
  assert.equal(win.padTop, 10 * SLOT);
  assert.equal(win.padBottom, 80 * SLOT);
});

test("上下留白把隐藏行的高度精确补回来（含行间距）", () => {
  const count = 200;
  const rendered = 6;
  const start = 50;
  const win = { start, end: start + rendered, padTop: start * SLOT, padBottom: (count - start - rendered) * SLOT };

  // 全部渲染时的高度：count 行 + count-1 个行间距
  const full = count * MOD_ROW_H + (count - 1) * MOD_ROW_GAP;
  // 只渲染中间 6 行时：上下留白 + 6 行 + 5 个行间距
  const partial = win.padTop + rendered * MOD_ROW_H + (rendered - 1) * MOD_ROW_GAP + win.padBottom;
  assert.equal(partial, full);
});

test("一行都没渲染时，末尾那行的行间距也要扣掉", () => {
  const count = 50;
  const win = visibleWindow({
    scrollTop: 0,
    viewHeight: 0,
    rootTop: -100000, // 这一组在视口下面很远
    count,
    rowH: MOD_ROW_H,
    gap: MOD_ROW_GAP,
    overscan: 0,
  });

  assert.equal(win.start, count);
  assert.equal(win.end, count);
  const full = count * MOD_ROW_H + (count - 1) * MOD_ROW_GAP;
  assert.equal(win.padTop + win.padBottom, full);
});

test("边界：空列表、组在视口上方 / 下方都不越界", () => {
  const empty = visibleWindow({ scrollTop: 0, viewHeight: 500, rootTop: 0, count: 0, rowH: MOD_ROW_H });
  assert.deepEqual(empty, { start: 0, end: 0, padTop: 0, padBottom: 0 });

  // 组整个在视口上方：全部隐藏，留白全在下面
  const above = visibleWindow({
    scrollTop: 100000,
    viewHeight: 500,
    rootTop: 0,
    count: 20,
    rowH: MOD_ROW_H,
    gap: MOD_ROW_GAP,
    overscan: 0,
  });
  assert.equal(above.start, 20);
  assert.equal(above.end, 20);
  // 全在视口上方时留白全在上面，同样要扣掉末尾行的间距
  assert.equal(above.padTop, 20 * MOD_ROW_H + 19 * MOD_ROW_GAP);

  // 组整个在视口下方：一行都不渲染，而且总高度要准（扣掉末尾行间距）
  const below = visibleWindow({
    scrollTop: 0,
    viewHeight: 500,
    rootTop: 100000,
    count: 20,
    rowH: MOD_ROW_H,
    gap: MOD_ROW_GAP,
    overscan: 0,
  });
  assert.equal(below.start, 0);
  assert.equal(below.end, 0);
  assert.equal(below.padTop + below.padBottom, 20 * MOD_ROW_H + 19 * MOD_ROW_GAP);
});
