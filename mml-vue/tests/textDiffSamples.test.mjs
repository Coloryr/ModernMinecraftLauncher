// 行级差异的"看完再信"用例：在测试目录下生成样本文件，算差异、查差异合不合理、
// 再用差异把"改之前"改成"改之后"，最后逐字符比对
//
// 每种格式（txt / json / yaml / toml / js / xml）各生成四份，跑完就在 `mml-vue/tests/` 下：
//
// | 文件 | 内容 |
// | --- | --- |
// | `sample-<格式>.before.<ext>` | "改之前"，几千行 |
// | `sample-<格式>.after.<ext>` | "改之后"，由同一串改动拼出来 |
// | `sample-<格式>.diff.txt` | 算出来的差异，人看的样子（`-` 删 / `+` 加，行号 1 起） |
// | `sample-<格式>.result.<ext>` | 用差异改"改之前"得到的结果，应当与 `.after` 逐字节相同 |
//
// 这几份都是每次跑用例时现生成的，`.gitignore` 里已排除，不入库。
import assert from "node:assert/strict";
import test from "node:test";
import { applyHunks, diffLines, joinLines } from "../src/lib/textDiff.ts";
import { applyOps, assertSameText, edit, renderDiff, writeSample } from "./support.mjs";

/** 每种格式生成多少行 */
const LINES = 4_000;

/**
 * 各格式的样本：`before` 现生成，`edits` 给出改动清单（位置相对"改之前"）
 *
 * 改动都挑在**互不相邻、内容唯一**的位置上，且每一段"换成什么"都不与它换掉的那几行首尾相同：
 * 这样差异段与改动清单就该一一对应，人扫 `.diff.txt` 时看到的也正是这几处改动。
 * （行内容全带下标，没有重复行，避开"删掉的那行别处也有一模一样的"这种位置歧义。）
 */
const FORMATS = [
  {
    name: "txt",
    ext: "txt",
    before: (count) =>
      Array.from(
        { length: count },
        (_, i) => `第 ${i} 行：中文与 emoji 🐟 混排，值 = ${(i * 31) % 997}`,
      ),
    edits: (count) => [
      edit(0, 0, ["# 头插两行", "# 第二行"]),
      edit(Math.floor(count * 0.4), 2, ["# 把这两行换成一行"]),
      edit(Math.floor(count * 0.7), 3, []),
      edit(count - 1, 1, ["# 尾行换成这一行"]),
    ],
  },
  {
    name: "json",
    ext: "json",
    before: (count) => {
      const lines = ["{", '  "name": "sample-config",', '  "entries": ['];
      for (let i = 3; i < count - 2; i += 1) {
        lines.push(
          `    { "id": ${i - 3}, "name": "item_${i - 3}", "enabled": ${i % 3 !== 0}, "weight": ${(i * 7) % 100} },`,
        );
      }
      lines.push("  ],", "}");
      return lines;
    },
    edits: (count) => [
      edit(3, 1, ['    { "id": 9001, "name": "改过的条目", "enabled": false, "weight": 42 },']),
      edit(Math.floor(count * 0.4), 0, [
        '    { "id": 9002, "name": "插入的条目 A", "enabled": true, "weight": 1 },',
        '    { "id": 9003, "name": "插入的条目 B", "enabled": true, "weight": 2 },',
      ]),
      edit(Math.floor(count * 0.7), 3, []),
      edit(count - 1, 0, ['  "version": 4']),
    ],
  },
  {
    name: "yaml",
    ext: "yaml",
    before: (count) => {
      const lines = ["name: sample-config", "items:"];
      let id = 0;
      while (lines.length + 3 <= count - 1) {
        lines.push(`  - id: ${id}`, `    name: item_${id}`, `    enabled: ${id % 3 !== 0}`);
        id += 1;
      }
      while (lines.length < count - 1) lines.push(`# 填充 ${lines.length}`);
      lines.push("version: 3");
      return lines;
    },
    edits: (count) => [
      edit(3, 1, ["    name: 改过的条目"]),
      edit(Math.floor(count * 0.4), 0, ["  - id: 9002", "    name: 插入的条目"]),
      edit(Math.floor(count * 0.7), 2, []),
      edit(count - 1, 1, ["version: 4"]),
    ],
  },
  {
    name: "toml",
    ext: "toml",
    before: (count) => {
      const lines = ["# 样本配置", 'name = "sample-config"'];
      let id = 0;
      while (lines.length + 4 <= count - 1) {
        lines.push("", "[[items]]", `id = ${id}`, `name = "item_${id}"`);
        id += 1;
      }
      while (lines.length < count - 1) lines.push(`# 填充 ${lines.length}`);
      lines.push("version = 3");
      return lines;
    },
    edits: (count) => [
      edit(4, 2, ["id = 9001", 'name = "改过的条目"', "weight = 42"]),
      edit(Math.floor(count * 0.4) + 2, 0, ['name = "插入的条目 A"', 'name = "插入的条目 B"']),
      edit(Math.floor(count * 0.7), 2, []),
      edit(count - 1, 1, ["version = 4"]),
    ],
  },
  {
    name: "js",
    ext: "js",
    before: (count) => {
      const lines = ["// 样本脚本", "export const items = ["];
      let id = 0;
      while (lines.length + 1 <= count - 3) {
        lines.push(`  { id: ${id}, name: "item_${id}", enabled: ${id % 3 !== 0} },`);
        id += 1;
      }
      while (lines.length < count - 3) lines.push(`  // 填充 ${lines.length}`);
      lines.push("];", "", "export const version = 3;");
      return lines;
    },
    edits: (count) => [
      edit(2, 1, ['  { id: 9001, name: "改过的条目", enabled: false, weight: 42 },']),
      edit(Math.floor(count * 0.4), 0, [
        '  { id: 9002, name: "插入的条目 A", enabled: true },',
        '  { id: 9003, name: "插入的条目 B", enabled: true },',
      ]),
      edit(Math.floor(count * 0.7), 3, []),
      edit(count - 1, 1, ["export const version = 4;"]),
    ],
  },
  {
    name: "xml",
    ext: "xml",
    before: (count) => {
      const lines = ['<?xml version="1.0" encoding="UTF-8"?>', '<items version="3">'];
      let id = 0;
      while (lines.length + 1 <= count - 1) {
        lines.push(`  <item id="${id}" name="item_${id}" enabled="${id % 3 !== 0}" />`);
        id += 1;
      }
      while (lines.length < count - 1) lines.push(`  <!-- 填充 ${lines.length} -->`);
      lines.push("</items>");
      return lines;
    },
    edits: (count) => [
      edit(1, 1, ['<items version="4">']),
      edit(Math.floor(count * 0.4), 0, [
        '  <item id="9002" name="插入的条目 A" enabled="true" />',
        '  <item id="9003" name="插入的条目 B" enabled="true" />',
      ]),
      edit(Math.floor(count * 0.7), 3, []),
      edit(count - 2, 1, ['  <item id="9001" name="改过的条目" enabled="false" />']),
    ],
  },
];

/**
 * 查差异本身合不合理
 *
 * - **段与预期改动一一对应**（起点、删几行、换成什么都要对得上）：既没把一处改动切碎，
 *   也没把几处合成一处；
 * - 段按起点升序、互不重叠（后端就是倒着套的，顺序错了会改错地方）；
 * - 每段都真的改了东西（不会出现"删 0 行加 0 行"的空段）；
 * - 每段两侧的边界是"改动与未改动的交界"：段前一行、段后一行在两边相同 ——
 *   差异没有把没变的行圈进来，也没有该并进这一段的没并进来。
 *
 * - `hunks`: 实际算出来的差异
 * - `before` / `after`: 两侧的行列表
 * - `ops`: 用例自己做的改动清单
 */
function assertDiffSane(hunks, before, after, ops) {
  const sorted = [...ops].sort((a, b) => a.start - b.start);
  assert.equal(hunks.length, sorted.length, `差异段数应为 ${sorted.length}，实际 ${hunks.length}`);

  // 逐段与预期改动对齐
  hunks.forEach((hunk, index) => {
    const want = sorted[index];
    assert.equal(hunk.start, want.start, `第 ${index + 1} 段起点`);
    assert.equal(hunk.deleteCount, want.deleteCount, `第 ${index + 1} 段删掉的行数`);
    assert.deepEqual(hunk.lines, want.lines, `第 ${index + 1} 段换上的行`);
  });

  // 升序、不重叠、非空
  let cursor = 0;
  hunks.forEach((hunk, index) => {
    assert.ok(hunk.start >= cursor, `第 ${index + 1} 段起点应不小于上一段末尾`);
    assert.ok(
      hunk.deleteCount > 0 || hunk.lines.length > 0,
      `第 ${index + 1} 段什么都没改，不该出现在差异里`,
    );
    cursor = hunk.start + hunk.deleteCount;
  });

  // 边界：段前一行与段后一行在两侧相同（差异的边界就是"变与没变"的交界）
  let shift = 0;
  hunks.forEach((hunk, index) => {
    const afterStart = hunk.start + shift;
    const beforePrev = before[hunk.start - 1];
    const afterPrev = after[afterStart - 1];
    if (beforePrev !== undefined && afterPrev !== undefined) {
      assert.equal(beforePrev, afterPrev, `第 ${index + 1} 段前一行应当没变`);
    }
    const beforeNext = before[hunk.start + hunk.deleteCount];
    const afterNext = after[afterStart + hunk.lines.length];
    if (beforeNext !== undefined && afterNext !== undefined) {
      assert.equal(beforeNext, afterNext, `第 ${index + 1} 段后一行应当没变`);
    }
    shift += hunk.lines.length - hunk.deleteCount;
  });
}

for (const format of FORMATS) {
  test(`样本文件：${format.name} —— 生成改前改后、算差异、查合理、用差异改回去`, () => {
    const before = format.before(LINES);
    const ops = format.edits(LINES);
    const after = applyOps(before, ops);
    assert.equal(before.length, LINES, `${format.name} 的样本行数应当是 ${LINES}`);

    // 1. 算差异
    const hunks = diffLines(before, after);

    // 2. 落盘：改之前 / 改之后 / 差异 / 用差异改出来的结果
    const beforeFile = writeSample(`sample-${format.name}.before.${format.ext}`, joinLines(before));
    const afterFile = writeSample(`sample-${format.name}.after.${format.ext}`, joinLines(after));
    const diffFile = writeSample(`sample-${format.name}.diff.txt`, renderDiff(hunks, before));
    const result = joinLines(applyHunks(before, hunks));
    const resultFile = writeSample(`sample-${format.name}.result.${format.ext}`, result);
    for (const file of [beforeFile, afterFile, diffFile, resultFile]) {
      assert.ok(file.endsWith(`.${format.ext}`) || file.endsWith(".diff.txt"), `${file} 文件名不对`);
    }

    // 3. 差异本身合理
    assertDiffSane(hunks, before, after, ops);

    // 4. 用差异改"改之前"，结果与"改之后"逐字符一致
    assertSameText(result, joinLines(after), `${format.name} 用差异改回去`);

    // 5. 反向也成立：拿"改之后"当原文，差异套回去应当还原"改之前"
    const reverse = joinLines(applyHunks(after, diffLines(after, before)));
    assertSameText(reverse, joinLines(before), `${format.name} 反向`);
  });
}

test("样本文件：差异为空时不生成任何改动", () => {
  const before = FORMATS[0].before(200);
  const hunks = diffLines(before, [...before]);
  assert.equal(hunks.length, 0);
  assertSameText(joinLines(applyHunks(before, hunks)), joinLines(before), "无改动");
});
