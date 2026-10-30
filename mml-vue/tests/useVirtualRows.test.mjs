// 行虚拟化的窗口计算：只算"该渲染哪几行、上下各留多少空白"，见 virtualWindow.ts。
// 盯的是三件事：可见区间包住视口、上下留白能把隐藏行的高度**精确**补回来、边界不越界。

import test from "node:test";
import assert from "node:assert/strict";
import {
  LIST_ROW_GAP,
  LIST_ROW_H,
  visibleWindow,
  visibleWindowOfHeights,
} from "../src/composables/virtualWindow.ts";

const SLOT = LIST_ROW_H + LIST_ROW_GAP;

test("视口里的行都渲染出来，上下各多留一段 overscan", () => {
  // 100 行，视口在第 10~20 行那一段
  const win = visibleWindow({
    scrollTop: 10 * SLOT,
    viewHeight: 10 * SLOT,
    rootTop: 0,
    count: 100,
    rowH: LIST_ROW_H,
    gap: LIST_ROW_GAP,
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
  const full = count * LIST_ROW_H + (count - 1) * LIST_ROW_GAP;
  // 只渲染中间 6 行时：上下留白 + 6 行 + 5 个行间距
  const partial = win.padTop + rendered * LIST_ROW_H + (rendered - 1) * LIST_ROW_GAP + win.padBottom;
  assert.equal(partial, full);
});

test("一行都没渲染时，末尾那行的行间距也要扣掉", () => {
  const count = 50;
  const win = visibleWindow({
    scrollTop: 0,
    viewHeight: 0,
    rootTop: -100000, // 这一组在视口下面很远
    count,
    rowH: LIST_ROW_H,
    gap: LIST_ROW_GAP,
    overscan: 0,
  });

  assert.equal(win.start, count);
  assert.equal(win.end, count);
  const full = count * LIST_ROW_H + (count - 1) * LIST_ROW_GAP;
  assert.equal(win.padTop + win.padBottom, full);
});

test("边界：空列表、组在视口上方 / 下方都不越界", () => {
  const empty = visibleWindow({ scrollTop: 0, viewHeight: 500, rootTop: 0, count: 0, rowH: LIST_ROW_H });
  assert.deepEqual(empty, { start: 0, end: 0, padTop: 0, padBottom: 0 });

  // 组整个在视口上方：全部隐藏，留白全在下面
  const above = visibleWindow({
    scrollTop: 100000,
    viewHeight: 500,
    rootTop: 0,
    count: 20,
    rowH: LIST_ROW_H,
    gap: LIST_ROW_GAP,
    overscan: 0,
  });
  assert.equal(above.start, 20);
  assert.equal(above.end, 20);
  // 全在视口上方时留白全在上面，同样要扣掉末尾行的间距
  assert.equal(above.padTop, 20 * LIST_ROW_H + 19 * LIST_ROW_GAP);

  // 组整个在视口下方：一行都不渲染，而且总高度要准（扣掉末尾行间距）
  const below = visibleWindow({
    scrollTop: 0,
    viewHeight: 500,
    rootTop: 100000,
    count: 20,
    rowH: LIST_ROW_H,
    gap: LIST_ROW_GAP,
    overscan: 0,
  });
  assert.equal(below.start, 0);
  assert.equal(below.end, 0);
  assert.equal(below.padTop + below.padBottom, 20 * LIST_ROW_H + 19 * LIST_ROW_GAP);
});

// ---------------- 变高行（分组列表：标题 / 实例行 / 平铺卡片各一档） ----------------

const H = [48, 60, 60, 60, 120];
/** 全部渲染时的高度：各行之和 + 行间距 */
const FULL = H.reduce((sum, h) => sum + h, 0) + (H.length - 1) * 8;

test("变高行：可见区间包住视口，留白把隐藏行的高度精确补回来", () => {
  const win = visibleWindowOfHeights({
    scrollTop: 0,
    viewHeight: 100,
    rootTop: 0,
    heights: H,
    gap: 8,
    overscan: 0,
  });
  // 顶边 0 / 56 / 124 / 192 / 260：视口 0~100 只盖得住前两行
  assert.equal(win.start, 0);
  assert.equal(win.end, 2);
  assert.equal(win.padTop, 0);
  assert.equal(win.padBottom, FULL - (H[0] + 8 + H[1]));
});

test("变高行：滚到中间时上下留白都按前缀和算", () => {
  const win = visibleWindowOfHeights({
    scrollTop: 124,
    viewHeight: 100,
    rootTop: 0,
    heights: H,
    gap: 8,
    overscan: 0,
  });
  assert.equal(win.start, 2);
  assert.equal(win.end, 4);
  assert.equal(win.padTop, 124);
  // 下面藏着的是一行卡片加上它前面那道行距（每行的留白都含自己前面那道间距）
  assert.equal(win.padBottom, H[4] + 8);
  // 留白 + 渲染出来的行 + 行间距 == 全部渲染时的高度
  const rendered = H[2] + 8 + H[3];
  assert.equal(win.padTop + rendered + win.padBottom, FULL);
});

test("变高行：整列在视口上方 / 下方时一行都不渲染，高度仍然对得上", () => {
  const above = visibleWindowOfHeights({
    scrollTop: 100000,
    viewHeight: 500,
    rootTop: 0,
    heights: H,
    gap: 8,
    overscan: 0,
  });
  assert.equal(above.start, H.length);
  assert.equal(above.end, H.length);
  assert.equal(above.padTop, FULL);

  const below = visibleWindowOfHeights({
    scrollTop: 0,
    viewHeight: 500,
    rootTop: 100000,
    heights: H,
    gap: 8,
    overscan: 0,
  });
  assert.equal(below.start, 0);
  assert.equal(below.end, 0);
  assert.equal(below.padBottom, FULL);
});

test("变高行：空列表返回空窗口", () => {
  const win = visibleWindowOfHeights({
    scrollTop: 0,
    viewHeight: 500,
    rootTop: 0,
    heights: [],
    gap: 8,
  });
  assert.deepEqual(win, { start: 0, end: 0, padTop: 0, padBottom: 0 });
});
