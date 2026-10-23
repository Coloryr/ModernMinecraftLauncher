// 直接运行组件中的状态函数，延迟 IPC 响应以验证编辑内容不会被异步操作覆盖。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
import { ref, computed } from "vue";
import { applyHunks, diffLines, joinLines, splitLines } from "../src/lib/textDiff.ts";

const source = readFileSync(new URL("../src/windows/config/ConfigWindow.vue", import.meta.url), "utf8")
  .split('<script setup lang="ts">')[1].split("</script>")[0];
const parsed = ts.createSourceFile("ConfigWindow.ts", source, ts.ScriptTarget.Latest, true);

function editor(overrides = {}) {
  const state = {
    selected: ref({ name: "first.json", fileType: "json" }),
    text: ref("edited"), origin: ref("original"), originSha1: ref("old-hash"),
    isText: ref(true), saving: ref(false), nbtSaving: ref(false), nbtDirty: ref(false),
    pendingDiscard: ref(null), loading: ref(false), instanceUuid: ref("instance"),
    source: ref(""), expanded: ref(new Set()), query: ref(""), files: ref([]),
    TEXT_TYPES: ["json"], clearCleanSessions: async () => {},
    listConfigFiles: async () => [], readConfigFile: async () => ({ text: "second", sha1: "new-hash" }),
    showToast: () => {}, t: (key) => key, tErr: (error) => String(error),
    applyHunks, diffLines, joinLines, splitLines,
    ...overrides,
  };
  state.dirty = computed(() => state.text.value !== state.origin.value);
  state.busy = computed(() => state.saving.value || state.nbtSaving.value);
  const names = ["allowTextLoss", "answerDiscard", "open", "loadFiles", "pickSource", "saveText"];
  const code = parsed.statements.filter((node) => ts.isFunctionDeclaration(node) && names.includes(node.name?.text))
    .map((node) => node.getText(parsed)).join("\n");
  const js = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return { state, ...new Function(...Object.keys(state), `${js}\nreturn {${names.join(",")}}`)(...Object.values(state)) };
}

test("切文件、刷新、换来源取消后保留未保存文本", async () => {
  for (const action of [(e) => e.open({ name: "second.json", fileType: "json" }), (e) => e.loadFiles(), (e) => e.pickSource("world")]) {
    const e = editor();
    const work = action(e);
    assert.equal(typeof e.state.pendingDiscard.value, "function");
    e.answerDiscard(false);
    await work;
    assert.equal(e.state.text.value, "edited");
    assert.equal(e.state.selected.value.name, "first.json");
    assert.equal(e.state.source.value, "");
    assert.equal(e.state.dirty.value, true);
  }
});

test("明确放弃后才读取另一个文件，保存期间拒绝切换", async () => {
  const e = editor();
  const work = e.open({ name: "second.json", fileType: "json" });
  e.answerDiscard(true);
  await work;
  assert.equal(e.state.text.value, "second");
  e.state.saving.value = true;
  await e.open({ name: "third.json", fileType: "json" });
  assert.equal(e.state.selected.value.name, "second.json");
});

test("保存只确认提交时的文本，等待期间的新编辑仍保持未保存", async () => {
  let complete;
  const writes = [];
  const e = editor({ writeConfigFile: (...args) => {
    writes.push(args);
    return new Promise((resolve) => { complete = resolve; });
  } });
  const saving = e.saveText();
  e.state.text.value = "newer edit";
  complete("saved-hash");
  await saving;
  assert.equal(e.state.origin.value, "edited");
  assert.equal(e.state.text.value, "newer edit");
  assert.equal(e.state.dirty.value, true);
  const nextSave = e.saveText();
  assert.equal(writes[1][3], "saved-hash");
  assert.equal(joinLines(applyHunks(splitLines("edited"), writes[1][5])), "newer edit");
  complete("next-hash");
  await nextSave;
  assert.equal(e.state.dirty.value, false);
});

test("保存期间变更了上下文，旧响应不覆盖新文件基线", async () => {
  let complete;
  const e = editor({ writeConfigFile: () => new Promise((resolve) => { complete = resolve; }) });
  const saving = e.saveText();
  e.state.selected.value = { name: "other.json", fileType: "json" };
  e.state.origin.value = "other file";
  e.state.originSha1.value = "other-hash";
  complete("old-file-hash");
  await saving;
  assert.equal(e.state.origin.value, "other file");
  assert.equal(e.state.originSha1.value, "other-hash");
});
