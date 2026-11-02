// 实例下拉的分组：默认组兜底、认不出的归属不丢、空分组不画
import test from "node:test";
import assert from "node:assert/strict";
import { groupInstances, hasInstanceMeta } from "../src/lib/instancePicker.ts";

const groups = [
  { uuid: "default", name: "" },
  { uuid: "g1", name: "整合包" },
  { uuid: "g2", name: "空组" },
];

test("按分组表顺序分节，默认分组用调用方给的名字", () => {
  const sections = groupInstances(
    [
      { uuid: "a", name: "A", group: "g1" },
      { uuid: "b", name: "B", group: null },
      { uuid: "c", name: "C", group: "unknown" },
    ],
    groups,
    "默认分组",
  );
  assert.deepEqual(
    sections.map((section) => [section.id, section.name, section.items.map((item) => item.uuid)]),
    [
      ["default", "默认分组", ["b", "c"]],
      ["g1", "整合包", ["a"]],
    ],
  );
});

test("拿不到分组表时所有实例归到唯一的一节", () => {
  const sections = groupInstances(
    [
      { uuid: "a", name: "A", group: "g1" },
      { uuid: "b", name: "B" },
    ],
    [],
    "默认分组",
  );
  assert.equal(sections.length, 1);
  assert.equal(sections[0].name, "默认分组");
  assert.deepEqual(sections[0].items.map((item) => item.uuid), ["a", "b"]);
});

test("空分组不出现，组内保持传入顺序", () => {
  const sections = groupInstances(
    [
      { uuid: "b", name: "B", group: "g1" },
      { uuid: "a", name: "A", group: "g1" },
    ],
    groups,
    "默认分组",
  );
  assert.equal(sections.length, 1);
  assert.deepEqual(sections[0].items.map((item) => item.uuid), ["b", "a"]);
});

test("副标题只在有版本或非原版加载器时出现", () => {
  assert.equal(hasInstanceMeta({ uuid: "a", name: "A" }), false);
  assert.equal(hasInstanceMeta({ uuid: "a", name: "A", loader: "normal" }), false);
  assert.equal(hasInstanceMeta({ uuid: "a", name: "A", version: "1.21.1" }), true);
  assert.equal(hasInstanceMeta({ uuid: "a", name: "A", loader: "forge" }), true);
});
