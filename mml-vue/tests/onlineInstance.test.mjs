// 在线实例地址口径：server.json 与整合包压缩包怎么分、目录怎么补清单文件名
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../src/lib/onlineInstance.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;

/** 用 CommonJS 的方式跑一遍转译结果，拿到导出的两个函数 */
function load() {
  const exports = {};
  new Function("exports", "module", js)(exports, { exports });
  return exports;
}

const { isArchiveUrl, serverPackUrl } = load();

test("压缩包地址走下载压缩包那条路", () => {
  for (const url of [
    "https://example.com/pack.zip",
    "https://example.com/a/b.mrpack",
    "https://example.com/pack.ZIP?raw=1",
    "https://example.com/pack.7z#top",
  ]) {
    assert.equal(isArchiveUrl(url), true, url);
  }
});

test("清单地址与目录不算压缩包", () => {
  for (const url of [
    "https://example.com/pack/server.json",
    "https://example.com/pack",
    "https://example.com/pack/",
  ]) {
    assert.equal(isArchiveUrl(url), false, url);
  }
});

test("目录地址补上 server.json", () => {
  assert.equal(serverPackUrl("https://example.com/pack"), "https://example.com/pack/server.json");
  assert.equal(serverPackUrl(" https://example.com/pack/ "), "https://example.com/pack/server.json");
  assert.equal(serverPackUrl("https://example.com"), "https://example.com/server.json");
});

test("已经是清单地址就原样留着，大小写不敏感", () => {
  assert.equal(serverPackUrl("https://example.com/pack/server.json"), "https://example.com/pack/server.json");
  assert.equal(serverPackUrl("https://example.com/pack/SERVER.JSON"), "https://example.com/pack/SERVER.JSON");
  assert.equal(serverPackUrl("server.json"), "server.json");
});

test("空地址保持空串，交给必填校验", () => {
  assert.equal(serverPackUrl("   "), "");
});
