/* eslint-disable no-console, no-empty, no-undef, preserve-caught-error */
// 前端页面截图器：Vite 开发服务器 + Edge headless + CDP。
//
// 不引入 Playwright / Puppeteer：Node 自带 WebSocket，直接说 CDP。
// 每个场景新建一个 target，注入 mock.js（替换 Tauri 运行时）+ 假数据，再截图。
//
// 用法：
//   node scripts/screenshot/run.mjs                       # 全部场景
//   node scripts/screenshot/run.mjs --scene=main-grid     # 只跑指定场景（精确名字，逗号分隔）
//   node scripts/screenshot/run.mjs main-grid settings-ui # 同上，位置参数写法
//   node scripts/screenshot/run.mjs --only=settings       # 只跑 id 含 settings 的
//   node scripts/screenshot/run.mjs --list[=md]           # 列场景名 / 生成 Markdown 表
//   node scripts/screenshot/run.mjs --clean               # 先清空旧截图再跑
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, markdownTable } from "./catalog.mjs";
import { scenarios } from "./scenarios.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const VUE_DIR = resolve(HERE, "../..");
const ROOT = resolve(VUE_DIR, "..");
const MOCK = readFileSync(join(HERE, "mock.js"), "utf8");

const args = process.argv.slice(2);
const flag = (name) => args.some((a) => a === "--" + name);
const value = (name, fallback) => {
  const hit = args.find((a) => a.startsWith("--" + name + "="));
  return hit ? hit.slice(name.length + 3) : fallback;
};
const only = value("only", "");
// 排除：--skip=lobby 按 id 子串排除，逗号分隔多组
const skip = value("skip", "").split(",").map((s) => s.trim()).filter(Boolean);
// 精确名字：--scene=id / --ids=id / 直接写位置参数，三种写法等价
const named = [
  ...value("scene", "").split(","),
  ...value("ids", "").split(","),
  ...args.filter((a) => !a.startsWith("--")),
].map((s) => s.trim()).filter(Boolean);
const clean = flag("clean");
const listMode = value("list", flag("list") ? "ids" : "");
const OUT_DIR = resolve(ROOT, value("out", "target/screens"));
const PORT = Number(value("port", "1420"));
const EDGE = value("edge", "C:\\\\Program Files (x86)\\\\Microsoft\\\\Edge\\\\Application\\\\msedge.exe");

if (listMode === "ids") {
  for (const s of scenarios) console.log(s.id);
  process.exit(0);
}
if (listMode === "md") {
  console.log(markdownTable(scenarios, describe));
  process.exit(0);
}
if (flag("help")) {
  console.log([
    "用法: node scripts/screenshot/run.mjs [场景名…] [选项]",
    "",
    "场景名   精确匹配 id，可写多个或用逗号分隔；不写则跑全部",
    "         node scripts/screenshot/run.mjs main-grid settings-ui",
    "         node scripts/screenshot/run.mjs --scene=main-grid,settings-ui",
    "--only=  按 id 子串筛选，例如 --only=settings",
    "--skip=  按 id 子串排除，例如 --skip=lobby（逗号分隔多组）",
    "--list   只列场景 id；--list=md 输出 Markdown 表",
    "--clean  先清空输出目录里的旧截图与 report.json",
    "--out=   输出目录，默认 target/screens",
    "--edge=  浏览器路径，默认 Windows Edge",
    "--port=  Vite 端口，默认 1420",
  ].join("\n"));
  process.exit(0);
}

const missing = named.filter((id) => !scenarios.some((s) => s.id === id));
if (missing.length) {
  console.error("找不到场景:", missing.join(", "));
  process.exit(1);
}

const chosen = (named.length
  ? scenarios.filter((s) => named.includes(s.id))
  : only
    ? scenarios.filter((s) => s.id.includes(only))
    : scenarios).filter((s) => !skip.some((k) => s.id.includes(k)));
if (!chosen.length) {
  console.error("没有匹配的场景:", only);
  process.exit(1);
}
if (!existsSync(EDGE)) {
  console.error("找不到 Edge:", EDGE);
  process.exit(1);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** 杀掉子进程及其派生进程：Vite / Edge 都是 shell 拉起来的，只杀 shell 会留下监听端口 */
function killTree(child) {
  if (!child) return;
  if (process.platform === "win32") {
    if (child.exitCode === null) spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
    return;
  }
  // 两个子进程都以独立进程组启动，向整组发信号才能收掉 npm 下面的 vite
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    try {
      child.kill();
    } catch { }
  }
}

/** 等 Vite 起来；已经在跑就直接用 */
async function ensureVite() {
  const url = "http://127.0.0.1:" + PORT + "/";
  for (let i = 0; i < 20; i += 1) {
    try {
      const res = await fetch(url);
      if (res.ok) return null;
    } catch { }
    await sleep(300);
  }
  console.log("[shots] 启动 Vite …");
  const child = spawn("npm run dev -- --host 127.0.0.1 --port " + PORT, {
    cwd: VUE_DIR,
    shell: true,
    stdio: "ignore",
    detached: process.platform !== "win32",
  });
  for (let i = 0; i < 120; i += 1) {
    await sleep(500);
    try {
      const res = await fetch(url);
      if (res.ok) return child;
    } catch { }
  }
  killTree(child);
  throw new Error("Vite 启动超时");
}

/** 一个很薄的 CDP 客户端 */
class Cdp {
  constructor(url) {
    this.url = url;
    this.id = 0;
    this.pending = new Map();
    this.handlers = new Map();
  }
  async open() {
    this.ws = new WebSocket(this.url);
    this.ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve: ok, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(msg.error.message));
        else ok(msg.result);
        return;
      }
      const key = msg.method;
      for (const cb of this.handlers.get(key) || []) cb(msg.params, msg.sessionId);
    };
    await new Promise((resolve, reject) => {
      this.ws.onopen = resolve;
      this.ws.onerror = reject;
    });
  }
  send(method, params = {}, sessionId) {
    const id = ++this.id;
    const payload = { id, method, params };
    if (sessionId) payload.sessionId = sessionId;
    this.ws.send(JSON.stringify(payload));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error("CDP 超时: " + method));
        }
      }, 30000);
    });
  }
  on(method, cb) {
    if (!this.handlers.has(method)) this.handlers.set(method, []);
    this.handlers.get(method).push(cb);
  }
}

async function evalIn(cdp, sessionId, expression) {
  const res = await cdp.send(
    "Runtime.evaluate",
    { expression, returnByValue: true, awaitPromise: true, userGesture: true },
    sessionId,
  );
  if (res.exceptionDetails) {
    throw new Error(res.exceptionDetails.text + " " + (res.exceptionDetails.exception?.description || ""));
  }
  return res.result.value;
}

async function waitReady(cdp, sessionId, scenario) {
  const selector = JSON.stringify(scenario.ready || "#app");
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      const ok = await evalIn(
        cdp,
        sessionId,
        "(() => { const el = document.querySelector(" + selector + "); return !!(el && el.children.length && document.readyState === 'complete'); })()",
      );
      if (ok) {
        await sleep(700);
        await evalIn(cdp, sessionId, "document.documentElement.classList.add('no-anim')");
        await sleep(120);
        return;
      }
    } catch { }
    await sleep(120);
  }
  throw new Error("页面没有就绪: " + scenario.id);
}

function scenarioSource(scenario) {
  const payload = {
    commands: scenario.commands || {},
    label: scenario.label || "mml-" + scenario.kind,
    width: (scenario.size || [1280, 800])[0],
    height: (scenario.size || [1280, 800])[1],
    savePath: scenario.savePath || undefined,
    openPath: scenario.openPath || undefined,
  };
  return "window.__SHOT__ = " + JSON.stringify(payload) + ";\n" + MOCK;
}

async function shoot(cdp, browser, scenario, index) {
  const [width, height] = scenario.size || [1280, 800];
  const { targetId } = await browser.send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await browser.send("Target.attachToTarget", { targetId, flatten: true });
  const consoleErrors = [];
  const asserts = [];
  cdp.on("Runtime.exceptionThrown", (params, sid) => {
    if (sid === sessionId) consoleErrors.push(params.exceptionDetails?.text || "exception");
  });
  cdp.on("Runtime.consoleAPICalled", (params, sid) => {
    if (sid === sessionId && (params.type === "error" || params.type === "warning")) {
      consoleErrors.push(
        params.type + ": " + params.args.map((a) => a.value ?? a.description ?? a.type).join(" "),
      );
    }
  });

  try {
    await cdp.send("Page.enable", {}, sessionId);
    await cdp.send("Runtime.enable", {}, sessionId);
    await cdp.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false }, sessionId);
    await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: scenarioSource(scenario) }, sessionId);
    const query = scenario.query ? "&" + scenario.query.replace(/^\?/, "") : "";
    // 普通场景按 ?window=<kind> 载入窗口；控件预览这类独立页面直接给 path
    const url = scenario.path
      ? "http://127.0.0.1:" + PORT + scenario.path
      : "http://127.0.0.1:" + PORT + "/?window=" + scenario.kind + query;
    await cdp.send("Page.navigate", { url }, sessionId);
    await waitReady(cdp, sessionId, scenario);

    for (const action of scenario.actions || []) {
      if (typeof action === "string") await evalIn(cdp, sessionId, action);
      else if (action.wait) await sleep(action.wait);
      else if (action.eval) await evalIn(cdp, sessionId, action.eval);
      else if (action.assert) {
        // 交互型断言：点完之后确认命令真的发出去了（截图只证明画出来了）
        const ok = await evalIn(cdp, sessionId, "Boolean(" + action.assert + ")");
        if (!ok) throw new Error("断言失败: " + (action.label || action.assert));
        asserts.push(action.label || action.assert);
      }
      await sleep(action.after ?? 220);
    }

    const shot = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true }, sessionId);
    const file = join(OUT_DIR, (scenario.id || "shot-" + index) + ".png");
    writeFileSync(file, Buffer.from(shot.data, "base64"));

    const runtimeErrors = await evalIn(cdp, sessionId, "JSON.stringify(window.__SHOT__ ? window.__SHOT__.errors : [])");
    const unknown = await evalIn(cdp, sessionId, "JSON.stringify(window.__SHOT__ ? [...new Set(window.__SHOT__.unknown)] : [])");
    // 本场景实际调用过的命令：用来统计哪些命令从来没被任何场景碰到
    const calls = await evalIn(cdp, sessionId, "JSON.stringify(window.__SHOT__ ? [...new Set(window.__SHOT__.calls.map((c) => c.cmd))] : [])");
    return {
      id: scenario.id,
      kind: scenario.kind,
      file,
      consoleErrors: [...new Set(consoleErrors)].slice(0, 20),
      runtimeErrors: JSON.parse(runtimeErrors || "[]").slice(0, 20),
      unknown: JSON.parse(unknown || "[]"),
      calls: JSON.parse(calls || "[]"),
      asserts,
    };
  } catch (error) {
    let diag = "";
    try {
      diag = await evalIn(
        cdp,
        sessionId,
        "JSON.stringify({ errors: window.__SHOT__?.errors ?? [], unknown: window.__SHOT__?.unknown ?? [], text: (document.body?.innerText ?? '').slice(0, 400) })",
      );
    } catch { }
    throw new Error(String(error) + " | " + diag);
  } finally {
    await cdp.send("Target.closeTarget", { targetId }).catch(() => { });
  }
}

async function launchEdge(port, profile) {
  if (existsSync(profile)) rmSync(profile, { recursive: true, force: true });
  mkdirSync(profile, { recursive: true });
  const child = spawn(
    EDGE,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-background-networking",
      "--disable-component-update",
      "--disable-sync",
      "--mute-audio",
      "--remote-debugging-port=" + port,
      "--user-data-dir=" + profile,
      "--force-device-scale-factor=1",
      "--window-size=1400,900",
      "about:blank",
    ],
    { stdio: "ignore", detached: process.platform !== "win32" },
  );
  for (let i = 0; i < 60; i += 1) {
    await sleep(250);
    try {
      const res = await fetch("http://127.0.0.1:" + port + "/json/version");
      if (res.ok) {
        const info = await res.json();
        return { child, wsUrl: info.webSocketDebuggerUrl };
      }
    } catch { }
  }
  killTree(child);
  throw new Error("Edge 调试端口没起来");
}


/** 生成一个可直接打开的索引页：目录里的每张截图都列出，失败与未 mock 一并标出 */
function writeIndex(report) {
  // 以目录里的 PNG 为准：只跑部分场景时，旧截图也要留在索引里
  const byId = new Map(report.map((item) => [item.id, item]));
  const ids = readdirSync(OUT_DIR)
    .filter((name) => name.endsWith(".png"))
    .map((name) => name.slice(0, -4))
    .sort();
  // 失败场景没有 PNG，也要进索引，否则看不出哪张没跑出来
  for (const item of report) {
    const failed = item.error || (item.unknown && item.unknown.length) || (item.runtimeErrors && item.runtimeErrors.length);
    if (failed && !ids.includes(item.id)) ids.push(item.id);
  }
  const cards = ids
    .map((id) => {
      const item = byId.get(id) ?? { id };
      const notes = [];
      if (item.error) notes.push("失败：" + item.error);
      for (const u of item.unknown || []) notes.push("未 mock：" + u);
      for (const e of item.runtimeErrors || []) notes.push("页面错误：" + e);
      const name = id + ".png";
      const img = existsSync(join(OUT_DIR, name)) ? '<a href="' + name + '"><img loading="lazy" src="' + name + '" alt="' + id + '"></a>' : "";
      return '<figure><figcaption>' + id + "</figcaption>" + img + (notes.length ? "<p>" + notes.join("<br>") + "</p>" : "") + "</figure>";
    })
    .join("\n");
  const html = [
    "<!doctype html>",
    '<meta charset="utf-8">',
    "<title>M²L 页面截图</title>",
    "<style>",
    "body{font-family:system-ui,sans-serif;background:#eef1f5;margin:20px;color:#1d2229}",
    "h1{font-size:18px}",
    ".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:16px}",
    "figure{margin:0;background:#fff;border:1px solid #d8dce3;border-radius:10px;padding:8px}",
    "figcaption{font-size:12px;font-weight:600;margin-bottom:6px}",
    "img{width:100%;height:auto;border-radius:6px;display:block}",
    "p{font-size:11px;color:#6a7480;margin:6px 0 0}",
    "</style>",
    "<h1>M²L 页面截图 · " + ids.length + " 个场景</h1>",
    '<div class="grid">' + cards + "</div>",
  ].join("\n");
  writeFileSync(join(OUT_DIR, "index.html"), html);
}

/** PID 对应的进程是否还活着；Windows 上已退出的 PID 会抛错 */
function isProcessAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** 清掉上次异常退出留下的 Edge 配置目录；只删进程已不存在的，避免误删并行截图 */
function cleanupStaleProfiles() {
  const tempDir = join(ROOT, "target", "temp");
  if (!existsSync(tempDir)) return;
  for (const name of readdirSync(tempDir)) {
    if (!name.startsWith("shot-edge-")) continue;
    const pid = Number(name.slice("shot-edge-".length));
    if (!Number.isInteger(pid) || pid <= 0 || pid === process.pid) continue;
    if (isProcessAlive(pid)) continue;
    try {
      rmSync(join(tempDir, name), {
        recursive: true,
        force: true,
        maxRetries: 5,
        retryDelay: 200,
      });
    } catch {
      // Edge 可能刚退出还在释放文件；下次运行再清
    }
  }
}

/** 等子进程退出，超时也继续清理；Windows 上先杀父进程，文件锁通常随后释放 */
function waitExit(child, timeout) {
  return new Promise((resolve) => {
    if (child.exitCode !== null) {
      resolve();
      return;
    }
    const timer = setTimeout(resolve, timeout);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

/** 删 Edge 配置目录；被杀进程可能短暂占着文件，重试几次 */
async function removeProfile(profile) {
  for (let i = 0; i < 5; i += 1) {
    try {
      rmSync(profile, {
        recursive: true,
        force: true,
        maxRetries: 5,
        retryDelay: 200,
      });
      return;
    } catch {
      await sleep(300);
    }
  }
  console.warn("临时目录未能删除：", profile);
}

/** 合并多次运行的报告：按场景表顺序排列，同 id 以本次结果为准 */
function mergeReports(previous, current) {
  const order = new Map(scenarios.map((s, i) => [s.id, i]));
  const byId = new Map();
  for (const item of previous) if (item && item.id) byId.set(item.id, item);
  for (const item of current) if (item && item.id) byId.set(item.id, item);
  return [...byId.values()]
    .filter((item) => order.has(item.id))
    .sort((a, b) => order.get(a.id) - order.get(b.id));
}

async function main() {
  cleanupStaleProfiles();
  mkdirSync(OUT_DIR, { recursive: true });
  // 只跑部分场景时保留已有截图，方便逐张补齐；要整批重来用 --clean
  if (clean) {
    for (const name of readdirSync(OUT_DIR)) {
      if (name.endsWith(".png") || name === "report.json" || name === "index.html") {
        rmSync(join(OUT_DIR, name), { force: true });
      }
    }
  }
  let previous = [];
  try {
    const file = join(OUT_DIR, "report.json");
    if (!clean && existsSync(file)) {
      const parsed = JSON.parse(readFileSync(file, "utf8"));
      if (Array.isArray(parsed)) {
        // 图片被手动删掉的旧记录不再进索引，避免索引里出现破图
        previous = parsed.filter((item) => item && item.id && (!item.file || existsSync(item.file)));
      }
    }
  } catch {
    previous = [];
  }
  const vite = await ensureVite();
  const port = 9300 + (process.pid % 400);
  const profile = join(ROOT, "target", "temp", "shot-edge-" + process.pid);
  const report = [];
  let edge = null;
  try {
    edge = await launchEdge(port, profile);
    const cdp = new Cdp(edge.wsUrl);
    await cdp.open();
    for (let i = 0; i < chosen.length; i += 1) {
      const scenario = chosen[i];
      process.stdout.write("[" + (i + 1) + "/" + chosen.length + "] " + scenario.id + " … ");
      try {
        const item = await shoot(cdp, cdp, scenario, i);
        report.push(item);
        console.log("ok");
      } catch (e) {
        report.push({ id: scenario.id, error: String(e) });
        console.log("失败:", String(e));
      }
    }
  } finally {
    const merged = mergeReports(previous, report);
    writeFileSync(join(OUT_DIR, "report.json"), JSON.stringify(merged, null, 2));
    writeIndex(merged);
    if (edge) {
      killTree(edge.child);
      await waitExit(edge.child, 3000);
    }
    await removeProfile(profile);
    if (vite) killTree(vite);
  }
  const failed = report.filter((r) => r.error || (r.unknown && r.unknown.length) || (r.consoleErrors && r.consoleErrors.length));
  const total = readdirSync(OUT_DIR).filter((name) => name.endsWith(".png")).length;
  console.log("\n[done] 本次 " + report.length + " 张，目录共 " + total + " 张:", OUT_DIR);
  if (failed.length) {
    console.log("[有问题的场景]");
    for (const item of failed) {
      console.log("-", item.id);
      if (item.error) console.log("   ", item.error);
      for (const u of item.unknown || []) console.log("    未 mock:", u);
      for (const e of item.runtimeErrors || []) console.log("    页面错误:", e);
      for (const e of item.consoleErrors || []) console.log("    控制台:", e);
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
