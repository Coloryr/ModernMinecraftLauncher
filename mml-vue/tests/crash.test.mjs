// 崩溃报告文件名的解析与"最近一份"的挑选
import test from "node:test";
import assert from "node:assert/strict";
import { crashTimeOf, latestCrash } from "../src/lib/crash.ts";

/** 按 Minecraft 的命名规则拼一个崩溃报告文件名 */
function crashName(date) {
  const p = (n) => String(n).padStart(2, "0");
  return `crash-${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}_${p(date.getHours())}.${p(date.getMinutes())}.${p(date.getSeconds())}-client.txt`;
}

test("从文件名解析崩溃时间", () => {
  const path = "C:\\game\\crash-reports\\crash-2026-10-21_17.05.32-client.txt";
  assert.equal(crashTimeOf(path), new Date(2026, 9, 21, 17, 5, 32).getTime());
  assert.equal(crashTimeOf("latest.log"), null);
  assert.equal(crashTimeOf("crash-broken.txt"), null);
});

test("只挑 2 分钟内最新的崩溃报告", () => {
  const now = new Date();
  const recent = new Date(now.getTime() - 30_000);
  const older = new Date(now.getTime() - 60_000);
  const stale = new Date(now.getTime() - 3 * 60_000);

  const files = [
    `logs/${crashName(older)}`,
    `logs/${crashName(recent)}`,
    `logs/${crashName(stale)}`,
    "logs/latest.log",
  ];

  assert.equal(latestCrash(files), `logs/${crashName(recent)}`);
  assert.equal(latestCrash([`logs/${crashName(stale)}`]), null);
  assert.equal(latestCrash([]), null);
});
