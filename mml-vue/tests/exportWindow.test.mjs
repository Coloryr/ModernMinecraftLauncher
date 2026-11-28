// 在真实组件函数上控制 IPC 返回顺序，不启动窗口。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
import { fileCheckStates } from "../src/windows/export/picked.ts";

/** 这一组函数互相调用（loadInfo → applyDefaults → reseedFilePicks → syncModExclusions），少抽一个就会在运行期 ReferenceError */
const FUNCTIONS = ["isLockedFile", "isOnlineId", "isOnlineMod", "seedModPicks", "reseedFilePicks", "syncModExclusions", "applyDefaults", "loadInfo", "changeFileFormat", "toggleFile"];

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
    autoExcluded: new Set(), urls: new Map(), modIds: new Map(), pack: "curseforge", filePicksReady: false, filePicksError: null })
    .map(([key, value]) => [key, { value }]));
  // 模组页只在带清单的格式下出现：组件里是 computed，这里照着 pack 现算
  const showModsTab = { get value() { return refs.pack.value === "curseforge" || refs.pack.value === "modrinth"; } };
  const errors = [];
  const context = { ...refs, showModsTab, filePickCache: new Map(),
    api: { getExportInfo: async () => null, listExportDir: async () => [], listExportInstanceDir: async () => [], ...api },
    DEFAULT_SKIP: new Set(["logs", "crash-reports"]), MMC_SKIP: new Set(["game.json", "icon.png"]), GAME_DIR: ".minecraft",
    showToast: (e) => errors.push(e), tErr: String };
  const actions = new Function(...Object.keys(context), `${js}\nreturn { loadInfo, reseedFilePicks, changeFileFormat, toggleFile };`)(...Object.values(context));
  function setFormat(next) {
    const previous = refs.pack.value;
    refs.pack.value = next;
    return actions.changeFileFormat(next, previous);
  }
  return { ...refs, ...actions, setFormat, errors };
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
  assert.equal(state.info.value.name, "a");
  assert.equal(state.filePicksReady.value, false);
  state.currentUuid.value = "b";
  await state.loadInfo();
  a.resolve([{ path: "config", dir: true }]);
  await first;
  assert.equal(state.info.value.name, "b");
  assert.equal(state.pickedFiles.value.size, 0);
  assert.equal(state.filePicksReady.value, true);
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
  assert.equal(state.filePicksReady.value, false);
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

test("切回缓存的格式保留手选与模组排除，未读完的格式再次进入时重新读取", async () => {
  const config = { path: "config", dir: true };
  const mods = { path: "mods", dir: true };
  const onlineMod = { path: "mods/online.jar", modId: "11", fileId: "22" };
  const reads = [];
  const state = fixture({
    getExportInfo: async () => ({ name: "a", mods: [onlineMod] }),
    listExportDir: async () => [config, mods],
    listExportInstanceDir: () => { const read = deferred(); reads.push(read); return read.promise; },
  });
  await state.loadInfo();
  state.toggleFile(config, false);
  const oldRead = state.setFormat("mmc");
  assert.equal(state.filePicksReady.value, false);
  await state.setFormat("curseforge");
  assert.equal(state.filePicksReady.value, true);
  reads[0].resolve([{ path: "old" }]);
  await oldRead;
  assert.deepEqual([...state.pickedFiles.value.keys()], [mods.path]);
  assert.deepEqual(state.unpickedFiles.value, new Set([onlineMod.path, config.path]));
  assert.deepEqual(state.autoExcluded.value, new Set([onlineMod.path]));
  assert.deepEqual([...state.pickedMods.value.keys()], [onlineMod.path]);

  const freshRead = state.setFormat("mmc");
  assert.equal(state.filePicksReady.value, false);
  assert.equal(reads.length, 2);
  reads[1].resolve([{ path: "game.json" }, { path: ".minecraft", dir: true }]);
  await freshRead;
  assert.equal(state.filePicksReady.value, true);
  assert.deepEqual([...state.pickedFiles.value.keys()], [".minecraft"]);
  assert.equal(state.pickedMods.value.size, 0);
});

test("勾选读取失败允许重试，空目录可以导出，切实例后的旧错误不覆盖新状态", async () => {
  const reads = [];
  const state = fixture({
    getExportInfo: async (id) => ({ name: id, mods: [] }),
    listExportDir: async (id) => [{ path: `config-${id}`, dir: true }],
    listExportInstanceDir: (id) => {
      if (id === "b") return Promise.resolve([{ path: "game.json" }]);
      const read = deferred();
      reads.push(read);
      return read.promise;
    },
  });
  await state.loadInfo();
  const failed = state.setFormat("mmc");
  reads[0].reject("access denied");
  await failed;
  assert.equal(state.filePicksReady.value, false);
  assert.equal(state.filePicksError.value, "access denied");

  const retry = state.reseedFilePicks("a");
  assert.equal(state.filePicksError.value, null);
  assert.equal(state.filePicksReady.value, false);
  reads[1].resolve([]);
  await retry;
  assert.equal(state.pickedFiles.value.size, 0);
  assert.equal(state.filePicksReady.value, true);

  const stale = state.setFormat("mml");
  state.currentUuid.value = "b";
  await state.loadInfo();
  reads[2].reject("old format failed");
  await stale;
  assert.equal(state.filePicksError.value, null);
  assert.equal(state.filePicksReady.value, true);
  assert.deepEqual([...state.pickedFiles.value.keys()], ["game.json"]);
  await state.setFormat("curseforge");
  assert.deepEqual([...state.pickedFiles.value.keys()], ["config-b"]);
});

test("目录半选沿多级勾选和排除向上传递，目录展开后按实际子项归并", () => {
  const directory = (path) => ({ path, dir: true });
  const file = (path) => ({ path, dir: false });
  const children = new Map([["", ["mods", "mods2", "config", "empty"].map(directory)], ["empty", []]]);
  const picked = new Map(["mods", "config/nested/yes", "empty"].map((path) => [path, true]));
  const unpicked = new Set(["mods/nested/no", "empty/missing"]);
  let states = fileCheckStates(children, picked, unpicked);
  assert.deepEqual(Object.fromEntries(states), { mods: "mixed", mods2: "unchecked", config: "mixed", empty: "checked" });

  children.set("mods", [directory("mods/nested"), file("mods/yes")]);
  children.set("config", [directory("config/nested")]);
  states = fileCheckStates(children, picked, unpicked);
  assert.equal(states.get("mods/nested"), "mixed");
  assert.equal(states.get("config/nested"), "mixed");
  children.set("mods/nested", [file("mods/nested/yes")]);
  children.set("config/nested", [file("config/nested/yes"), file("config/nested/no")]);
  states = fileCheckStates(children, picked, unpicked);
  assert.equal(states.get("mods"), "checked");
  assert.equal(states.get("config"), "mixed");
  assert.equal(states.get("config/nested/yes"), "checked");
  assert.equal(states.get("config/nested/no"), "unchecked");
  unpicked.add("config/nested/yes");
  assert.equal(fileCheckStates(children, picked, unpicked).get("config"), "unchecked");
});

test("整选半选目录会清除后代例外和模组清单，整取消不影响同前缀兄弟", async () => {
  const roots = ["mods", "mods-extra"].map((path) => ({ path, dir: true }));
  const mods = roots.map((root) => ({ path: `${root.path}/deep/online.jar`, modId: "11", fileId: "22" }));
  const state = fixture({ getExportInfo: async () => ({ name: "a", mods }), listExportDir: async () => roots });
  await state.loadInfo();
  state.toggleFile({ path: "mods/deep/manual.jar", dir: false }, false);
  state.toggleFile({ path: "mods/deep/local.jar", dir: false }, true);
  const checkState = (path) => fileCheckStates(new Map([["", roots]]), state.pickedFiles.value, state.unpickedFiles.value).get(path);
  assert.equal(checkState("mods"), "mixed");
  state.toggleFile(roots[0], true);
  assert.equal(checkState("mods"), "checked");
  assert.deepEqual([...state.pickedFiles.value.keys()], ["mods", "mods-extra"]);
  assert.deepEqual([...state.pickedMods.value.keys()], [mods[1].path]);
  assert.deepEqual(state.autoExcluded.value, new Set([mods[1].path]));
  assert.deepEqual(state.unpickedFiles.value, new Set([mods[1].path]));
  state.toggleFile(roots[0], false);
  assert.equal(checkState("mods"), "unchecked");
  assert.equal(checkState("mods-extra"), "mixed");
});
