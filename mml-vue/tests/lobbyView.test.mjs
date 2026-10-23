// 执行组件快照合并，覆盖工作进程重启和迟到轮询，无需启动窗口。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../src/windows/lobby/LobbyWindow.vue", import.meta.url), "utf8")
  .split('<script setup lang="ts">')[1].split("</script>")[0];
const parsed = ts.createSourceFile("LobbyWindow.ts", source, ts.ScriptTarget.Latest, true);
const code = parsed.statements.find((node) => ts.isFunctionDeclaration(node) && node.name?.text === "applyView").getText(parsed);
const js = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;

test("新会话可接受低序号状态，旧会话迟到响应不能覆盖它", () => {
  const view = { value: { sessionGeneration: 1, state: { index: 100, state: "guest-ok" }, nodes: ["old"] } };
  const selectedNodes = { value: ["old"] };
  const apply = new Function("view", "selectedNodes", `${js}\nreturn applyView;`)(view, selectedNodes);
  const current = { sessionGeneration: 2, state: { index: 1, state: "waiting" }, nodes: ["new"] };
  apply(current);
  assert.equal(view.value.state.state, "waiting");
  assert.deepEqual(selectedNodes.value, []);
  apply({ sessionGeneration: 1, state: { index: 101, state: "guest-ok" }, nodes: ["old"] });
  assert.equal(view.value, current);
  apply({ sessionGeneration: 2, state: { index: 0, state: "stopped" }, nodes: ["new"] });
  assert.equal(view.value.state.state, "waiting");
});
