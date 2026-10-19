// 文本行级差异的回归测试：`node --test` 直接跑（Node 能抹掉 TS 类型，所以能直接 import 源码）
//
// 这一侧负责**算**差异（Monaco 的 LcsDiff），后端负责**套**差异（`mml-sys::text_diff`），
// 两边各有大文本用例，比的都是**逐字符**一致：几 MB 的输入上用 assert.equal，
// 失败信息会把整份内容糊出来，等于什么也没说。
//
// 每条用例的期望文本都由测试自己按同一串编辑独立拼出来，不借 applyHunks —— 那样等于自己验自己。
// 生成样本文件、查差异合理性那套流程在 textDiffSamples.test.mjs。
import test from "node:test";
import { applyHunks, diffLines, joinLines } from "../src/lib/textDiff.ts";
import { assertSameText, bigLines } from "./support.mjs";

/**
 * 把原文与编辑后的行算成差异，再套回原文，逐字符比对
 *
 * - `origin`: 原文行列表
 * - `edited`: 编辑后的行列表
 * - `tag`: 用例名
 */
function checkRoundTrip(origin, edited, tag) {
  const hunks = diffLines(origin, edited);

  // 差异必须按原文升序、互不重叠：后端是倒着套的，顺序错了就会改错地方
  for (let i = 1; i < hunks.length; i += 1) {
    const prev = hunks[i - 1];
    const cur = hunks[i];
    if (cur.start < prev.start + prev.deleteCount) {
      throw new Error(`${tag}：差异顺序不对 ${JSON.stringify(hunks)}`);
    }
  }

  const back = joinLines(applyHunks(origin, hunks));
  assertSameText(back, joinLines(edited), tag);

  // 反向也要成立：拿编辑后的当原文，差异套回去应当还原出原文
  const reverse = joinLines(applyHunks(edited, diffLines(edited, origin)));
  assertSameText(reverse, joinLines(origin), `${tag}（反向）`);
}

test("大文本：头插、中替、中删、尾追加，差异套回原文逐字符一致", () => {
  const origin = bigLines(50_000);
  const edited = origin.slice();
  // 倒着改，前面的行号才不会被后面的改动带偏
  edited.splice(50_000, 0, "# 尾追加");
  edited.splice(30_000, 20);
  edited.splice(12_345, 5, "# 中部替换 A", "# 中部替换 B");
  edited.splice(0, 0, "# 头插一行");
  checkRoundTrip(origin, edited, "大文本多处改动");
});

test("大文本：整份替换（一段盖住全文）", () => {
  const origin = bigLines(20_000);
  const edited = origin.map((line) => `# ${line}`);

  // 整份替换是 Myers 最费的一档（改动数 ≈ 行数）。这里同时钉住"算不完就整段替换"那条短路：
  // 没有它这条用例要跑好几秒，界面在这期间是卡住的
  const started = Date.now();
  checkRoundTrip(origin, edited, "大文本整份替换");
  const elapsed = Date.now() - started;
  if (elapsed > 2000) throw new Error(`整份替换花了 ${elapsed}ms，步数上限没生效`);
});

test("大文本：没有改动时差异为空", () => {
  const origin = bigLines(20_000);
  const hunks = diffLines(origin, origin.slice());
  if (hunks.length !== 0) throw new Error(`没有改动却有 ${hunks.length} 段差异`);
  assertSameText(joinLines(applyHunks(origin, hunks)), joinLines(origin), "大文本无改动");
});

test("大文本：1 MB 的单行替换，前后两行不动", () => {
  const origin = ["头", "x".repeat(1024 * 1024), "尾"];
  const edited = ["头", `y${"z".repeat(1024 * 1024)}`, "尾"];
  checkRoundTrip(origin, edited, "超长单行");
});

test("大文本：两百处小改动", () => {
  const origin = bigLines(20_000);
  const edited = origin.slice();
  const starts = [];
  for (let i = 0; i < 200; i += 1) starts.push(i * 100);
  for (const start of [...starts].reverse()) {
    edited.splice(start, 1, `# 改过的第 ${start} 行`);
  }
  checkRoundTrip(origin, edited, "多处小改动");
});

test("大文本：CRLF 与空行混排", () => {
  const origin = [];
  for (let i = 0; i < 5_000; i += 1) origin.push(i % 5 === 0 ? "" : `行 ${i}\r`);
  const edited = origin.slice();
  edited.splice(1_000, 3, "改过的\r", "", "又一行\r");
  checkRoundTrip(origin, edited, "CRLF 与空行");
});

test("边界：空文件、单行、只加一个换行", () => {
  checkRoundTrip([""], ["", ""], "空文件加一行");
  checkRoundTrip([""], ["a"], "空文件写内容");
  checkRoundTrip(["a"], [""], "清空内容");
  checkRoundTrip(["a", "b", ""], ["a", "b"], "去掉末尾空行");
  checkRoundTrip(["a", "b"], ["a", "b", ""], "补上末尾空行");
});
