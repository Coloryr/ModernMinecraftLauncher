// 从 mml-vue/ui-kit 的控件清单里挑控件，生成 CSS 覆盖骨架
//
// 用法（Node 24 直接跑 .ts）：
//   node scripts/controls.ts                  列出全部控件
//   node scripts/controls.ts --filter btn     只看名字里带 btn 的
//   node scripts/controls.ts --variables      列出可用的 CSS 变量
//   node scripts/controls.ts --pick           交互选择控件，输出 CSS 骨架
//
// 换实例启动按钮的完整示例见 src/replace.ts（运行时入口，npm run pack 时编译成 custom.js）。
// 清单由mml-vue 的 scripts/gen-ui.ts 从 mml-vue 生成（mml-vue/src/generated/controls.ts），不在这里手抄。
// 改 mml-vue 的控件后重跑 `node mml-vue/scripts/gen-ui.ts` 即可。

import { createInterface } from "node:readline/promises";
import { CONTROLS, CSS_VARIABLES, type ControlEntry } from "mml-vue/ui-kit/controls";

/** 取 --name 后面的参数值 */
function argOf(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

/** 命令行里有没有这个开关 */
function hasFlag(name: string): boolean {
  return process.argv.includes(name);
}

const filter = (argOf("--filter") ?? "").toLowerCase();

/** 按分类分组，组内按类名排；分组的顺序也稳定 */
function grouped(): Array<[string, ControlEntry[]]> {
  const map = new Map<string, ControlEntry[]>();
  for (const item of CONTROLS) {
    if (!item.className.toLowerCase().includes(filter)) continue;
    const list = map.get(item.category) ?? [];
    list.push(item);
    map.set(item.category, list);
  }
  for (const list of map.values()) list.sort((a, b) => a.className.localeCompare(b.className));
  return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0]));
}

/** 定义文件只显示第一个，够用来定位 */
function sourceOf(item: ControlEntry): string {
  return item.sources[0] ?? "";
}

/** 打印控件清单 */
function printList() {
  const groups = grouped();
  const total = groups.reduce((n, [, list]) => n + list.length, 0);
  if (total === 0) {
    console.log("没有匹配的控件：换个 --filter，或去掉过滤");
    return;
  }
  console.log(total + " 个控件（来自 mml-vue/ui-kit）\n");
  for (const [category, list] of groups) {
    console.log("【" + category + "】");
    for (const item of list) {
      console.log("  ." + item.className.padEnd(26) + sourceOf(item));
    }
    console.log("");
  }
}

/** 打印变量表 */
function printVariables() {
  console.log(CSS_VARIABLES.length + " 个变量（来自 mml-vue/ui-kit）\n");
  for (const item of CSS_VARIABLES) {
    console.log("  " + item.name.padEnd(24) + item.description);
  }
}

/** 交互选控件，输出可以直接粘进 custom.css 的骨架 */
async function pick() {
  if (!process.stdin.isTTY) {
    console.error("✗ --pick 需要交互终端；先用 --list 看清单，或把要改的类名写进 custom.css");
    process.exit(1);
  }

  const flat: ControlEntry[] = [];
  for (const [, list] of grouped()) flat.push(...list);
  if (flat.length === 0) {
    console.log("没有匹配的控件：换个 --filter，或去掉过滤");
    return;
  }

  console.log("选择一个或多个控件，输入编号（逗号分隔）：\n");
  flat.forEach((item, index) => {
    console.log("  " + String(index + 1).padStart(3) + ". ." + item.className + "  【" + item.category + "】");
  });

  const rl = createInterface({ input: process.stdin, output: process.stdout });
  const answer = (await rl.question("\n编号（回车取消）：")).trim();
  rl.close();
  if (!answer) return;

  const picked: ControlEntry[] = [];
  for (const part of answer.split(",")) {
    const index = Number(part.trim());
    if (!Number.isInteger(index) || index < 1 || index > flat.length) continue;
    const item = flat[index - 1];
    if (item && !picked.includes(item)) picked.push(item);
  }
  if (picked.length === 0) {
    console.error("没有合法的编号");
    process.exit(1);
  }

  console.log("\n/* 从 mml-vue/ui-kit 控件清单生成，按需删改 */");
  for (const item of picked) {
    console.log("\n/* " + item.category + " · " + sourceOf(item) + " */");
    console.log("." + item.className + " {");
    console.log("  /* 在这里写覆盖样式 */");
    console.log("}");
  }
  console.log("");
}

if (hasFlag("--variables")) {
  printVariables();
} else if (hasFlag("--pick")) {
  await pick();
} else {
  printList();
}
