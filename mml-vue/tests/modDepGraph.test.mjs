// 依赖图**布局**的回归：关系（谁依赖谁、缺没缺、装没装）由后端算，见 AGENTS.md §12.1，
// 前端只剩"把后端给的图交给 dagre"。所以这里只盯两件事：每个节点都拿到坐标，
// 画布把所有节点都包住；建图本身的口径在内核侧（mml-gui 的 windows/resource/dep_tree.rs）。

import test from "node:test";
import assert from "node:assert/strict";
import {
  NODE_H,
  NODE_W,
  layoutDepGraph,
} from "../src/windows/resource/composables/modDepGraph.ts";

/**
 * 造一个后端返回的节点
 *
 * - `key`: 节点键，同时当显示名
 */
function node(key) {
  return {
    key,
    label: key,
    modId: key,
    version: "1.0",
    icon: "",
    root: false,
    nested: false,
    virtual: false,
    installed: true,
    missing: false,
    disabled: false,
    mandatory: false,
    required: [],
    problem: null,
  };
}

/**
 * 造一条后端返回的连线
 *
 * - `from`: 谁需要
 * - `to`: 需要谁
 */
function edge(from, to) {
  return {
    id: from + ">" + to,
    from,
    to,
    optional: false,
    required: "*",
    backward: false,
    loader: false,
  };
}

test("布局：每个节点都有坐标，画布把它们都包住", () => {
  const tree = {
    nodes: [node("root"), node("a"), node("b"), node("c"), node("d")],
    edges: [
      edge("root", "a"),
      edge("root", "b"),
      edge("root", "c"),
      edge("root", "d"),
      edge("b", "d"),
      edge("c", "d"),
    ],
  };

  const graph = layoutDepGraph(tree);

  assert.equal(graph.nodes.length, 5);
  assert.ok(graph.width > 0 && graph.height > 0, "画布应当有大小");
  for (const item of graph.nodes) {
    assert.ok(Number.isFinite(item.x) && Number.isFinite(item.y), item.key + " 没有坐标");
    assert.ok(item.x >= 0 && item.y >= 0, item.key + " 跑到画布外了");
    assert.ok(item.x + NODE_W <= graph.width, item.key + " 超出画布宽");
    assert.ok(item.y + NODE_H <= graph.height, item.key + " 超出画布高");
  }
});

test("布局不改后端给的关系：节点与连线原样带出来", () => {
  const tree = { nodes: [node("root"), node("api")], edges: [edge("root", "api")] };

  const graph = layoutDepGraph(tree);

  assert.deepEqual(graph.edges, tree.edges);
  assert.deepEqual(
    graph.nodes.map((item) => item.key).sort(),
    ["api", "root"],
  );
});
