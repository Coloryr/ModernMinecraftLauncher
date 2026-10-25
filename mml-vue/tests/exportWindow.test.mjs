// 在真实组件函数上控制 IPC 返回顺序，不启动窗口。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

/** 这一组函数互相调用（loadInfo → applyDefaults → reseedFilePicks → syncModExclusions），少抽一个就会在运行期 ReferenceError */
const FUNCTIONS = ["isOnlineId", "isOnlineMod", "seedModPicks", "reseedFilePicks", "syncModExclusions", "applyDefaults", "loadInfo"];

/** 两个请求序号是模块级 let，不带上就取不到 */
const LOCALS = ["infoRequest", "picksSeq"];

/** 从组件里抽出上面那几个函数与序号声明 */
function componentFunctions() {
  const source = readFileSync(new URL("../src/windows/export/ExportWindow.vue", import.meta.url), "utf8")
    .split('<script setup lang="ts">')[1].split("</script>")[0];
  const parsed = ts.createSourceFile("ExportWindow.ts", source, ts.ScriptTarget.Latest, true);
  const statements = parsed.statements.filter((node) =>
    (ts.isFunctionDeclaration(node) && FUNCTIONS.includes(node.name?.text))
    || LOCALS.some((name) => node.getText(parsed).startsWith(`let ${name} =`)));
  // 名字对不上时宁可直接失败，也别让后面的用例在缺函数的情况下跑出假阴性
  assert.equal(statements.filter(ts.isFunctionDeclaration).length, FUNCTIONS.length);
  assert.equal(statements.length - FUNCTIONS.length, LOCALS.length);
  return ts.transpileModule(statements.map((node) => node.getText(parsed)).join("\n"),
    { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
}

const js = componentFunctions();

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

/** 铺一套后端给的真实入参形状：清单格式、清单开关开、运行期目录有默认跳过 */
function fixture(api) {
  const refs = Object.fromEntries(Object.entries({ currentUuid: "a", info: null, loadingInfo: false,
    packName: "", summary: "", pickedMods: new Map(), pickedFiles: new Map(), unpickedFiles: new Set(),
    autoExcluded: new Set(), urls: new Map(), modIds: new Map(), pack: "curseforge", modsInManifest: true })
    .map(([key, value]) => [key, { value }]));
  const errors = [];
  const context = { ...refs,
    api: { getExportInfo: async () => null, listExportDir: async () => [], listExportInstanceDir: async () => [], ...api },
    DEFAULT_SKIP: new Set(["logs", "crash-reports"]), GAME_DIR: ".minecraft",
    showToast: (e) => errors.push(e), tErr: String };
  const loadInfo = new Function(...Object.keys(context), `${js}\nreturn loadInfo;`)(...Object.values(context));
  return { ...refs, loadInfo, errors };
}

test("旧实例信息迟到不能覆盖当前实例，也不能提前解除加载状态", async () => {
  const a = deferred();
  const b = deferred();
  const state = fixture({ getExportInfo: (id) => (id === "a" ? a : b).promise });
  const first = state.loadInfo();
  state.currentUuid.value = "b";
  const second = state.loadInfo();
  a.resolve({ name: "A", mods: [] });
  await first;
  assert.equal(state.loadingInfo.value, true);
  b.resolve({ name: "B", mods: [{ path: "mods/b.jar", url: "https://example.invalid/b", modId: "11", fileId: "22" }] });
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
  const state = fixture({ getExportInfo: async () => ({ name: "a", mods: [{ path: "mods/a.jar", modId: "11", fileId: "22" }] }) });
  await state.loadInfo();
  assert.equal(state.pickedMods.value.size, 1);
  state.currentUuid.value = "";
  await state.loadInfo();
  assert.equal(state.info.value, null);
  assert.equal(state.pickedMods.value.size, 0);
  assert.equal(state.loadingInfo.value, false);
});

test("旧请求失败不能清除新实例，当前失败解除加载并显示原因", async () => {
  const a = deferred();
  const state = fixture({ getExportInfo: (id) => id === "a" ? a.promise : Promise.resolve({ name: "b", mods: [] }) });
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

test("只有下载地址、没有项目编号的模组不按在线勾选，仍随文件打包", async () => {
  const state = fixture({ getExportInfo: async () => ({ name: "a", mods: [{ path: "mods/a.jar", url: "https://example.invalid/a" }] }) });
  await state.loadInfo();
  assert.equal(state.pickedMods.value.size, 0);
  assert.deepEqual([...state.urls.value.keys()], ["mods/a.jar"]);
});

test("文件默认勾选跳过运行期目录，模组与文件不重号", async () => {
  const state = fixture({ getExportInfo: async () => ({ name: "a", mods: [] }),
    listExportDir: async () => [{ path: "mods", dir: true }, { path: "config", dir: true }, { path: "logs", dir: true }] });
  await state.loadInfo();
  assert.deepEqual([...state.pickedFiles.value.keys()], ["mods", "config"]);
  assert.equal(state.unpickedFiles.value.size, 0);
  assert.equal(state.autoExcluded.value.size, 0);
});
