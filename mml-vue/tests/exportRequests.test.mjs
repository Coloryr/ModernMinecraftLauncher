// 使用真实组件函数，控制保存对话框和 IPC 的返回时机。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
import { computed, ref } from "vue";

function functions(path, names, context) {
  const source = readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8")
    .split('<script setup lang="ts">')[1].split("</script>")[0];
  const parsed = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
  const selected = parsed.statements.filter((node) => ts.isFunctionDeclaration(node) && names.includes(node.name?.text));
  assert.equal(selected.length, names.length);
  const js = ts.transpileModule(selected.map((node) => node.getText(parsed)).join("\n")
    .replaceAll('import("@tauri-apps/plugin-dialog")', "dialogModule"),
  { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(context), `${js}\nreturn ${names[0]};`)(...Object.values(context));
}

function deferred() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}

function modFixture(save = async () => "mods.txt") {
  const calls = [], messages = [], closed = [];
  const state = { props: { uuid: "a", selectedIds: ["one"] }, format: ref("txt"), fields: ref(["name", "file"]), onlySelected: ref(true), busy: ref(false) };
  const context = { ...state, dialogModule: Promise.resolve({ save }),
    commands: { resource: { exportMods: async (...args) => { calls.push(args); return 1; } } },
    showToast: (text) => messages.push(text), t: (key) => key, tErr: String, emit: (event) => closed.push(event) };
  return { ...state, calls, messages, closed, saveList: functions("windows/resource/parts/mod/ModExportModal.vue", ["saveList"], context) };
}

test("模组导出固定点击时的实例、所选 ID、格式和字段", async () => {
  const dialog = deferred();
  const state = modFixture(() => dialog.promise);
  const saving = state.saveList();
  state.props.uuid = "b";
  state.props.selectedIds.splice(0);
  state.format.value = "json";
  state.fields.value.push("note");
  dialog.resolve("mods.txt");
  await saving;
  assert.deepEqual(state.calls, [["a", { file: "mods.txt", format: "txt", fields: ["name", "file"], ids: ["one"] }]]);
  assert.deepEqual(state.closed, ["close"]);
});

test("仅导出所选但选择已清空时不能退化为导出全部", async () => {
  let opened = false;
  const state = modFixture(async () => { opened = true; return "mods.txt"; });
  state.props.selectedIds = [];
  await state.saveList();
  assert.equal(opened, false);
  assert.deepEqual(state.calls, []);
  state.onlySelected.value = false;
  await state.saveList();
  assert.deepEqual(state.calls[0][1].ids, []);
});

test("取消或打开保存对话框失败后不导出，并恢复按钮", async () => {
  for (const save of [async () => null, async () => { throw new Error("dialog failed"); }]) {
    const state = modFixture(save);
    await state.saveList();
    assert.equal(state.busy.value, false);
    assert.deepEqual(state.calls, []);
    assert.deepEqual(state.closed, []);
  }
});

test("实例信息在加载对话框模块前固定目标，不跟随后续切换", async () => {
  const module = deferred(), calls = [], dialogs = [];
  const props = { uuid: "a", name: "实例 A", disabled: false };
  const busy = ref(false);
  const saveInfo = functions("windows/export/parts/InfoExportButton.vue", ["saveInfo"], {
    props, busy, dialogModule: module.promise, commands: { export: { generateInfo: async (...args) => calls.push(args) } },
    showToast() {}, t: (key) => key, tErr: String,
  });
  const saving = saveInfo();
  props.uuid = "b";
  props.name = "实例 B";
  module.resolve({ save: async (options) => { dialogs.push(options); return "a-info.json"; } });
  await saving;
  assert.equal(dialogs[0].defaultPath, "实例 A-info.json");
  assert.deepEqual(calls, [["a", "a-info.json"]]);
  assert.equal(busy.value, false);
});

function packFixture(save) {
  const calls = [], errors = [];
  // 单实例导出：不是服务器包窗口，实例多选那几个引用短路不取
  const state = Object.fromEntries(Object.entries({ currentUuid: "a", preparing: false, loadingInfo: false, progress: null,
    pack: "modrinth", includeLauncher: false, packName: "A", author: "author", version: "1", summary: "summary",
    packLabel: "Modrinth", fileExt: "mrpack", zipLevel: "normal", customLoaderLost: false,
    info: { name: "A", canBundleLauncher: true },
    pickedMods: new Map([["mods/a.jar", {}]]), pickedFiles: new Map(), unpickedFiles: new Set(),
    urls: new Map([["mods/a.jar", "https://example.invalid/a"]]), modIds: new Map(),
    serverPack: false, serverPicked: [], guiConfig: null,
  }).map(([key, value]) => [key, ref(value)]));
  const serverOptions = { config: true, guiConfig: true, page: false, css: false, home: false };
  const busy = computed(() => state.preparing.value || state.progress.value?.state === "running");
  const startExport = functions("windows/export/ExportWindow.vue", ["startExport", "picksOf", "filePicks"], {
    ...state, serverOptions, busy, inTauri: true, dialogModule: Promise.resolve({ save }), loadCustomAssets: async () => {},
    api: { runExport: async (...args) => calls.push(args) }, showToast: (e) => errors.push(e), t: (key) => key, tErr: String,
  });
  return { ...state, calls, errors, startExport };
}

test("实例包固定保存前的请求，并阻止双击打开第二个保存对话框", async () => {
  const dialog = deferred(), options = [];
  const state = packFixture((value) => { options.push(value); return dialog.promise; });
  const saving = state.startExport();
  await state.startExport();
  state.currentUuid.value = "b";
  state.pack.value = "hmcl";
  state.pickedMods.value.clear();
  state.urls.value.clear();
  dialog.resolve("a.mrpack");
  await saving;
  assert.equal(options.length, 1);
  assert.equal(options[0].defaultPath, "A.mrpack");
  assert.equal(state.calls[0][0], "a");
  assert.equal(state.calls[0][1].pack, "modrinth");
  assert.deepEqual(state.calls[0][1].mods, [{ path: "mods/a.jar", url: "https://example.invalid/a", modId: null, fileId: null, exclude: false }]);
  assert.deepEqual(state.calls[0][1].files, []);
  assert.equal(state.preparing.value, false);
});

test("实例包保存取消或失败不进入运行状态", async () => {
  for (const save of [async () => null, async () => { throw new Error("denied"); }]) {
    const state = packFixture(save);
    await state.startExport();
    assert.equal(state.progress.value, null);
    assert.equal(state.preparing.value, false);
    assert.deepEqual(state.calls, []);
  }
});
