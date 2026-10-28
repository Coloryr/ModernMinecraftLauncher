// 生成 docs/screenshots.md：场景表来自 scenarios.mjs，中文说明来自 catalog.mjs。
//
// 用法：cd mml-vue && npm run shots:docs
/* eslint-disable no-console */
import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GROUPS, describe, groupOf, markdownTable } from "./catalog.mjs";
import { scenarios } from "./scenarios.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = resolve(HERE, "../../../docs/screenshots.md");

const grouped = new Map(GROUPS.map((group) => [group.key, []]));
for (const scenario of scenarios) {
  const key = groupOf(scenario.id);
  if (!grouped.has(key)) grouped.set(key, []);
  grouped.get(key).push(scenario);
}

const lines = [
  "# 前端页面截图目录",
  "",
  "`mml-vue` 把每个窗口与状态做成一个固定截图场景，另有一组控件预览页（`control-*`）。",
  "每个场景都有独立名字，可以只渲染其中一张，不需要等整批跑完。",
  "",
  "## 用法",
  "",
  "```bash",
  "cd mml-vue",
  "",
  "# 只跑一张：名字就是下面的「图片」列",
  "npm run shots -- --scene=main-grid",
  "npm run shots -- main-grid",
  "",
  "# 一次跑多张",
  "npm run shots -- --scene=main-grid,settings-ui",
  "",
  "# 按前缀成组跑",
  "npm run shots -- --only=settings",
  "npm run shots -- --only=control",
  "",
  "# 看全部名字 / 输出 Markdown 表",
  "npm run shots -- --list",
  "npm run shots -- --list=md",
  "",
  "# 清空旧截图后整批重跑",
  "npm run shots -- --clean",
  "```",
  "",
  "只跑部分场景时，输出目录里的其他截图会保留，可以逐张补齐；`--clean` 才会先清空。",
  "浏览器默认用 Windows Edge，其他平台用 `--edge=<路径>` 指定；端口默认 1420，可用 `--port=` 改。",
  "更多细节见 [`mml-vue/scripts/screenshot/README.md`](../mml-vue/scripts/screenshot/README.md)。",
  "",
  "产物：",
  "",
  "- `target/screens/<图片名>.png`：每个场景一张，视口见下表；",
  "- `target/screens/index.html`：本次与历史截图的索引页；",
  "- `target/screens/report.json`：失败原因、页面错误、未 mock 的命令。",
  "",
  "## 场景表",
  "",
  "共 " + scenarios.length + " 个场景，按窗口分组。",
  "「窗口」列是页面的 `kind`，窗口族场景会自动带上实例参数，不需要在命令里另写；",
  "控件场景的 `control` 是截图工具里的预览页，不是启动器窗口。",
  "",
];

for (const group of GROUPS) {
  const items = grouped.get(group.key) ?? [];
  if (!items.length) continue;
  lines.push("### " + group.title, "", markdownTable(items, describe), "");
}

writeFileSync(OUT, lines.join("\n"), "utf8");
console.log("[docs] " + OUT + "（" + scenarios.length + " 个场景）");
