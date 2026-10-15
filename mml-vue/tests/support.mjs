// 文本行级差异测试的公用件：逐字符断言、样本落盘、改动清单的拼装
//
// 放在单独一个文件里，供 textDiff.test.mjs（内存往返）与 textDiffSamples.test.mjs
// （生成样本文件那套流程）共用；`node --test "tests/**/*.test.mjs"` 不会把它当用例跑。

import fs from "node:fs";
import path from "node:path";

/** 样本落在测试目录下：跑完能直接在 `mml-vue/tests/` 里看到（已在 .gitignore 里排除） */
export const SAMPLE_DIR = import.meta.dirname;

/**
 * 把文本写进测试目录下的样本文件
 *
 * - `name`: 文件名，如 `sample-json.before.json`
 * - `text`: 内容
 *
 * # 返回值
 *
 * 写好的文件路径
 */
export function writeSample(name, text) {
  const file = path.join(SAMPLE_DIR, name);
  fs.writeFileSync(file, text);
  return file;
}

/**
 * 逐字符比对，第一处不一样就报出来
 *
 * 几 MB 的文本上 `assert.equal` 会把整份内容糊进失败信息，等于什么也没说；
 * 这里走到第一处不同就报字符下标、行号与两边的上下文。
 *
 * - `got`: 实际文本
 * - `want`: 期望文本
 * - `tag`: 用例名，报错时带上
 */
export function assertSameText(got, want, tag) {
  if (got === want) return;

  const min = Math.min(got.length, want.length);
  let index = 0;
  while (index < min && got[index] === want[index]) index += 1;

  /** 取第 `index` 个字符所在的那一行 */
  const lineAt = (text) => {
    const start = text.lastIndexOf("\n", index - 1) + 1;
    const end = text.indexOf("\n", index);
    const line = text.slice(0, start).split("\n").length;
    return `第 ${line} 行 ${JSON.stringify(text.slice(start, end === -1 ? text.length : end))}`;
  };

  throw new Error(
    `${tag}：第 ${index} 个字符起不一样（两边分别 ${got.length} / ${want.length} 个字符）\n` +
      `  实际：${lineAt(got)}\n  预期：${lineAt(want)}`,
  );
}

/** 造一份大文本的行：含空行、缩进、中文与 emoji */
export function bigLines(count) {
  const lines = [];
  for (let i = 0; i < count; i += 1) {
    switch (i % 7) {
      case 0:
        lines.push("");
        break;
      case 1:
        lines.push(`第 ${i} 行：中文与 emoji 🐟 混排`);
        break;
      case 2:
        lines.push(`\t缩进 ${i} = ${i * 31}`);
        break;
      default:
        lines.push(`line ${i} value=${(i * 7) % 1013}`);
    }
  }
  return lines;
}

/** 一处改动：把 [start, start+deleteCount) 换成 lines */
export function edit(start, deleteCount, lines) {
  return { start, deleteCount, lines };
}

/**
 * 按改动清单拼出"改之后"
 *
 * 倒着改（先动靠后的下标），前面的行号才不会被后面的改动带偏。
 */
export function applyOps(lines, ops) {
  const out = [...lines];
  for (const op of [...ops].sort((a, b) => b.start - a.start)) {
    out.splice(op.start, op.deleteCount, ...op.lines);
  }
  return out;
}

/**
 * 把差异渲染成人看的样子，落成 `.diff.txt`
 *
 * 行号按 1 起、相对"改之前"；每段给出删掉的行（`-`）与换上的行（`+`）。
 */
export function renderDiff(hunks, before) {
  const out = [`# 差异 ${hunks.length} 段；行号相对"改之前"，从 1 起`, ""];
  hunks.forEach((hunk, index) => {
    out.push(
      `@@ 第 ${index + 1} 段：第 ${hunk.start + 1} 行起，删 ${hunk.deleteCount} 行，加 ${hunk.lines.length} 行 @@`,
    );
    for (const line of before.slice(hunk.start, hunk.start + hunk.deleteCount)) out.push(`- ${line}`);
    for (const line of hunk.lines) out.push(`+ ${line}`);
    out.push("");
  });
  return out.join("\n");
}
