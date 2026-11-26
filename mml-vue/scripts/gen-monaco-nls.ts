/* eslint-disable no-console */
// 生成 Monaco 的中文文案表：src/lib/monacoNls.zh.ts
//
// Monaco 的界面文案走 localize(<下标>, "英文原文", …)，npm 包不带任何语言包，
// 所以中文要靠我们挂到 globalThis._VSCODE_NLS_MESSAGES 上。下标会随版本变，
// 这里按"英文原文"对上号再写进表里，升级 monaco-editor 后重跑一次即可。
//
// 用法：cd mml-vue && npm run gen:nls
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** Monaco 源码里 localize / localize2 的调用 */
const CALL = /localize(?:2)?\(\s*(\d+)\s*,\s*("(?:[^"\\]|\\.)*")/g;

/** 英文原文 → 中文；键必须与 Monaco 源码里的英文逐字一致，多一个空格都对不上 */
const ZH: Record<string, string> = {
  // 查找框
  "Find / Replace": "查找 / 替换",
  Find: "查找",
  "Previous Match": "上一个匹配",
  "Next Match": "下一个匹配",
  "Find in Selection": "在选区中查找",
  Close: "关闭",
  Replace: "替换",
  "Replace All": "全部替换",
  "Toggle Replace": "切换替换",
  "Only the first {0} results are highlighted, but all find operations work on the entire text.":
    "只高亮前 {0} 个结果，全部查找操作仍在全文上生效。",
  "{0} of {1}": "{0} / {1}",
  "No results": "没有结果",
  "{0} found": "找到 {0} 个",
  "{0} found for '{1}'": "为 '{1}' 找到 {0} 个",
  "{0} found for '{1}', at {2}": "为 '{1}' 找到 {0} 个，位于 {2}",
  "Press {0} for accessibility help": "按 {0} 查看无障碍帮助",
  "{0}, {1}": "{0}，{1}",

  // 查找 / 替换的命令与提示
  "The file is too large to perform a replace all operation.": "文件太大，无法执行全部替换。",
  "&&Find": "&&查找",
  "&&Replace": "&&替换",
  "No matches. Try searching for something else.": "没有匹配项，换个词试试。",
  "Type a number to go to a specific match (between 1 and {0})": "输入数字跳到指定匹配项，范围 1 到 {0}",
  "Please type a number between 1 and {0}": "请输入 1 到 {0} 之间的数字",
  "Find with Arguments": "带参数的查找",
  "Find with Selection": "在选区中查找",
  "Find Next": "查找下一个",
  "Find Previous": "查找上一个",
  "Go to Match...": "跳到匹配项…",
  "Find Next Selection": "查找下一个选区",
  "Find Previous Selection": "查找上一个选区",

  // 补全框
  "Accepting '{0}' made {1} additional edits": "接受 '{0}' 时另做了 {1} 处修改",
  Insert: "插入",
  Select: "选择",
  "Show Less": "收起",
  "Show More": "展开",
  "Trigger Suggest": "触发补全",
  "Reset Suggest Widget Size": "重置补全框大小",
  "Loading...": "正在加载…",
  "No suggestions.": "没有建议。",
  Suggest: "建议",
  "{0}, docs: {1}": "{0}，文档：{1}",
};

const ROOT = "node_modules/monaco-editor/esm";
const OUT = "src/lib/monacoNls.zh.ts";

/** 还原 JS 字符串字面量里的转义 */
function unescape(text: string): string {
  return text.replace(/\\(.)/g, (_m, c: string) => (c === "n" ? "\n" : c === "t" ? "\t" : c));
}

/** 扫一遍 Monaco 的 ESM 源码，收下每处 localize 的下标与英文原文 */
function collect(): Map<number, string> {
  const found = new Map<number, string>();
  const conflicts: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) {
        walk(path);
        continue;
      }
      if (!name.endsWith(".js")) continue;
      const text = readFileSync(path, "utf8");
      for (const match of text.matchAll(CALL)) {
        const index = Number(match[1]);
        const english = unescape(match[2].slice(1, -1));
        const known = found.get(index);
        if (known === undefined) found.set(index, english);
        else if (known !== english) conflicts.push(`${index}: ${known} / ${english}`);
      }
    }
  };
  walk(ROOT);
  if (conflicts.length) {
    console.warn("同一个下标对应了不同文案，已取第一条：");
    for (const line of conflicts) console.warn("  " + line);
  }
  return found;
}

const found = collect();
const rows: Array<[number, string]> = [];
const used = new Set<string>();
for (const [index, english] of found) {
  // 用 hasOwn：英文原文可能是 constructor / toString 这类原型上的名字
  const chinese = Object.hasOwn(ZH, english) ? ZH[english] : undefined;
  if (chinese === undefined) continue;
  rows.push([index, chinese]);
  used.add(english);
}
rows.sort((a, b) => a[0] - b[0]);

const body = rows.map(([index, chinese]) => `  ${index}: ${JSON.stringify(chinese)},`).join("\n");
const file = `// 由 scripts/gen-monaco-nls.ts 生成，勿手改。
// 升级 monaco-editor 后重跑：cd mml-vue && npm run gen:nls
//
// 下标取自 Monaco 的 localize(<下标>, "英文原文", …)，中文按英文原文对号入座；
// 键值里保留 {0} {1} 这类占位符，由 Monaco 的 _format 按参数填。

/** 下标 → 中文 */
export const ZH_MESSAGES: Record<number, string> = {
${body}
};
`;
writeFileSync(OUT, file);

console.log(`生成 ${OUT}：${rows.length} 条`);
const missing = Object.keys(ZH).filter((english) => !used.has(english));
if (missing.length) {
  console.warn("这些英文在 Monaco 源码里没找到，可能已改名或删除：");
  for (const english of missing) console.warn("  " + english);
}
