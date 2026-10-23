// 在真实组件函数上控制 IPC 返回顺序，不启动窗口。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../src/windows/export/ExportWindow.vue", import.meta.url), "utf8")
  .split('<script setup lang="ts">')[1].split("</script>")[0];
const parsed = ts.createSourceFile("ExportWindow.ts", source, ts.ScriptTarget.Latest, true);
const statements = parsed.statements.filter((node) =>
  (ts.isFunctionDeclaration(node) && ["loadInfo", "applyDefaults"].includes(node.name?.text))
  || node.getText(parsed).startsWith("let infoRequest ="));
const js = ts.transpileModule(statements.map((node) => node.getText(parsed)).join("\n"),
  { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

function fixture(api) {
  const refs = Object.fromEntries(Object.entries({ currentUuid: "a", info: null, loadingInfo: false,
    packName: "", summary: "", pickedMods: new Map(), pickedFiles: new Map(), urls: new Map() })
    .map(([key, value]) => [key, { value }]));
  const errors = [];
  const context = { ...refs, api, DEFAULT_DIRS: ["config", "resourcepacks", "shaderpacks"], showToast: (e) => errors.push(e), tErr: String };
  const loadInfo = new Function(...Object.keys(context), `${js}\nreturn loadInfo;`)(...Object.values(context));
  return { ...refs, loadInfo, errors };
}

test("旧实例信息迟到不能覆盖当前实例，也不能提前解除加载状态", async () => {
  const a = deferred();
  const b = deferred();
  const state = fixture({ getExportInfo: (id) => (id === "a" ? a : b).promise, listExportDir: async () => [] });
  const first = state.loadInfo();
  state.currentUuid.value = "b";
  const second = state.loadInfo();
  a.resolve({ name: "A", mods: [] });
  await first;
  assert.equal(state.loadingInfo.value, true);
  b.resolve({ name: "B", mods: [{ path: "mods/b.jar", url: "https://example.invalid/b" }] });
  await second;
  assert.equal(state.info.value.name, "B");
  assert.deepEqual([...state.pickedMods.value.keys()], ["mods/b.jar"]);
});

test("目录加载属于原实例，完成前不能导出，迟到后不能污染新实例", async () => {
  const a = deferred();
  const state = fixture({ getExportInfo: async (id) => ({ name: id, mods: [] }),
    listExportDir: (id) => id === "a" ? a.promise : Promise.resolve([]) });
  const first = state.loadInfo();
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(state.loadingInfo.value, true);
  state.currentUuid.value = "b";
  await state.loadInfo();
  a.resolve([{ path: "config", dir: true }]);
  await first;
  assert.equal(state.info.value.name, "b");
  assert.equal(state.pickedFiles.value.size, 0);
});

test("清空实例后清除旧快照和勾选", async () => {
  const state = fixture({ getExportInfo: async () => ({ name: "a", mods: [{ path: "mods/a.jar" }] }), listExportDir: async () => [] });
  await state.loadInfo();
  state.currentUuid.value = "";
  await state.loadInfo();
  assert.equal(state.info.value, null);
  assert.equal(state.pickedMods.value.size, 0);
  assert.equal(state.loadingInfo.value, false);
});

test("旧请求失败不能清除新实例，当前失败解除加载并显示原因", async () => {
  const a = deferred();
  const state = fixture({ getExportInfo: (id) => id === "a" ? a.promise : Promise.resolve({ name: "b", mods: [] }), listExportDir: async () => [] });
  const first = state.loadInfo();
  state.currentUuid.value = "b";
  await state.loadInfo();
  a.reject("stale");
  await first;
  assert.equal(state.info.value.name, "b");
  assert.deepEqual(state.errors, []);
  const failed = fixture({ getExportInfo: async () => { throw "unreadable"; } });
  await failed.loadInfo();
  assert.equal(failed.info.value, null);
  assert.equal(failed.loadingInfo.value, false);
  assert.deepEqual(failed.errors, ["unreadable"]);
});
