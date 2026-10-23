// 校验真实卡片的渲染和展开交互，不创建窗口或访问用户桌面。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { compileScript, parse } from "@vue/compiler-sfc";
import { renderToString } from "@vue/server-renderer";
import * as vue from "vue";
import ts from "typescript";
import { presentFinding } from "../src/lib/crashAnalysisPresentation.ts";
import zh from "../src/lib/i18n/locales/zh-CN.ts";
import en from "../src/lib/i18n/locales/en-US.ts";

function translator(messages) {
  return (key, params = {}) => (messages[key] ?? key).replace(/\{(\w+)\}/g, (match, name) => String(params[name] ?? match));
}

function finding(id, details = []) {
  return { id, confidence: "high", severity: "error", evidence: ["original evidence"], mods: [], details };
}

function analysis(primary = finding("bytecode_verification", ["hint=lwjgl_type_conflict"])) {
  return { kind: "crash_report", description: "Initializing game", exception: "java.lang.InternalError: wrapper",
    causes: ["java.lang.VerifyError: bad type"], findings: [primary, finding("mod_entrypoint_failure")],
    suspectedMods: ["example"], minecraftVersion: "1.12.2", loader: "forge", loaderVersion: null,
    javaVersion: "25.0.1", memoryMaxMb: 4096 };
}

function component(messages) {
  const cache = new Map();
  const load = (name) => {
    if (cache.has(name)) return cache.get(name);
    const path = new URL(`../src/components/${name}`, import.meta.url);
    const { descriptor } = parse(readFileSync(path, "utf8"));
    const script = compileScript(descriptor, { id: name, inlineTemplate: true });
    const js = ts.transpileModule(script.content, { compilerOptions: {
      module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
    } }).outputText;
    const exports = {};
    const require = (id) => {
      if (id === "vue") return vue;
      if (id === "../lib/i18n") return { t: translator(messages) };
      if (id === "../lib/crashAnalysisPresentation") return { presentFinding };
      if (id.endsWith(".vue")) return { __esModule: true, default: load(id.replace(/^\.\//, "")) };
      throw new Error(`Unexpected runtime import: ${id}`);
    };
    new Function("require", "exports", js)(require, exports);
    cache.set(name, exports.default);
    return exports.default;
  };
  return load("CrashAnalysisCard.vue");
}

test("每种后端结论都有完整的中英文解释与建议", () => {
  const model = readFileSync(new URL("../../mml-core/mml-analysis/src/model.rs", import.meta.url), "utf8");
  const ids = [...model.matchAll(/FindingId::\w+ => "([a-z_]+)"/g)].map((match) => match[1]);
  assert.ok(ids.length >= 37);
  for (const messages of [zh, en]) {
    for (const id of ids) {
      for (const part of ["finding", "reason", "advice"]) assert.ok(messages[`analysis.${part}.${id}`], `${part}.${id}`);
    }
  }
});

test("诊断细节保留等号、翻译已知线索并忽略未知线索", () => {
  const result = presentFinding(finding("watchdog_timeout", ["location=at a.run(x=y)",
    "hint=watchdog_log_output", "hint=watchdog_log_output", "hint=future_hint", "future=value"]), translator(zh));
  assert.deepEqual(result.details, ["异常调用位置：at a.run(x=y)", "future=value"]);
  assert.equal(result.hints.length, 1);
  assert.match(result.hints[0], /日志输出/);
  assert.ok(!JSON.stringify(result).includes("analysis.hint."));
});

test("旧前端遇到未知结论时保留标题并提供证据不足的说明", () => {
  const result = presentFinding(finding("future_failure"), translator(en));
  assert.equal(result.title, "future_failure");
  assert.equal(result.reason, en["analysis.reason.unclassified"]);
  assert.equal(result.advice, en["analysis.advice.unclassified"]);
});

for (const [name, messages] of [["中文", zh], ["英文", en]]) {
  test(`${name}卡片直接显示建议并转义日志文本，证据默认收起`, async () => {
    const data = analysis(finding("bytecode_verification", ["hint=lwjgl_type_conflict", "location=<script>alert('x')</script>"]));
    const app = vue.createSSRApp(component(messages), { analysis: data });
    app.directive("tip", {});
    const html = await renderToString(app);
    assert.ok(html.includes(messages["analysis.finding.bytecode_verification"]));
    assert.ok(html.includes(messages["analysis.advice.bytecode_verification"].replaceAll("'", "&#39;")));
    assert.ok(html.includes("CustomBuffer"));
    assert.ok(html.includes("&lt;script&gt;"));
    assert.ok(!html.includes("<script>"));
    assert.ok(!html.includes("original evidence"));
    assert.ok(!html.includes("java.lang.InternalError: wrapper"));
  });
}

// 在内存树上执行 Vue 事件与响应式更新，不依赖浏览器或屏幕。
function mountCard(data) {
  const node = (tag, text = "") => ({ tag, text, props: {}, children: [], parent: null });
  const renderer = vue.createRenderer({
    createElement: node, createText: (text) => node("text", text), createComment: () => node("comment"),
    setText: (el, text) => { el.text = text; }, setElementText: (el, text) => { el.text = text; el.children = []; },
    patchProp: (el, key, _previous, value) => { el.props[key] = value; },
    parentNode: (el) => el.parent, nextSibling: (el) => el.parent?.children[el.parent.children.indexOf(el) + 1] ?? null,
    insert: (el, parent, anchor = null) => {
      if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1);
      el.parent = parent;
      const at = anchor ? parent.children.indexOf(anchor) : parent.children.length;
      parent.children.splice(at, 0, el);
    },
    remove: (el) => { el.parent?.children.splice(el.parent.children.indexOf(el), 1); },
  });
  const current = vue.shallowRef(data);
  const Card = component(zh);
  const app = renderer.createApp({ setup: () => () => vue.h(Card, { analysis: current.value }) });
  app.directive("tip", {});
  const root = node("root");
  app.mount(root);
  const all = (el = root) => [el, ...el.children.flatMap((child) => all(child))];
  const text = (el = root) => all(el).map((item) => item.text).join("");
  const click = async (label) => {
    const button = all().find((el) => el.tag === "button" && text(el).includes(label));
    assert.ok(button, `Missing button: ${label}`);
    button.props.onClick();
    await vue.nextTick();
  };
  return { current, text, click, unmount: () => app.unmount() };
}

test("证据展开包含异常链，次级发现有建议，切换报告后收起旧内容", async () => {
  const card = mountCard(analysis());
  try {
    await card.click("查看日志证据");
    assert.match(card.text(), /外层异常：java.lang.InternalError: wrapper/);
    assert.match(card.text(), /由此引起：java.lang.VerifyError: bad type/);
    assert.match(card.text(), /original evidence/);
    await card.click("展开其它发现");
    assert.ok(card.text().includes(zh["analysis.advice.mod_entrypoint_failure"]));
    card.current.value = { ...analysis(finding("null_pointer")), exception: "new exception", causes: [] };
    await vue.nextTick();
    assert.ok(!card.text().includes("original evidence"));
    assert.ok(!card.text().includes("java.lang.VerifyError"));
    assert.ok(!card.text().includes(zh["analysis.advice.mod_entrypoint_failure"]));
    assert.ok(card.text().includes(zh["analysis.finding.null_pointer"]));
  } finally {
    card.unmount();
  }
});
