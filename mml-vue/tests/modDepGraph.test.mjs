// 依赖图的回归测试：`node --test` 直接跑（Node 能抹掉 TS 类型，所以能直接 import 源码）
//
// 盯的是建图这一段：同一个前置被多条路径共用、绕回来成环、声明写重了，
// 以及"装没装 / 版本合不合要求"的口径。布局交给 dagre，这里只查坐标有没有算出来、
// 画布有没有把节点包住。

import test from "node:test";
import assert from "node:assert/strict";
import {
  NODE_H,
  NODE_W,
  buildDepGraph,
} from "../src/windows/resource/composables/modDepGraph.ts";

/**
 * 造一个模组条目，只填建图用得到的字段
 *
 * - `modId`: modid，同时用作文件名
 * - `deps`: 它声明的前置
 */
function mod(modId, deps = []) {
  return {
    uuid: modId + "-uuid",
    sha1: modId + "-sha1",
    file: modId + ".jar",
    modId,
    name: modId,
    version: "1.0",
    icon: "",
    disable: false,
    fail: false,
    jarInJar: [],
    dependencies: deps,
  };
}

/**
 * 一条前置声明
 *
 * - `modId`: 前置的 modid
 * - `optional`: 是不是推荐安装
 * - `required`: 版本要求
 */
function dep(modId, optional = false, required = "*") {
  return { modId, optional, required };
}

/** modid 小写 → 提供它的条目 */
function providerMap(items) {
  const map = new Map();
  for (const item of items) map.set(item.modId.toLowerCase(), [{ item, nested: false }]);
  return map;
}

/** 节点键集合，方便断言"出现且只出现一次" */
function keysOf(graph) {
  return graph.nodes.map((node) => node.key).sort();
}

/** 取一个节点 */
function nodeOf(graph, key) {
  return graph.nodes.find((node) => node.key === key);
}

test("同一个前置被两条路径共用：只画一个节点，两条边汇过去", () => {
  const c = mod("C");
  const a = mod("A", [dep("C", false, "[1.0,)")]);
  const b = mod("B", [dep("C")]);
  const root = mod("Root", [dep("A"), dep("B")]);

  const graph = buildDepGraph(root, providerMap([root, a, b, c]), () => null);

  assert.deepEqual(keysOf(graph), ["a", "b", "c", "root"]);
  // A → C 与 B → C 都在，目标都是同一个节点
  assert.equal(graph.edges.filter((edge) => edge.to === "c").length, 2);
  assert.equal(graph.edges.filter((edge) => edge.backward).length, 0);
  // 三条都装着，标记该打勾
  for (const key of ["a", "b", "c"]) assert.equal(nodeOf(graph, key).installed, true);
});

test("环：回边标出来且不再展开，节点不重复", () => {
  const a = mod("A", [dep("Root")]);
  const root = mod("Root", [dep("A")]);

  const graph = buildDepGraph(root, providerMap([root, a]), () => null);

  assert.deepEqual(keysOf(graph), ["a", "root"]);
  const backward = graph.edges.filter((edge) => edge.backward);
  assert.equal(backward.length, 1);
  assert.equal(backward[0].from, "a");
  assert.equal(backward[0].to, "root");
});

test("模组自己依赖自己：只留下一条回边", () => {
  const root = mod("Root", [dep("Root")]);
  const graph = buildDepGraph(root, providerMap([root]), () => null);

  assert.deepEqual(keysOf(graph), ["root"]);
  assert.equal(graph.edges.length, 1);
  assert.equal(graph.edges[0].backward, true);
});

test("同一条声明写两遍：只连一条边", () => {
  const c = mod("C");
  const root = mod("Root", [dep("C"), dep("C")]);
  const graph = buildDepGraph(root, providerMap([root, c]), () => null);

  assert.equal(graph.edges.filter((edge) => edge.to === "c").length, 1);
});

test("缺失只认强制前置：可选前置没装不算缺失，虚拟前置也不算", () => {
  const root = mod("Root", [dep("Missing"), dep("Optional", true), dep("Minecraft")]);
  const graph = buildDepGraph(root, providerMap([root]), (id) =>
    id === "minecraft" ? "1.20.4" : null,
  );

  const missing = nodeOf(graph, "missing");
  const optional = nodeOf(graph, "optional");
  const virtual = nodeOf(graph, "minecraft");

  assert.equal(missing.missing, true);
  assert.equal(missing.installed, false);
  // 必装 / 可选：节点边框按它配色
  assert.equal(missing.mandatory, true);
  assert.equal(optional.mandatory, false);
  assert.equal(optional.missing, false);
  assert.equal(virtual.missing, false);
  assert.equal(virtual.virtual, true);
  // 游戏 / 加载器提供的也算"装了"
  assert.equal(virtual.installed, true);
  assert.equal(virtual.version, "1.20.4");
  // 可选那条边要标出来，图里画成点线
  assert.equal(graph.edges.find((edge) => edge.to === "optional").optional, true);
});

test("反着只画一层：谁直接依赖它，那一级自己的上游不再展开", () => {
  const root = mod("Root");
  const parent = mod("Parent", [dep("Root")]);
  const grand = mod("Grand", [dep("Parent")]);
  const dependents = new Map([["root", [{ item: parent, dep: dep("Root") }]]]);

  const graph = buildDepGraph(
    root,
    providerMap([root, parent, grand]),
    () => null,
    dependents,
  );

  // Parent 画进来了；Parent 自己的上游 Grand 不画（只到上一级）
  assert.deepEqual(keysOf(graph), ["parent", "root"]);
  const edge = graph.edges.find((each) => each.from === "parent" && each.to === "root");
  assert.ok(edge, "应当有一条 Parent → Root 的边");
  assert.equal(edge.optional, false);
  assert.equal(nodeOf(graph, "parent").installed, true);
});

test("反过来那条边照样分可选 / 必装", () => {
  const root = mod("Root");
  const soft = mod("Soft", [dep("Root", true)]);
  const dependents = new Map([["root", [{ item: soft, dep: dep("Root", true) }]]]);

  const graph = buildDepGraph(root, providerMap([root, soft]), () => null, dependents);

  const edge = graph.edges.find((each) => each.from === "soft" && each.to === "root");
  assert.equal(edge.optional, true);
});

test("加载器 → minecraft：两边都在图上时补一条黄线", () => {
  const root = mod("Root", [dep("Minecraft"), dep("MinecraftForge")]);
  const graph = buildDepGraph(
    root,
    providerMap([root]),
    (id) => (id === "minecraft" ? "1.20.4" : id === "minecraftforge" ? "47.2.0" : null),
    new Map(),
    new Set(["forge", "minecraftforge"]),
  );

  const link = graph.edges.find((edge) => edge.from === "minecraftforge" && edge.to === "minecraft");
  assert.ok(link, "应当补一条 加载器 → minecraft 的边");
  assert.equal(link.loader, true);
  // 模组自己声明的那条边不能带上这个标记
  assert.equal(graph.edges.find((edge) => edge.from === "root" && edge.to === "minecraft").loader, false);
});

test("加载器 → minecraft：只声明了 minecraft 时不凭空补出加载器", () => {
  const root = mod("Root", [dep("Minecraft")]);
  const graph = buildDepGraph(
    root,
    providerMap([root]),
    (id) => (id === "minecraft" ? "1.20.4" : null),
    new Map(),
    new Set(["forge"]),
  );

  assert.deepEqual(keysOf(graph), ["minecraft", "root"]);
  assert.equal(graph.edges.some((edge) => edge.loader), false);
});

test("布局：每个节点都有坐标，画布把它们都包住", () => {
  const b = mod("B", [dep("D")]);
  const c = mod("C", [dep("D")]);
  const root = mod("Root", [dep("A"), dep("B"), dep("C"), dep("D")]);

  const graph = buildDepGraph(root, providerMap([root, mod("A"), b, c, mod("D")]), () => null);

  assert.ok(graph.width > 0 && graph.height > 0, "画布应当有大小");
  for (const node of graph.nodes) {
    assert.ok(Number.isFinite(node.x) && Number.isFinite(node.y), node.key + " 没有坐标");
    assert.ok(node.x >= 0 && node.y >= 0, node.key + " 跑到画布外了");
    assert.ok(node.x + NODE_W <= graph.width, node.key + " 超出画布宽");
    assert.ok(node.y + NODE_H <= graph.height, node.key + " 超出画布高");
  }
});
