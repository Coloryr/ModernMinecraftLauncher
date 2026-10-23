// 直接执行组件中的宽度与拖动逻辑，不启动窗口或捕获界面。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
import { computed, ref } from "vue";

const source = readFileSync(new URL("../src/windows/config/ConfigWindow.vue", import.meta.url), "utf8")
  .split('<script setup lang="ts">')[1].split("</script>")[0];
const parsed = ts.createSourceFile("ConfigWindow.ts", source, ts.ScriptTarget.Latest, true);
const variables = ["treeWidth", "resizing", "TREE_MIN_WIDTH", "TREE_MAX_WIDTH", "bodyWidth",
  "treeMaxWidth", "treeMinWidth", "visibleTreeWidth", "stopResize"];
const functions = ["clampTreeWidth", "persistTreeWidth", "startResize", "onResizeKeydown"];
const code = parsed.statements.filter((node) =>
  (ts.isVariableStatement(node) && node.declarationList.declarations.some((decl) => variables.includes(decl.name.getText(parsed))))
  || (ts.isFunctionDeclaration(node) && functions.includes(node.name?.text)),
).map((node) => node.getText(parsed)).join("\n");
const js = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;

function layout() {
  const listeners = new Map();
  const writes = [];
  const window = {
    addEventListener(type, fn) {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type).add(fn);
    },
    removeEventListener(type, fn) { listeners.get(type)?.delete(fn); },
  };
  const state = new Function("computed", "ref", "window", "setConfigTreeWidth", "showToast", "tErr",
    `${js}\nreturn { ${variables.filter((name) => name !== "stopResize").join(",")}, ${functions.join(",")}, dispose: () => stopResize?.() };`,
  )(computed, ref, window, async (width) => { writes.push(width); }, assert.fail, String);
  return {
    ...state, writes,
    dispatch(type, event = {}) {
      for (const fn of [...(listeners.get(type) ?? [])]) fn(event);
    },
    listenerCount() { return [...listeners.values()].reduce((sum, set) => sum + set.size, 0); },
  };
}

function pointer(clientX, pointerId = 1) {
  return { clientX, pointerId, button: 0, isPrimary: true, preventDefault() {} };
}

test("窄窗口保留编辑区空间，恢复窗口宽度后仍使用原偏好", () => {
  const l = layout();
  l.treeWidth.value = 600;
  l.bodyWidth.value = 800;
  assert.equal(l.visibleTreeWidth.value, 400);
  assert.equal(l.treeWidth.value, 600);
  l.bodyWidth.value = 1200;
  assert.equal(l.visibleTreeWidth.value, 600);
  l.bodyWidth.value = 2000;
  l.treeWidth.value = 1000;
  assert.equal(l.visibleTreeWidth.value, 640);
  l.treeWidth.value = 100;
  assert.equal(l.visibleTreeWidth.value, 200);
  l.bodyWidth.value = 320;
  assert.equal(l.visibleTreeWidth.value, 160);
  assert.deepEqual(l.writes, []);
});

test("拖动从实际可见宽度开始，仅处理当前指针并在松手后保存一次", () => {
  const l = layout();
  l.treeWidth.value = 600;
  l.bodyWidth.value = 800;
  l.startResize(pointer(400));
  l.dispatch("pointermove", pointer(380));
  assert.equal(l.treeWidth.value, 380);
  l.dispatch("pointermove", pointer(100, 2));
  l.dispatch("pointerup", pointer(100, 2));
  assert.equal(l.treeWidth.value, 380);
  assert.equal(l.resizing.value, true);
  assert.deepEqual(l.writes, []);
  l.dispatch("pointerup", pointer(380));
  assert.deepEqual(l.writes, [380]);
  assert.equal(l.resizing.value, false);
  assert.equal(l.listenerCount(), 0);
  l.dispatch("pointerup", pointer(380));
  assert.deepEqual(l.writes, [380]);
});

test("取消或失焦还原拖动前的偏好，卸载清理后不再响应指针", () => {
  for (const end of ["pointercancel", "blur", "dispose"]) {
    const l = layout();
    l.startResize(pointer(300));
    l.dispatch("pointermove", pointer(340));
    if (end === "dispose") l.dispose();
    else l.dispatch(end, pointer(340));
    assert.equal(l.resizing.value, false);
    assert.equal(l.listenerCount(), 0);
    assert.equal(l.treeWidth.value, end === "dispose" ? 340 : 300);
    l.dispatch("pointermove", pointer(500));
    l.dispatch("pointerup", pointer(500));
    assert.deepEqual(l.writes, []);
  }
});

test("分隔线键盘调整遵守边界，无效键与边界处重复操作不写配置", () => {
  const l = layout();
  l.bodyWidth.value = 800;
  const prevented = [];
  const press = (key) => l.onResizeKeydown({ key, preventDefault() { prevented.push(key); } });
  press("ArrowRight");
  press("Home");
  press("ArrowLeft");
  press("End");
  press("ArrowRight");
  press("Tab");
  assert.deepEqual(l.writes, [316, 200, 400]);
  assert.deepEqual(prevented, ["ArrowRight", "Home", "ArrowLeft", "End", "ArrowRight"]);
  l.startResize({ ...pointer(400), button: 2 });
  assert.equal(l.listenerCount(), 0);
});
