import assert from "node:assert/strict";
import test from "node:test";
import { buildConfigTree, visibleConfigRows } from "../src/windows/config/configTree.ts";

const tree = buildConfigTree([
  "options.txt",
  "config/zeta.json",
  "config/nested/Target.json",
  "config/alpha.toml",
  "empty/",
]);

test("目录优先排序，折叠分支不显示子项", () => {
  assert.deepEqual(visibleConfigRows(tree, new Set(), "").map((row) => row.key), [
    "config", "empty", "options.txt",
  ]);
  assert.deepEqual(visibleConfigRows(tree, new Set(["config"]), "").map((row) => row.key), [
    "config", "config/nested", "config/alpha.toml", "config/zeta.json", "empty", "options.txt",
  ]);
});

test("搜索保留祖先并显示展开状态，清空搜索后恢复用户的折叠状态", () => {
  const expanded = new Set();
  const rows = visibleConfigRows(tree, expanded, " TARGET ");
  assert.deepEqual(rows.map(({ key, depth, expanded }) => ({ key, depth, expanded })), [
    { key: "config", depth: 0, expanded: true },
    { key: "config/nested", depth: 1, expanded: true },
    { key: "config/nested/Target.json", depth: 2, expanded: false },
  ]);
  assert.equal(expanded.size, 0);
  assert.equal(visibleConfigRows(tree, expanded, "").length, 3);
});

test("完整路径命中目录时保留其文件，空目录也可以搜索", () => {
  assert.deepEqual(visibleConfigRows(tree, new Set(), "config/").map((row) => row.key), [
    "config", "config/nested", "config/nested/Target.json", "config/alpha.toml", "config/zeta.json",
  ]);
  assert.deepEqual(visibleConfigRows(tree, new Set(), "empty").map((row) => row.key), ["empty"]);
  assert.deepEqual(visibleConfigRows(tree, new Set(), "missing"), []);
});

test("空白搜索与未搜索一致，匹配到文件不带出无关兄弟项", () => {
  const expanded = new Set(["config"]);
  assert.deepEqual(visibleConfigRows(tree, expanded, "  "), visibleConfigRows(tree, expanded, ""));
  assert.deepEqual(visibleConfigRows(tree, expanded, "alpha").map((row) => row.key), [
    "config", "config/alpha.toml",
  ]);
});
